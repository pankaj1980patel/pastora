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

#[cfg(windows)]
fn written() -> i64 {
    change_count()
}

/// Linux has no clipboard sequence number: bump a counter whenever the content changes.
#[cfg(not(windows))]
mod counter {
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::sync::Mutex;

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

    pub fn current() -> i64 {
        let fp = fingerprint();
        let mut s = STATE.lock().unwrap();
        if fp != s.1 {
            *s = (s.0 + 1, fp);
        }
        s.0
    }

    /// After our own write: record the new content so the watcher sees it as ours.
    pub fn written() -> i64 {
        let fp = fingerprint();
        let mut s = STATE.lock().unwrap();
        *s = (s.0 + 1, fp);
        s.0
    }
}

#[cfg(not(windows))]
pub fn change_count() -> i64 {
    counter::current()
}

#[cfg(not(windows))]
fn written() -> i64 {
    counter::written()
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
    with_clip(|c| {
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
        set.text(text)
    });
    written()
}

pub fn write_files(paths: &[String]) -> i64 {
    with_clip(|c| c.set().file_list(paths));
    written()
}

fn write_rgba(img: image::RgbaImage) -> i64 {
    let (w, h) = img.dimensions();
    with_clip(|c| c.set_image(ImageData { width: w as usize, height: h as usize, bytes: Cow::Owned(img.into_raw()) }));
    written()
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
    with_clip(|c| c.clear());
    written()
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
        *LAST.lock().unwrap() = (pid, hwnd as usize);
        Some(pid as i32)
    }

    pub fn activate_pid(pid: i32) -> bool {
        let (last_pid, hwnd) = *LAST.lock().unwrap();
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

pub fn biometric_auth(_reason: &str) -> Result<(), String> {
    Err("Not supported on this platform yet".into())
}

pub fn open_path(path: &std::path::Path) {
    let opener = if cfg!(windows) { "explorer" } else { "xdg-open" };
    let _ = std::process::Command::new(opener).arg(path).spawn();
}
