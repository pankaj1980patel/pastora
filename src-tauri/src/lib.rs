mod actions;
mod commands;
mod db;
mod history;
mod keychain;
mod pins;
mod platform;
mod settings;
mod shortcuts;
mod state;
mod tray;
mod vault;
mod watcher;
mod window;

use state::AppState;
use tauri::{Manager, RunEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show_popup(app, None);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| shortcuts::handle(app, shortcut, event))
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(data_dir.join("images"))?;
            let conn = db::open(&data_dir.join("clipboard.db"))?;
            let settings = settings::load(&conn);
            let _ = history::prune(&conn, settings.history_limit, settings.history_days, &data_dir.join("images"));
            app.manage(AppState::new(conn, settings.clone(), data_dir));

            window::apply_window_settings(&handle);
            tray::create(&handle)?;
            if let Err(e) = shortcuts::apply(&handle, &settings.shortcuts) {
                log::warn!("shortcuts: {e}");
            }
            // Keep the login item in sync with the saved setting. Re-enabling on every launch
            // rewrites it with the current app path, so it keeps working if the app is moved.
            let al = handle.autolaunch();
            if settings.launch_at_login {
                let _ = al.enable();
            } else if al.is_enabled().unwrap_or(false) {
                let _ = al.disable();
            }

            watcher::spawn(handle.clone());
            actions::spawn_background(handle.clone());

            if !settings.onboarded {
                window::show_popup(&handle, None);
            }
            Ok(())
        })
        .on_window_event(window::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::list_history,
            commands::delete_clip,
            commands::clear_history,
            commands::use_item,
            commands::save_image,
            commands::pin_clip,
            commands::unpin_clip,
            commands::list_pins,
            commands::add_pin,
            commands::update_pin,
            commands::delete_pin,
            commands::reorder_pins,
            commands::vault_status,
            commands::vault_setup,
            commands::vault_unlock,
            commands::vault_unlock_biometric,
            commands::vault_lock,
            commands::vault_set_biometric,
            commands::vault_change_password,
            commands::vault_list,
            commands::vault_get,
            commands::vault_save,
            commands::vault_delete,
            commands::vault_set_favorite,
            commands::vault_reorder,
            commands::vault_copy_field,
            commands::vault_reveal_field,
            commands::vault_totp,
            commands::vault_paste_login,
            commands::vault_export,
            commands::vault_import,
            commands::vault_reset,
            commands::get_settings,
            commands::set_settings,
            commands::reset_shortcuts,
            commands::pause_recording,
            commands::app_info,
            commands::running_apps,
            commands::accessibility_status,
            commands::request_accessibility,
            commands::reset_accessibility,
            commands::open_data_folder,
            commands::open_website,
            commands::open_settings,
            commands::hide_popup,
            commands::export_data,
            commands::import_data,
            commands::reset_all,
            commands::finish_onboarding,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Pastora")
        .run(|_app, event| {
            // Closing the settings window must not quit the menu bar app.
            if let RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
