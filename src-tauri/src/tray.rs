use crate::state::state;
use crate::{actions, window};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

pub struct TrayHandles {
    pause: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
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
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    *state(app).tray.lock().unwrap() = Some(TrayHandles { pause, autostart });
    Ok(())
}

/// Syncs the check marks with the current settings.
pub fn refresh(app: &AppHandle) {
    let st = state(app);
    let s = st.settings();
    let tray = st.tray.lock().unwrap();
    if let Some(t) = tray.as_ref() {
        let _ = t.pause.set_checked(s.is_paused(crate::db::now_ms()));
        let _ = t.autostart.set_checked(s.launch_at_login);
    }
}
