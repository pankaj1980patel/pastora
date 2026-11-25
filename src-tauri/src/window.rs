use crate::state::state;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
#[cfg(not(target_os = "macos"))]
use tauri::{LogicalSize, PhysicalPosition};

pub const POPUP: &str = "popup";
pub const SETTINGS: &str = "settings";

/// The app that had focus before the popup opened.
static PREVIOUS_PID: AtomicI32 = AtomicI32::new(0);
/// When the popup last hid because it lost focus (ms), so a click on the menu bar icon
/// that caused the blur doesn't immediately reopen it.
static LAST_BLUR_HIDE: AtomicI64 = AtomicI64::new(0);
/// Bumped on every show/hide, so a fade-out that finishes after the popup was
/// re-opened doesn't hide it.
static ANIM_GEN: AtomicU64 = AtomicU64::new(0);
/// A fade-out is running.
static HIDING: AtomicBool = AtomicBool::new(false);

pub fn popup(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(POPUP)
}

pub fn show_popup(app: &AppHandle, tab: Option<&str>) {
    let mode = state(app).settings().popup_position;
    show_popup_at(app, tab, &mode);
}

/// Menu bar icon clicked: drop the popup down below the icon, like a menu.
pub fn toggle_from_tray(app: &AppHandle) {
    let visible = popup(app).map(|w| w.is_visible().unwrap_or(false)).unwrap_or(false);
    let just_hidden = crate::db::now_ms() - LAST_BLUR_HIDE.load(Ordering::SeqCst) < 350;
    if visible {
        hide_popup(app);
    } else if !just_hidden {
        show_popup_at(app, None, "menubar");
    }
}

fn show_popup_at(app: &AppHandle, tab: Option<&str>, mode: &str) {
    let Some(w) = popup(app) else { return };
    let s = state(app).settings();
    let own = std::process::id() as i32;
    if let Some(pid) = crate::platform::frontmost_pid() {
        if pid != own {
            PREVIOUS_PID.store(pid, Ordering::SeqCst);
        }
    }
    ANIM_GEN.fetch_add(1, Ordering::SeqCst);
    HIDING.store(false, Ordering::SeqCst);
    // Reset the list/search before the window is visible, so old content doesn't flash.
    let _ = app.emit_to(POPUP, "popup-shown", json!({ "tab": tab }));
    place_popup(app, &w, mode, s.popup_width as f64, s.popup_height as f64);
    let _ = w.show();
    let _ = w.set_focus();
    #[cfg(target_os = "macos")]
    if let Ok(ns) = w.ns_window() {
        let ns = ns as usize;
        let _ = app.run_on_main_thread(move || unsafe { crate::platform::animate_in(ns as *mut _) });
    }
}

/// Fades the popup out, then hides it.
pub fn hide_popup(app: &AppHandle) {
    let Some(w) = popup(app) else { return };
    if !w.is_visible().unwrap_or(false) || HIDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let gen = ANIM_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let hide = move || {
        if ANIM_GEN.load(Ordering::SeqCst) != gen {
            return false;
        }
        HIDING.store(false, Ordering::SeqCst);
        let _ = w.hide();
        // Lets the UI drop secrets and stop timers while nobody can see it.
        let _ = w.emit_to(POPUP, "popup-hidden", ());
        true
    };
    #[cfg(target_os = "macos")]
    if let Some(ns) = popup(app).and_then(|w| w.ns_window().ok()) {
        let ns = ns as usize;
        let _ = app.run_on_main_thread(move || unsafe { crate::platform::animate_out(ns as *mut _, hide) });
        return;
    }
    hide();
}

pub fn toggle_popup(app: &AppHandle, tab: Option<&str>) {
    match popup(app) {
        Some(w) if w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false) && tab.is_none() => {
            hide_popup(app)
        }
        _ => show_popup(app, tab),
    }
}

