//! Windows and Linux: clipboard through arboard, paste through enigo.
//! No biometrics yet, so Secure Clips is password-only on these platforms.
use super::Snapshot;
use arboard::{Clipboard, ImageData};
use std::borrow::Cow;
use std::sync::Mutex;

/// One shared clipboard handle. On X11 the owner must stay alive to serve what we copied.
static CLIP: Mutex<Option<Clipboard>> = Mutex::new(None);

fn with_clip<T>(f: impl FnOnce(&mut Clipboard) -> T) -> Option<T> {
    let mut guard = CLIP.lock().ok()?;
    if guard.is_none() {
        *guard = Clipboard::new().ok();
    }
    guard.as_mut().map(f)
}

// ---------- Change counter ----------

#[cfg(windows)]
pub fn change_count() -> i64 {
    unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() as i64 }
}

/// Windows counts every change itself; nothing to remember before a write.
#[cfg(windows)]
fn before_write() -> i64 {
    0
}

#[cfg(windows)]
fn written(_before: i64) -> i64 {
    change_count()
}

/// X11 has no clipboard sequence number. Preferred: count XFixes "selection owner changed"
/// events (every copy makes a new owner), so polling is a single atomic load. Fallback, if
/// XFixes isn't available: fingerprint the content on every poll, which reads (and for
/// images decodes) the whole clipboard each time.
#[cfg(not(windows))]
mod counter {
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    /// Owner changes seen by the event thread.
    static EVENTS: AtomicI64 = AtomicI64::new(0);
    /// Cleared if the event thread loses its X connection.
    static EVENTS_OK: AtomicBool = AtomicBool::new(false);
    static STARTED: OnceLock<()> = OnceLock::new();

    #[cfg(target_os = "linux")]
    fn start_events() -> Result<(), Box<dyn std::error::Error>> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
        use x11rb::protocol::xproto::{ConnectionExt as _, CreateWindowAux, WindowClass};
        use x11rb::protocol::Event;

        let (conn, screen) = x11rb::connect(None)?;
        conn.xfixes_query_version(5, 0)?.reply()?;
        let root = conn.setup().roots[screen].root;
        let win = conn.generate_id()?;
        conn.create_window(0, win, root, 0, 0, 1, 1, 0, WindowClass::INPUT_ONLY, 0, &CreateWindowAux::new())?;
        let clipboard = conn.intern_atom(false, b"CLIPBOARD")?.reply()?.atom;
        let mask = SelectionEventMask::SET_SELECTION_OWNER
            | SelectionEventMask::SELECTION_WINDOW_DESTROY
            | SelectionEventMask::SELECTION_CLIENT_CLOSE;
        conn.xfixes_select_selection_input(win, clipboard, mask)?.check()?;
        EVENTS_OK.store(true, Ordering::SeqCst);
        std::thread::Builder::new().name("x11-clipboard-events".into()).spawn(move || loop {
            match conn.wait_for_event() {
                Ok(Event::XfixesSelectionNotify(_)) => {
                    EVENTS.fetch_add(1, Ordering::SeqCst);
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!("clipboard events stopped ({e}); falling back to polling");
                    EVENTS_OK.store(false, Ordering::SeqCst);
                    break;
                }
            }
        })?;
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    fn start_events() -> Result<(), Box<dyn std::error::Error>> {
        Err("not supported".into())
    }

    fn events() -> bool {
        STARTED.get_or_init(|| {
            if let Err(e) = start_events() {
                log::warn!("XFixes clipboard events unavailable ({e}); polling clipboard content");
            }
        });
        EVENTS_OK.load(Ordering::SeqCst)
    }

    static STATE: Mutex<(i64, u64)> = Mutex::new((0, 0));

    fn fingerprint() -> u64 {
        let mut h = DefaultHasher::new();
        super::with_clip(|c| {
            if let Ok(t) = c.get_text() {
                t.hash(&mut h);
            } else if let Ok(img) = c.get_image() {
                img.bytes.hash(&mut h);
            }
        });
        h.finish()
    }

    /// Event counts and fingerprint counts are kept apart (fingerprint values are
    /// negative), so switching to the fallback mid-run is still seen as a change.
    pub fn current() -> i64 {
        if events() {
            return EVENTS.load(Ordering::SeqCst);
        }
        let fp = fingerprint();
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if fp != s.1 {
            *s = (s.0 - 1, fp);
        }
        s.0
    }

    pub fn before_write() -> i64 {
        if events() {
            EVENTS.load(Ordering::SeqCst)
        } else {
            0
        }
    }

    /// After our own write: the count the watcher will see for it, so it's skipped as ours.
    pub fn written(before: i64) -> i64 {
        if events() {
            // Our new ownership arrives as an event shortly after the write returns.
            let start = Instant::now();
            while EVENTS.load(Ordering::SeqCst) == before && start.elapsed() < Duration::from_millis(250) {
                std::thread::sleep(Duration::from_millis(2));
            }
            return EVENTS.load(Ordering::SeqCst);
        }
        let fp = fingerprint();
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        *s = (s.0 - 1, fp);
        s.0
    }
}

#[cfg(not(windows))]
pub fn change_count() -> i64 {
    counter::current()
}

#[cfg(not(windows))]
fn before_write() -> i64 {
    counter::before_write()
}

#[cfg(not(windows))]
fn written(before: i64) -> i64 {
    counter::written(before)
}

/// Runs a clipboard write and returns the change count it produced.
fn write_with(f: impl FnOnce(&mut Clipboard)) -> i64 {
    let before = before_write();
    with_clip(f);
    written(before)
}

