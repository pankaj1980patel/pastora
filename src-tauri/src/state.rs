use crate::settings::Settings;
use crate::vault::crypto::Key;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64};
use std::sync::{Mutex, MutexGuard, RwLock};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    OpenPopup,
    OpenPins,
    OpenSecure,
    LockVault,
    TogglePause,
}

/// "Paste username, then password": the password is pasted on the next hotkey press.
pub struct PendingSequence {
    pub entry_id: String,
    pub field_id: String,
    pub created: Instant,
}

pub struct AppState {
    db: Mutex<Connection>,
    settings: RwLock<Settings>,
    pub vault_key: Mutex<Option<Key>>,
    pub vault_activity: Mutex<Instant>,
    pub data_dir: PathBuf,
    pub images_dir: PathBuf,
    /// Change count of the last pasteboard write made by us.
    pub self_change: AtomicI64,
    /// Change count right after we copied a secret (cleared on lock / timer).
    pub secret_change: AtomicI64,
    /// While true, the popup does not hide when it loses focus (e.g. Touch ID sheet).
    pub suppress_blur: AtomicBool,
    pub shortcut_map: Mutex<HashMap<u32, Action>>,
    pub pending_sequence: Mutex<Option<PendingSequence>>,
    pub tray: Mutex<Option<crate::tray::TrayHandles>>,
    /// Newer release found by the update check.
    pub update: Mutex<Option<crate::updates::UpdateInfo>>,
}

impl AppState {
    pub fn new(conn: Connection, settings: Settings, data_dir: PathBuf) -> Self {
        let images_dir = data_dir.join("images");
        Self {
            db: Mutex::new(conn),
            settings: RwLock::new(settings),
            vault_key: Mutex::new(None),
            vault_activity: Mutex::new(Instant::now()),
            data_dir,
            images_dir,
            self_change: AtomicI64::new(-1),
            secret_change: AtomicI64::new(-1),
            suppress_blur: AtomicBool::new(false),
            shortcut_map: Mutex::new(HashMap::new()),
            pending_sequence: Mutex::new(None),
            tray: Mutex::new(None),
            update: Mutex::new(None),
        }
    }

    pub fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn replace_settings(&self, s: Settings) -> Result<(), String> {
        crate::settings::save(&self.db(), &s).map_err(|e| e.to_string())?;
        *self.settings.write().unwrap_or_else(|e| e.into_inner()) = s;
        Ok(())
    }

    pub fn touch_vault(&self) {
        *self.vault_activity.lock().unwrap() = Instant::now();
    }

    /// Runs `f` with the vault key, or fails if the vault is locked.
    pub fn with_key<T>(&self, f: impl FnOnce(&Key) -> Result<T, String>) -> Result<T, String> {
        let guard = self.vault_key.lock().unwrap();
        let key = guard.as_ref().ok_or_else(|| "LOCKED".to_string())?;
        self.touch_vault();
        f(key)
    }
}

pub fn emit(app: &AppHandle, event: &str) {
    let _ = app.emit(event, ());
}

pub fn toast(app: &AppHandle, message: &str) {
    let _ = app.emit("toast", serde_json::json!({ "message": message }));
}

pub fn state(app: &AppHandle) -> tauri::State<'_, AppState> {
    app.state::<AppState>()
}
