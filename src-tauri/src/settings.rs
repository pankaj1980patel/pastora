use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Shortcuts {
    pub open_popup: String,
    pub open_pins: String,
    pub open_secure: String,
    pub lock_vault: String,
    pub toggle_pause: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            open_popup: "CommandOrControl+Shift+V".into(),
            open_pins: "CommandOrControl+Shift+P".into(),
            open_secure: "CommandOrControl+Shift+S".into(),
            lock_vault: "CommandOrControl+Shift+L".into(),
            toggle_pause: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IgnoredApp {
    pub bundle_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    // General
    pub launch_at_login: bool,
    pub show_dock_icon: bool,
    pub theme: String,          // system | light | dark
    pub popup_position: String, // menubar | cursor | center

    // History
    pub history_limit: u32, // 0 = unlimited
    pub history_days: u32,  // 0 = keep forever
    pub store_images: bool,
    pub max_image_mb: u32,
    pub ignore_password_managers: bool,
    pub ignored_apps: Vec<IgnoredApp>,
    pub ignore_patterns: Vec<String>,
    pub paused_until: Option<i64>, // ms timestamp; i64::MAX = until resumed

    // Shortcuts
    pub shortcuts: Shortcuts,

    // Paste
    pub auto_paste: bool,
    pub move_to_top_on_use: bool,

    // Secure Clips
    pub auto_lock_minutes: u32, // 0 = never
    pub lock_on_sleep: bool,
    pub clear_clipboard_seconds: u32, // 0 = never
    pub touch_id_enabled: bool,
    pub require_password_days: u32, // 0 = never
    pub hide_from_screen_capture: bool,

    // Appearance
    pub popup_width: u32,
    pub popup_height: u32,
    pub font_size: u32,
    pub show_preview: bool,
    pub show_source_app: bool,
    pub relative_time: bool,

    pub onboarded: bool,
    /// Check GitHub once a day for a newer release.
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            launch_at_login: true,
            show_dock_icon: false,
            theme: "system".into(),
            popup_position: "menubar".into(),
            history_limit: 500,
            history_days: 30,
            store_images: true,
            max_image_mb: 10,
            ignore_password_managers: true,
            ignored_apps: vec![
                IgnoredApp { bundle_id: "com.1password.1password".into(), name: "1Password".into() },
                IgnoredApp { bundle_id: "com.bitwarden.desktop".into(), name: "Bitwarden".into() },
                IgnoredApp { bundle_id: "org.keepassxc.keepassxc".into(), name: "KeePassXC".into() },
                IgnoredApp { bundle_id: "com.apple.keychainaccess".into(), name: "Keychain Access".into() },
                IgnoredApp { bundle_id: "com.apple.Passwords".into(), name: "Passwords".into() },
            ],
            ignore_patterns: vec![],
            paused_until: None,
            shortcuts: Shortcuts::default(),
            auto_paste: true,
            move_to_top_on_use: true,
            auto_lock_minutes: 5,
            lock_on_sleep: true,
            clear_clipboard_seconds: 30,
            touch_id_enabled: false,
            require_password_days: 7,
            hide_from_screen_capture: true,
            popup_width: 440,
            popup_height: 560,
            font_size: 13,
            show_preview: true,
            show_source_app: true,
            relative_time: true,
            onboarded: false,
            check_updates: true,
        }
    }
}

impl Settings {
    pub fn is_paused(&self, now: i64) -> bool {
        matches!(self.paused_until, Some(t) if t > now)
    }
}

const KEY: &str = "app";

pub fn load(conn: &Connection) -> Settings {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [KEY], |r| r.get(0))
        .optional()
        .ok()
        .flatten();
    raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(conn: &Connection, s: &Settings) -> rusqlite::Result<()> {
    let json = serde_json::to_string(s).expect("settings serialize");
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![KEY, json],
    )?;
    Ok(())
}

/// Small key/value values that are not user settings (e.g. last password unlock).
pub fn get_meta(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_defaults_for_missing_fields() {
        let conn = crate::db::memory();
        assert_eq!(load(&conn), Settings::default());
        let mut s = Settings::default();
        s.history_limit = 42;
        s.shortcuts.open_popup = "Alt+Space".into();
        save(&conn, &s).unwrap();
        assert_eq!(load(&conn), s);
        // Older JSON without newer fields still loads.
        conn.execute("UPDATE settings SET value = '{\"historyLimit\":7}' WHERE key='app'", []).unwrap();
        let l = load(&conn);
        assert_eq!(l.history_limit, 7);
        assert_eq!(l.auto_lock_minutes, 5);
    }
}
