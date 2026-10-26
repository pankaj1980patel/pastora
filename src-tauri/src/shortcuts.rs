use crate::settings::Shortcuts;
use crate::state::{state, Action};
use crate::{actions, window};
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

pub fn handle(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != ShortcutState::Pressed {
        return;
    }
    let action = state(app).shortcut_map.lock().unwrap().get(&shortcut.id()).copied();
    match action {
        Some(Action::OpenPopup) => {
            if !actions::continue_sequence(app) {
                window::toggle_popup(app, None);
            }
        }
        Some(Action::OpenPins) => window::show_popup(app, Some("pins")),
        Some(Action::OpenSecure) => window::show_popup(app, Some("secure")),
        Some(Action::LockVault) => actions::lock_vault(app),
        Some(Action::TogglePause) => actions::toggle_pause(app),
        None => {}
    }
}

/// Registers all global shortcuts. On error nothing is left half-registered.
pub fn apply(app: &AppHandle, sc: &Shortcuts) -> Result<(), String> {
    let pairs = [
        (&sc.open_popup, Action::OpenPopup),
        (&sc.open_pins, Action::OpenPins),
        (&sc.open_secure, Action::OpenSecure),
        (&sc.lock_vault, Action::LockVault),
        (&sc.toggle_pause, Action::TogglePause),
    ];
    let mut parsed = vec![];
    for (accel, action) in pairs {
        if accel.trim().is_empty() {
            continue;
        }
        let s: Shortcut = accel.parse().map_err(|e| format!("“{accel}” is not a valid shortcut: {e}"))?;
        if parsed.iter().any(|(p, _): &(Shortcut, Action)| p.id() == s.id()) {
            return Err(format!("“{accel}” is used for two actions"));
        }
        parsed.push((s, action));
    }

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut map = HashMap::new();
    for (s, action) in &parsed {
        if let Err(e) = gs.register(*s) {
            let _ = gs.unregister_all();
            return Err(format!("Could not register {}: {e}. Another app may already use it.", s.into_string()));
        }
        map.insert(s.id(), *action);
    }
    *state(app).shortcut_map.lock().unwrap() = map;
    Ok(())
}