/// Hides the popup and returns focus to the previous app (so a paste lands there).
pub fn return_focus(app: &AppHandle) {
    hide_popup(app);
    let pid = PREVIOUS_PID.load(Ordering::SeqCst);
    if pid > 0 && crate::platform::activate_pid(pid) {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = app.hide();
}

/// Blocks until another app is frontmost again (or `timeout` passes), so Cmd+V
/// isn't sent while our own window still has focus.
pub fn wait_for_previous_app(timeout: std::time::Duration) {
    let own = std::process::id() as i32;
    let prev = PREVIOUS_PID.load(Ordering::SeqCst);
    if crate::platform::frontmost_pid().is_none() {
        // Can't tell which app is in front (Linux): give the window manager a moment.
        std::thread::sleep(std::time::Duration::from_millis(150));
        return;
    }
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match crate::platform::frontmost_pid() {
            Some(pid) if pid == prev || (pid != own && pid > 0) => return,
            _ => std::thread::sleep(std::time::Duration::from_millis(15)),
        }
    }
}

/// The menu bar / tray icon's rect, in the physical units Tauri reports
/// (on macOS: points times the backing scale of the icon's screen).
fn tray_rect(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let rect = app.tray_by_id("main")?.rect().ok().flatten()?;
    let pos = rect.position.to_physical::<f64>(1.0);
    let size = rect.size.to_physical::<f64>(1.0);
    Some((pos.x, pos.y, size.width, size.height))
}

/// macOS: Tauri's cursor, monitor and window coordinates each use a different scale factor
/// on mixed-DPI setups, and its setters apply asynchronously. So the popup is placed natively,
/// in points, on the main thread, before it is shown.
#[cfg(target_os = "macos")]
fn place_popup(app: &AppHandle, w: &WebviewWindow, mode: &str, width: f64, height: f64) {
    use crate::platform::Anchor;
    let anchor = match mode {
        "menubar" => match tray_rect(app) {
            Some((x, _, tw, _)) => Anchor::Tray { phys_x: x, phys_w: tw },
            None => Anchor::Cursor,
        },
        "center" => Anchor::Center,
        _ => Anchor::Cursor,
    };
    if let Ok(ns) = w.ns_window() {
        let ns = ns as usize;
        // Runs right away on the main thread, otherwise queued ahead of the show below.
        let _ = app.run_on_main_thread(move || unsafe { crate::platform::place_popup(ns as *mut _, anchor, width, height) });
    }
}

/// Keeps `v` in `[lo, hi]`; if the range is empty (window bigger than the area) uses `lo`.
#[cfg(not(target_os = "macos"))]
fn clamp_in(v: f64, lo: f64, hi: f64) -> f64 {
    if hi < lo { lo } else { v.clamp(lo, hi) }
}