// ---------- Read / write ----------

/// Formats password managers set on Windows to say "don't record this".
#[cfg(windows)]
fn concealed_types() -> Vec<String> {
    use windows_sys::Win32::System::DataExchange::{IsClipboardFormatAvailable, RegisterClipboardFormatW};
    let mut out = vec![];
    for name in ["ExcludeClipboardContentFromMonitorProcessing"] {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let fmt = unsafe { RegisterClipboardFormatW(wide.as_ptr()) };
        if fmt != 0 && unsafe { IsClipboardFormatAvailable(fmt) } != 0 {
            out.push(name.to_string());
        }
    }
    out
}

#[cfg(not(windows))]
fn concealed_types() -> Vec<String> {
    vec![]
}

pub fn read_snapshot() -> Snapshot {
    let types = concealed_types();
    with_clip(|c| {
        let files: Vec<String> = c
            .get()
            .file_list()
            .map(|v| v.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        let text = if files.is_empty() { c.get_text().ok() } else { None };
        let has_image = files.is_empty() && text.is_none() && c.get_image().is_ok();
        Snapshot { types, text, files, has_image }
    })
    .unwrap_or_default()
}

pub fn write_text(text: &str, concealed: bool) -> i64 {
    write_with(|c| {
        let set = c.set();
        #[cfg(windows)]
        let set = {
            use arboard::SetExtWindows;
            if concealed {
                set.exclude_from_monitoring()
            } else {
                set
            }
        };
        #[cfg(not(windows))]
        let _ = concealed;
        let _ = set.text(text);
    })
}

pub fn write_files(paths: &[String]) -> i64 {
    write_with(|c| {
        let _ = c.set().file_list(paths);
    })
}

fn write_rgba(img: image::RgbaImage) -> i64 {
    let (w, h) = img.dimensions();
    write_with(|c| {
        let _ = c.set_image(ImageData { width: w as usize, height: h as usize, bytes: Cow::Owned(img.into_raw()) });
    })
}

pub fn write_png(png_bytes: &[u8]) -> i64 {
    match image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png) {
        Ok(img) => write_rgba(img.to_rgba8()),
        Err(_) => change_count(),
    }
}

pub fn write_image_file(path: &str) -> Option<i64> {
    Some(write_rgba(image::open(path).ok()?.to_rgba8()))
}

pub fn image_size(path: &str) -> Option<(i64, i64)> {
    image::image_dimensions(path).ok().map(|(w, h)| (w as i64, h as i64))
}

pub fn clear_clipboard() -> i64 {
    write_with(|c| {
        let _ = c.clear();
    })
}

// ---------- Focus and paste ----------

#[cfg(windows)]
mod focus {
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND};
    use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow};

    /// The window behind the last `frontmost_pid` call, so focus can go back to that exact window.
    static LAST: Mutex<(u32, usize)> = Mutex::new((0, 0));

    fn foreground() -> Option<(u32, HWND)> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        (pid != 0).then_some((pid, hwnd))
    }

    pub fn frontmost_pid() -> Option<i32> {
        let (pid, hwnd) = foreground()?;
        *LAST.lock().unwrap_or_else(|e| e.into_inner()) = (pid, hwnd as usize);
        Some(pid as i32)
    }

    pub fn activate_pid(pid: i32) -> bool {
        let (last_pid, hwnd) = *LAST.lock().unwrap_or_else(|e| e.into_inner());
        last_pid == pid as u32 && hwnd != 0 && unsafe { SetForegroundWindow(hwnd as HWND) } != 0
    }

    pub fn frontmost_app() -> Option<(String, String)> {
        let (pid, _) = foreground()?;
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(h);
            if ok == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            let exe = std::path::Path::new(&path).file_name()?.to_string_lossy().into_owned();
            let name = exe.trim_end_matches(".exe").to_string();
            Some((exe, name))
        }
    }
}

#[cfg(windows)]
pub use focus::{activate_pid, frontmost_app, frontmost_pid};

#[cfg(not(windows))]
pub fn frontmost_app() -> Option<(String, String)> {
    None
}

#[cfg(not(windows))]
pub fn frontmost_pid() -> Option<i32> {
    None
}

#[cfg(not(windows))]
pub fn activate_pid(_pid: i32) -> bool {
    false
}

/// Presses Ctrl+V in the focused app.
pub fn send_paste() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    e.key(Key::Control, Direction::Press).map_err(|e| e.to_string())?;
    let res = e.key(Key::Unicode('v'), Direction::Click).map_err(|e| e.to_string());
    let _ = e.key(Key::Control, Direction::Release);
    res
}

pub fn running_apps() -> Vec<(String, String)> {
    vec![]
}

pub fn accessibility_trusted(_prompt: bool) -> bool {
    true
}

pub fn open_accessibility_settings() {}

pub fn reset_accessibility(_bundle_id: &str) {}

pub fn screen_locked() -> bool {
    false
}

pub fn biometrics_available() -> bool {
    false
}

pub fn biometric_name() -> &'static str {
    if cfg!(windows) {
        "Windows Hello"
    } else {
        "Fingerprint"
    }
}

/// No biometrics here: nothing to carry from the check to the keychain.
pub type AuthContext = ();

pub fn biometric_auth(_reason: &str) -> Result<AuthContext, String> {
    Err("Not supported on this platform yet".into())
}

pub fn open_path(path: &std::path::Path) {
    let opener = if cfg!(windows) { "explorer" } else { "xdg-open" };
    let _ = std::process::Command::new(opener).arg(path).spawn();
}
