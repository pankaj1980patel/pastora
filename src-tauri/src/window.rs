use crate::state::state;
use serde_json::json;
use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

pub const POPUP: &str = "popup";
pub const SETTINGS: &str = "settings";

/// The app that had focus before the popup opened.
static PREVIOUS_PID: AtomicI32 = AtomicI32::new(0);
/// When the popup last hid because it lost focus (ms), so a click on the menu bar icon
/// that caused the blur doesn't immediately reopen it.
static LAST_BLUR_HIDE: AtomicI64 = AtomicI64::new(0);

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
    let _ = w.set_size(LogicalSize::new(s.popup_width as f64, s.popup_height as f64));
    position_popup(app, &w, mode);
    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit_to(POPUP, "popup-shown", json!({ "tab": tab }));
}

pub fn hide_popup(app: &AppHandle) {
    if let Some(w) = popup(app) {
        let _ = w.hide();
    }
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
    let settings_open = app
        .get_webview_window(SETTINGS)
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    let pid = PREVIOUS_PID.load(Ordering::SeqCst);
    if settings_open && pid > 0 && crate::platform::activate_pid(pid) {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = app.hide();
}

fn tray_rect(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let rect = app.tray_by_id("main")?.rect().ok().flatten()?;
    let pos = rect.position.to_physical::<f64>(1.0);
    let size = rect.size.to_physical::<f64>(1.0);
    Some((pos.x, pos.y, size.width, size.height))
}

fn position_popup(app: &AppHandle, w: &WebviewWindow, mode: &str) {
    let Ok(cursor) = app.cursor_position() else { return };
    let tray = if mode == "menubar" { tray_rect(app) } else { None };
    // Use the monitor that holds the menu bar icon (or the pointer).
    let (px, py) = tray.map(|(x, y, _, _)| (x, y)).unwrap_or((cursor.x, cursor.y));
    let monitor = app
        .monitor_from_point(px, py)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(m) = monitor else { return };
    let area = m.work_area();
    let Ok(size) = w.outer_size() else { return };
    let (ax, ay) = (area.position.x as f64, area.position.y as f64);
    let (aw, ah) = (area.size.width as f64, area.size.height as f64);
    let (ww, wh) = (size.width as f64, size.height as f64);
    let (x, y) = if let Some((tx, ty, tw, th)) = tray {
        (tx + tw / 2.0 - ww / 2.0, ty + th + 4.0)
    } else if mode == "center" {
        (ax + (aw - ww) / 2.0, ay + (ah - wh) / 3.0)
    } else {
        (cursor.x - 24.0, cursor.y + 12.0)
    };
    let x = x.clamp(ax + 8.0, (ax + aw - ww - 8.0).max(ax));
    let y = y.clamp(ay + 8.0, (ay + ah - wh - 8.0).max(ay));
    let _ = w.set_position(PhysicalPosition::new(x as i32, y as i32));
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
            let _ = window.hide();
        }
    }
}

pub fn apply_window_settings(app: &AppHandle) {
    let s = state(app).settings();
    if let Some(w) = popup(app) {
        let _ = w.set_content_protected(s.hide_from_screen_capture);
        let _ = w.set_size(LogicalSize::new(s.popup_width as f64, s.popup_height as f64));
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