/// Windows / Linux: physical coordinates are global here, so Tauri's monitor APIs line up.
#[cfg(not(target_os = "macos"))]
fn place_popup(app: &AppHandle, w: &WebviewWindow, mode: &str, width: f64, height: f64) {
    use tauri::PhysicalSize;
    let cursor = app.cursor_position().ok();
    let tray = if mode == "menubar" { tray_rect(app) } else { None };
    let probe = tray.map(|(x, y, tw, th)| (x + tw / 2.0, y + th / 2.0)).or(cursor.map(|c| (c.x, c.y)));
    let monitor = probe
        .and_then(|(x, y)| app.monitor_from_point(x, y).ok().flatten())
        .or_else(|| w.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(m) = monitor else {
        let _ = w.set_size(LogicalSize::new(width, height));
        return;
    };
    let scale = m.scale_factor();
    let area = m.work_area();
    let (ax, ay) = (area.position.x as f64, area.position.y as f64);
    let (aw, ah) = (area.size.width as f64, area.size.height as f64);
    // Size for the target monitor's scale, never larger than its work area.
    let ww = (width * scale).min(aw - 16.0).max(1.0).round();
    let wh = (height * scale).min(ah - 16.0).max(1.0).round();
    let (x, y) = if let Some((tx, ty, tw, th)) = tray {
        // Taskbar at the bottom: open above the icon; at the top: below it.
        let y = if ty + th / 2.0 > ay + ah / 2.0 { ty - wh - 4.0 } else { ty + th + 4.0 };
        (tx + tw / 2.0 - ww / 2.0, y)
    } else if let (Some(c), false) = (cursor, mode == "center") {
        (c.x - 24.0 * scale, c.y + 12.0 * scale)
    } else {
        (ax + (aw - ww) / 2.0, ay + (ah - wh) / 3.0)
    };
    let x = clamp_in(x, ax + 8.0, ax + aw - ww - 8.0);
    let y = clamp_in(y, ay + 8.0, ay + ah - wh - 8.0);
    let pos = PhysicalPosition::new(x as i32, y as i32);
    // Move first: crossing to a monitor with another DPI makes Windows rescale the window,
    // so the size for the new monitor is set after the move (and the position re-applied).
    let _ = w.set_position(pos);
    let _ = w.set_size(PhysicalSize::new(ww as u32, wh as u32));
    let _ = w.set_position(pos);
}

/// Puts a newly created window in the middle of the screen the user is working on.
fn center_on_active_screen(app: &AppHandle, w: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    if let Ok(ns) = w.ns_window() {
        let ns = ns as usize;
        let _ = app.run_on_main_thread(move || unsafe { crate::platform::center_on_mouse_screen(ns as *mut _) });
    }
    #[cfg(not(target_os = "macos"))]
    {
        let Ok(c) = app.cursor_position() else { return };
        let Some(m) = app.monitor_from_point(c.x, c.y).ok().flatten() else { return };
        let Ok(size) = w.outer_size() else { return };
        let area = m.work_area();
        let x = area.position.x as f64 + (area.size.width as f64 - size.width as f64).max(0.0) / 2.0;
        let y = area.position.y as f64 + (area.size.height as f64 - size.height as f64).max(0.0) / 2.0;
        let _ = w.set_position(PhysicalPosition::new(x as i32, y as i32));
    }
}

pub fn open_settings(app: &AppHandle, section: Option<&str>) {
    hide_popup(app);
    if let Some(w) = app.get_webview_window(SETTINGS) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    } else {
        let url = match section {
            Some(s) => format!("index.html?section={s}"),
            None => "index.html".to_string(),
        };
        let built = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App(url.into()))
            .title("Pastora Settings")
            .inner_size(820.0, 600.0)
            .min_inner_size(680.0, 460.0)
            .resizable(true)
            .center()
            .focused(true)
            .build();
        if let Ok(w) = built {
            center_on_active_screen(app, &w);
            let _ = w.set_focus();
        }
    }
    if let Some(s) = section {
        let _ = app.emit_to(SETTINGS, "settings-section", s);
    }
}

pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() != POPUP {
        return;
    }
    if let WindowEvent::Focused(false) = event {
        let st = state(window.app_handle());
        if !st.suppress_blur.load(Ordering::SeqCst) {
            LAST_BLUR_HIDE.store(crate::db::now_ms(), Ordering::SeqCst);
            hide_popup(window.app_handle());
        }
    }
}

pub fn apply_window_settings(app: &AppHandle) {
    let s = state(app).settings();
    if let Some(w) = popup(app) {
        let _ = w.set_content_protected(s.hide_from_screen_capture);
        // The size is applied (per screen) each time the popup opens.
        #[cfg(target_os = "macos")]
        if let Ok(ns) = w.ns_window() {
            let ns = ns as usize;
            let _ = app.run_on_main_thread(move || unsafe { crate::platform::configure_popup(ns as *mut _) });
        }
    }
    #[cfg(target_os = "macos")]
    {
        let policy = if s.show_dock_icon {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        let _ = app.set_activation_policy(policy);
    }
}
