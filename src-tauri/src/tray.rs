use crate::state::state;
use crate::{actions, window};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

pub struct TrayHandles {
    menu: Menu<Wry>,
    pause: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
    /// "Allow Accessibility…" (macOS, only while paste isn't allowed).
    access: MenuItem<Wry>,
    /// "Update available…" (only while a newer release exists).
    update: MenuItem<Wry>,
    shown_access: bool,
    shown_update: bool,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let s = state(app).settings();
    let open = MenuItem::with_id(app, "open", "Open Clipboard History", true, None::<&str>)?;
    let pins = MenuItem::with_id(app, "pins", "Pins", true, None::<&str>)?;
    let secure = MenuItem::with_id(app, "secure", "Secure Clips", true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, "pause", "Pause Recording", true, s.is_paused(crate::db::now_ms()), None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", "Lock Secure Clips", true, None::<&str>)?;
    let clear = MenuItem::with_id(app, "clear", "Clear History", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let autostart = CheckMenuItem::with_id(app, "autostart", "Start at Login", true, s.launch_at_login, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Pastora", true, Some("CmdOrCtrl+Q"))?;
    let access = MenuItem::with_id(app, "access", "Allow Accessibility for Paste…", true, None::<&str>)?;
    let update = MenuItem::with_id(app, "update", "Update Available…", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &pins,
            &secure,
            &PredefinedMenuItem::separator(app)?,
            &pause,
            &clear,
            &lock,
            &PredefinedMenuItem::separator(app)?,
            &settings,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(include_bytes!("../icons/tray@2x.png"))?)
        .icon_as_template(true)
        .tooltip("Pastora")
        .menu(&menu)
        // Left click drops down the clipboard popup (like CopyClip); right click shows this menu.
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                window::toggle_from_tray(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => window::show_popup(app, Some("history")),
            "pins" => window::show_popup(app, Some("pins")),
            "secure" => window::show_popup(app, Some("secure")),
            "pause" => actions::toggle_pause(app),
            "lock" => actions::lock_vault(app),
            "clear" => {
                let st = state(app);
                let _ = crate::history::clear(&st.db(), &st.images_dir);
                crate::state::emit(app, "history-changed");
            }
            "settings" => window::open_settings(app, None),
            "autostart" => {
                let mut s = state(app).settings();
                s.launch_at_login = !s.launch_at_login;
                let _ = crate::commands::apply_settings(app, s);
            }
            "access" => {
                crate::platform::accessibility_trusted(true);
                crate::platform::open_accessibility_settings();
            }
            "update" => {
                if let Some(u) = state(app).update.lock().unwrap().clone() {
                    crate::platform::open_path(std::path::Path::new(&u.url));
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    *state(app).tray.lock().unwrap() =
        Some(TrayHandles { menu, pause, autostart, access, update, shown_access: false, shown_update: false });
    refresh(app);
    Ok(())
}

/// Syncs the check marks with the current settings, and shows the "Allow Accessibility"
/// and "Update available" items only when they apply.
pub fn refresh(app: &AppHandle) {
    let st = state(app);
    let s = st.settings();
    let update = st.update.lock().unwrap().clone();
    let need_access = cfg!(target_os = "macos") && s.auto_paste && !crate::platform::accessibility_trusted(false);
    let mut tray = st.tray.lock().unwrap();
    if let Some(t) = tray.as_mut() {
        let _ = t.pause.set_checked(s.is_paused(crate::db::now_ms()));
        let _ = t.autostart.set_checked(s.launch_at_login);
        if let Some(u) = &update {
            let _ = t.update.set_text(format!("Update Available: Pastora {}…", u.version));
        }
        // Both optional items sit at the top of the menu: update first, then accessibility.
        if t.shown_access != need_access {
            let _ = if need_access { t.menu.insert(&t.access, if t.shown_update { 1 } else { 0 }) } else { t.menu.remove(&t.access) };
            t.shown_access = need_access;
        }
        if t.shown_update != update.is_some() {
            let _ = if update.is_some() { t.menu.prepend(&t.update) } else { t.menu.remove(&t.update) };
            t.shown_update = update.is_some();
        }
    }
}
