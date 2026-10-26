//! Tauri commands called by the UI.
use crate::db::now_ms;
use crate::history::{self, Clip};
use crate::pins::{self, Pin};
use crate::settings::{self, Settings};
use crate::state::{emit, state, toast};
use crate::vault::{self, Entry, EntrySummary};
use crate::{actions, keychain, platform, shortcuts, tray, window};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_autostart::ManagerExt;

type Res<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------------- History ----------------

#[tauri::command]
pub fn list_history(app: AppHandle, query: String, limit: i64, offset: i64) -> Res<Vec<Clip>> {
    history::list(&state(&app).db(), query.trim(), limit, offset).map_err(err)
}

#[tauri::command]
pub fn delete_clip(app: AppHandle, id: i64) -> Res<()> {
    let st = state(&app);
    history::delete(&st.db(), id, &st.images_dir).map_err(err)?;
    emit(&app, "history-changed");
    Ok(())
}

#[tauri::command]
pub fn clear_history(app: AppHandle) -> Res<()> {
    let st = state(&app);
    history::clear(&st.db(), &st.images_dir).map_err(err)?;
    emit(&app, "history-changed");
    Ok(())
}

fn put_on_clipboard(app: &AppHandle, kind: &str, content: &str, image_path: Option<&str>) -> Res<()> {
    let st = state(app);
    let cc = match kind {
        "files" => platform::write_files(&content.lines().map(String::from).collect::<Vec<_>>()),
        "image" => {
            let path = image_path.ok_or("missing image")?;
            if path.to_lowercase().ends_with(".png") {
                let bytes = std::fs::read(path).map_err(|_| "Image file is missing")?;
                platform::write_png(&bytes)
            } else {
                platform::write_image_file(path).ok_or("Image file is missing")?
            }
        }
        _ => platform::write_text(content, false),
    };
    st.self_change.store(cc, Ordering::SeqCst);
    Ok(())
}

/// Puts a history item or pin on the clipboard. `paste` also presses Cmd+V in the previous app.
#[tauri::command]
pub fn use_item(app: AppHandle, source: String, id: i64, paste: bool, plain: Option<String>) -> Res<()> {
    let st = state(&app);
    // `plain` lets the UI paste a transformed version (e.g. trimmed / UPPERCASE).
    if let Some(text) = plain {
        actions::write_plain(&app, &text);
    } else if source == "pin" {
        let p = pins::get(&st.db(), id).map_err(err)?.ok_or("Pin not found")?;
        put_on_clipboard(&app, &p.kind, &p.content, p.image_path.as_deref())?;
    } else {
        let c = history::get(&st.db(), id).map_err(err)?.ok_or("Item not found")?;
        put_on_clipboard(&app, &c.kind, &c.content, c.image_path.as_deref())?;
        if st.settings().move_to_top_on_use {
            history::touch(&st.db(), id).map_err(err)?;
            emit(&app, "history-changed");
        }
    }
    actions::deliver(&app, paste && st.settings().auto_paste);
    Ok(())
}

/// Saves an image clip or pin to a file the user picks. Returns false if they cancel.
#[tauri::command]
pub async fn save_image(app: AppHandle, source: String, id: i64, name: String) -> Res<bool> {
    let st = state(&app);
    let (kind, image_path) = if source == "pin" {
        let p = pins::get(&st.db(), id).map_err(err)?.ok_or("Pin not found")?;
        (p.kind, p.image_path)
    } else {
        let c = history::get(&st.db(), id).map_err(err)?.ok_or("Item not found")?;
        (c.kind, c.image_path)
    };
    let src = image_path.filter(|_| kind == "image").ok_or("Not an image")?;
    let ext = std::path::Path::new(&src).extension().and_then(|e| e.to_str()).unwrap_or("png").to_string();

    let mut dialog = app.dialog().file().set_file_name(&name).add_filter("Image", &[ext.as_str()]);
    if let Ok(dir) = app.path().desktop_dir() {
        dialog = dialog.set_directory(dir);
    }
    let picked = with_popup_modal(&app, || dialog.blocking_save_file());

    let Some(dest) = picked.and_then(|p| p.into_path().ok()) else { return Ok(false) };
    std::fs::copy(&src, &dest).map_err(|e| format!("Couldn't save image: {e}"))?;
    toast(&app, "Image saved");
    Ok(true)
}

#[tauri::command]
pub fn pin_clip(app: AppHandle, id: i64) -> Res<i64> {
    let st = state(&app);
    let db = st.db();
    let c = history::get(&db, id).map_err(err)?.ok_or("Item not found")?;
    let pid = pins::add(&db, None, &c.kind, &c.content, c.image_path.as_deref(), (c.width, c.height)).map_err(err)?;
    drop(db);
    emit(&app, "pins-changed");
    emit(&app, "history-changed");
    Ok(pid)
}

#[tauri::command]
pub fn unpin_clip(app: AppHandle, id: i64) -> Res<()> {
    let st = state(&app);
    let db = st.db();
    let c = history::get(&db, id).map_err(err)?.ok_or("Item not found")?;
    pins::delete_matching(&db, &c.kind, &c.content).map_err(err)?;
    drop(db);
    emit(&app, "pins-changed");
    emit(&app, "history-changed");
    Ok(())
}

// ---------------- Pins ----------------

#[tauri::command]
pub fn list_pins(app: AppHandle, query: String) -> Res<Vec<Pin>> {
    pins::list(&state(&app).db(), query.trim()).map_err(err)
}

#[tauri::command]
pub fn add_pin(app: AppHandle, label: Option<String>, content: String) -> Res<i64> {
    if content.trim().is_empty() {
        return Err("Content is empty".into());
    }
    let id = pins::add(&state(&app).db(), label.as_deref().filter(|l| !l.trim().is_empty()), "text", &content, None, (None, None))
        .map_err(err)?;
    emit(&app, "pins-changed");
    emit(&app, "history-changed");
    Ok(id)
}

#[tauri::command]
pub fn update_pin(app: AppHandle, id: i64, label: Option<String>, content: Option<String>) -> Res<()> {
    pins::update(&state(&app).db(), id, label.as_deref(), content.as_deref()).map_err(err)?;
    emit(&app, "pins-changed");
    emit(&app, "history-changed");
    Ok(())
}

#[tauri::command]
pub fn delete_pin(app: AppHandle, id: i64) -> Res<()> {
    let st = state(&app);
    pins::delete(&st.db(), id).map_err(err)?;
    history::remove_orphan_images(&st.db(), &st.images_dir).map_err(err)?;
    emit(&app, "pins-changed");
    emit(&app, "history-changed");
    Ok(())
}

#[tauri::command]
pub fn reorder_pins(app: AppHandle, ids: Vec<i64>) -> Res<()> {
    pins::reorder(&mut state(&app).db(), &ids).map_err(err)?;
    emit(&app, "pins-changed");
    Ok(())
}

// ---------------- Secure Clips ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    initialized: bool,
    unlocked: bool,
    biometric_available: bool,
    biometric_name: &'static str,
    biometric_enabled: bool,
    /// True when biometrics can't be used because the periodic password check is due.
    password_required: bool,
}

const LAST_PASSWORD_UNLOCK: &str = "vault.lastPasswordUnlock";

fn password_due(app: &AppHandle) -> bool {
    let st = state(app);
    let days = st.settings().require_password_days;
    if days == 0 {
        return false;
    }
    let last = settings::get_meta(&st.db(), LAST_PASSWORD_UNLOCK).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0);
    now_ms() - last > days as i64 * 86_400_000
}

fn mark_password_unlock(app: &AppHandle) {
    let _ = settings::set_meta(&state(app).db(), LAST_PASSWORD_UNLOCK, &now_ms().to_string());
}

#[tauri::command]
pub fn vault_status(app: AppHandle) -> VaultStatus {
    let st = state(&app);
    let s = st.settings();
    let initialized = vault::is_initialized(&st.db());
    let unlocked = st.vault_key.lock().unwrap().is_some();
    VaultStatus {
        initialized,
        unlocked,
        biometric_available: platform::biometrics_available(),
        biometric_name: platform::biometric_name(),
        biometric_enabled: s.touch_id_enabled,
        password_required: password_due(&app),
    }
}

#[tauri::command]
pub async fn vault_setup(app: AppHandle, password: String) -> Res<()> {
    let st = state(&app);
    let key = vault::setup(&st.db(), &password)?;
    *st.vault_key.lock().unwrap() = Some(key);
    st.touch_vault();
    mark_password_unlock(&app);
    emit(&app, "vault-changed");
    Ok(())
}

#[tauri::command]
pub async fn vault_unlock(app: AppHandle, password: String) -> Res<()> {
    let st = state(&app);
    let key = vault::unlock(&st.db(), &password)?;
    *st.vault_key.lock().unwrap() = Some(key);
    st.touch_vault();
    mark_password_unlock(&app);
    emit(&app, "vault-changed");
    Ok(())
}

/// Runs system UI (Touch ID, save panel) over the popup: the popup doesn't hide itself when
/// that UI takes focus, and drops its always-on-top level so it can't cover it.
fn with_popup_modal<T>(app: &AppHandle, f: impl FnOnce() -> T) -> T {
    let st = state(app);
    let popup = window::popup(app);
    st.suppress_blur.store(true, Ordering::SeqCst);
    if let Some(w) = &popup {
        let _ = w.set_always_on_top(false);
    }
    let res = f();
    st.suppress_blur.store(false, Ordering::SeqCst);
    if let Some(w) = &popup {
        let _ = w.set_always_on_top(true);
        if w.is_visible().unwrap_or(false) {
            let _ = w.set_focus();
        }
    }
    res
}

/// A Touch ID prompt is showing. A second one would cancel the first.
static BIO_BUSY: AtomicBool = AtomicBool::new(false);

fn biometric_prompt(app: &AppHandle, reason: &str) -> Res<()> {
    if BIO_BUSY.swap(true, Ordering::SeqCst) {
        return Err("cancelled".into());
    }
    let res = with_popup_modal(app, || platform::biometric_auth(reason));
    BIO_BUSY.store(false, Ordering::SeqCst);
    res
}

#[tauri::command]
pub async fn vault_unlock_biometric(app: AppHandle) -> Res<()> {
    let st = state(&app);
    if !st.settings().touch_id_enabled {
        return Err("Touch ID is not turned on".into());
    }
    if password_due(&app) {
        return Err("Enter your password (required periodically)".into());
    }
    biometric_prompt(&app, "unlock Secure Clips")?;
    let key = keychain::load().map_err(|e| format!("Could not read the key from Keychain: {e}"))?;
    // Make sure the stored key really is this vault's key.
    vault::list(&st.db(), &key).map_err(|_| "Stored key does not match. Unlock with your password.")?;
    *st.vault_key.lock().unwrap() = Some(key);
    st.touch_vault();
    emit(&app, "vault-changed");
    Ok(())
}

#[tauri::command]
pub fn vault_lock(app: AppHandle) {
    actions::lock_vault(&app);
}

#[tauri::command]
pub async fn vault_set_biometric(app: AppHandle, enabled: bool) -> Res<()> {
    let st = state(&app);
    if enabled {
        if !platform::biometrics_available() {
            return Err(format!("{} is not available on this Mac", platform::biometric_name()));
        }
        let key = st.with_key(|k| Ok(k.clone()))?;
        biometric_prompt(&app, "turn on Touch ID for Secure Clips")?;
        keychain::store(&key)?;
    } else {
        keychain::delete();
    }
    let mut s = st.settings();
    s.touch_id_enabled = enabled;
    st.replace_settings(s.clone())?;
    let _ = app.emit("settings-changed", &s);
    emit(&app, "vault-changed");
    Ok(())
}

#[tauri::command]
pub async fn vault_change_password(app: AppHandle, old: String, new: String) -> Res<()> {
    let st = state(&app);
    let key = vault::change_password(&st.db(), &old, &new)?;
    *st.vault_key.lock().unwrap() = Some(key);
    st.touch_vault();
    mark_password_unlock(&app);
    toast(&app, "Master password changed");
    Ok(())
}

#[tauri::command]
pub fn vault_list(app: AppHandle) -> Res<Vec<EntrySummary>> {
    let st = state(&app);
    st.with_key(|k| Ok(vault::list(&st.db(), k)?.iter().map(vault::summarize).collect()))
}

/// Full entry including secret values, for the editor.
#[tauri::command]
pub fn vault_get(app: AppHandle, id: String) -> Res<Entry> {
    let st = state(&app);
    st.with_key(|k| vault::get(&st.db(), k, &id))
}

#[tauri::command]
pub fn vault_save(app: AppHandle, entry: Entry) -> Res<String> {
    let st = state(&app);
    let id = st.with_key(|k| vault::save(&st.db(), k, entry))?;
    emit(&app, "vault-changed");
    Ok(id)
}

#[tauri::command]
pub fn vault_delete(app: AppHandle, id: String) -> Res<()> {
    let st = state(&app);
    st.with_key(|_| vault::delete(&st.db(), &id))?;
    emit(&app, "vault-changed");
    Ok(())
}

#[tauri::command]
pub fn vault_set_favorite(app: AppHandle, id: String, favorite: bool) -> Res<()> {
    let st = state(&app);
    st.with_key(|_| vault::set_favorite(&st.db(), &id, favorite))?;
    emit(&app, "vault-changed");
    Ok(())
}

#[tauri::command]
pub fn vault_reorder(app: AppHandle, ids: Vec<String>) -> Res<()> {
    let st = state(&app);
    st.with_key(|_| vault::reorder(&mut st.db(), &ids))?;
    emit(&app, "vault-changed");
    Ok(())
}

fn field_value(app: &AppHandle, id: &str, field_id: &str) -> Res<(String, String)> {
    let st = state(app);
    st.with_key(|k| {
        let e = vault::get(&st.db(), k, id)?;
        let f = e.payload.fields.iter().find(|f| f.id == field_id).ok_or("Field not found")?;
        let value = if f.kind == "totp" {
            let secret = vault::totp::parse_secret(&f.value).ok_or("Invalid one-time code secret")?;
            vault::totp::code(&secret, (now_ms() / 1000) as u64).0
        } else {
            f.value.clone()
        };
        Ok((value, f.kind.clone()))
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyResult {
    clear_seconds: u32,
}

/// Copies one field. Secret fields are concealed and auto-cleared; others are copied normally.
#[tauri::command]
pub fn vault_copy_field(app: AppHandle, id: String, field_id: String, paste: bool) -> Res<CopyResult> {
    let (value, kind) = field_value(&app, &id, &field_id)?;
    let clear_seconds = if matches!(kind.as_str(), "secret" | "totp" | "note") {
        actions::write_secret(&app, &value)
    } else {
        // Usernames, URLs etc.: normal copy, but marked as ours so they stay out of history.
        actions::write_plain(&app, &value);
        0
    };
    if paste {
        actions::deliver(&app, state(&app).settings().auto_paste);
    }
    Ok(CopyResult { clear_seconds })
}

#[tauri::command]
pub fn vault_reveal_field(app: AppHandle, id: String, field_id: String) -> Res<String> {
    Ok(field_value(&app, &id, &field_id)?.0)
}

#[derive(Serialize)]
pub struct Totp {
    code: String,
    remaining: u64,
}

#[tauri::command]
pub fn vault_totp(app: AppHandle, id: String, field_id: String) -> Res<Totp> {
    let st = state(&app);
    st.with_key(|k| {
        let e = vault::get(&st.db(), k, &id)?;
        let f = e.payload.fields.iter().find(|f| f.id == field_id).ok_or("Field not found")?;
        let secret = vault::totp::parse_secret(&f.value).ok_or("Invalid one-time code secret")?;
        let (code, remaining) = vault::totp::code(&secret, (now_ms() / 1000) as u64);
        Ok(Totp { code, remaining })
    })
}

#[tauri::command]
pub fn vault_paste_login(app: AppHandle, id: String, user_field: String, pass_field: String) -> Res<()> {
    actions::start_sequence(&app, &id, &user_field, &pass_field)
}

#[tauri::command]
pub fn vault_export(app: AppHandle, path: String) -> Res<()> {
    let st = state(&app);
    let backup = vault::export(&st.db())?;
    std::fs::write(&path, serde_json::to_vec_pretty(&backup).map_err(err)?).map_err(err)
}

#[tauri::command]
pub async fn vault_import(app: AppHandle, path: String, password: String) -> Res<usize> {
    let st = state(&app);
    let raw = std::fs::read(&path).map_err(err)?;
    let backup: vault::Backup = serde_json::from_slice(&raw).map_err(|_| "This is not a Pastora vault backup")?;
    let n = st.with_key(|k| vault::import(&st.db(), k, &backup, &password))?;
    emit(&app, "vault-changed");
    Ok(n)
}

/// Deletes Secure Clips entirely (e.g. forgotten master password).
#[tauri::command]
pub fn vault_reset(app: AppHandle) -> Res<()> {
    let st = state(&app);
    actions::lock_vault(&app);
    vault::reset(&st.db())?;
    keychain::delete();
    let mut s = st.settings();
    s.touch_id_enabled = false;
    st.replace_settings(s.clone())?;
    let _ = app.emit("settings-changed", &s);
    emit(&app, "vault-changed");
    Ok(())
}

// ---------------- Settings ----------------

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    state(&app).settings()
}

/// Saves settings and applies every side effect. Shortcut errors are reported and nothing changes.
pub fn apply_settings(app: &AppHandle, mut new: Settings) -> Res<Settings> {
    let st = state(app);
    let old = st.settings();
    new.touch_id_enabled = old.touch_id_enabled; // only changed through vault_set_biometric

    if new.shortcuts != old.shortcuts {
        if let Err(e) = shortcuts::apply(app, &new.shortcuts) {
            let _ = shortcuts::apply(app, &old.shortcuts);
            return Err(e);
        }
    }
    if new.launch_at_login != old.launch_at_login {
        let al = app.autolaunch();
        let r = if new.launch_at_login { al.enable() } else { al.disable() };
        if let Err(e) = r {
            log::warn!("autostart: {e}");
        }
    }
    st.replace_settings(new.clone())?;

    if new.history_limit != old.history_limit || new.history_days != old.history_days {
        history::prune(&st.db(), new.history_limit, new.history_days, &st.images_dir).map_err(err)?;
        emit(app, "history-changed");
    }
    window::apply_window_settings(app);
    tray::refresh(app);
    let _ = app.emit("settings-changed", &new);
    Ok(new)
}

#[tauri::command]
pub fn set_settings(app: AppHandle, settings: Settings) -> Res<Settings> {
    apply_settings(&app, settings)
}

#[tauri::command]
pub fn reset_shortcuts(app: AppHandle) -> Res<Settings> {
    let mut s = state(&app).settings();
    s.shortcuts = Default::default();
    apply_settings(&app, s)
}

/// minutes: 0 = resume, -1 = until resumed.
#[tauri::command]
pub fn pause_recording(app: AppHandle, minutes: i64) {
    let until = match minutes {
        0 => None,
        m if m < 0 => Some(i64::MAX),
        m => Some(now_ms() + m * 60_000),
    };
    actions::set_paused(&app, until);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    data_dir: String,
    platform: &'static str,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state(&app).data_dir.to_string_lossy().to_string(),
        platform: std::env::consts::OS,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningApp {
    bundle_id: String,
    name: String,
}

#[tauri::command]
pub fn running_apps() -> Vec<RunningApp> {
    platform::running_apps().into_iter().map(|(bundle_id, name)| RunningApp { bundle_id, name }).collect()
}

#[tauri::command]
pub fn accessibility_status() -> bool {
    platform::accessibility_trusted(false)
}

#[tauri::command]
pub fn reset_accessibility(app: AppHandle) -> bool {
    platform::reset_accessibility(&app.config().identifier);
    request_accessibility()
}

#[tauri::command]
pub fn request_accessibility() -> bool {
    let ok = platform::accessibility_trusted(true);
    if !ok {
        platform::open_accessibility_settings();
    }
    ok
}

#[tauri::command]
pub fn open_data_folder(app: AppHandle) {
    platform::open_path(&state(&app).data_dir);
}

#[tauri::command]
pub fn open_website(page: Option<String>) {
    let url = match page.as_deref() {
        Some("support") => "https://ko-fi.com/D4H729IALL",
        _ => "https://github.com/pankaj1980patel/pastora",
    };
    platform::open_path(std::path::Path::new(url));
}

#[tauri::command]
pub fn open_settings(app: AppHandle, section: Option<String>) {
    window::open_settings(&app, section.as_deref());
}

#[tauri::command]
pub fn hide_popup(app: AppHandle) {
    window::return_focus(&app);
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportedPin {
    label: Option<String>,
    kind: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportFile {
    format: String,
    settings: Settings,
    pins: Vec<ExportedPin>,
}

/// Exports settings and text/file pins (not history, not Secure Clips).
#[tauri::command]
pub fn export_data(app: AppHandle, path: String) -> Res<()> {
    let st = state(&app);
    let pins = pins::list(&st.db(), "")
        .map_err(err)?
        .into_iter()
        .filter(|p| p.kind != "image")
        .map(|p| ExportedPin { label: p.label, kind: p.kind, content: p.content })
        .collect();
    let f = ExportFile { format: "pastora-data-v1".into(), settings: st.settings(), pins };
    std::fs::write(&path, serde_json::to_vec_pretty(&f).map_err(err)?).map_err(err)
}

#[tauri::command]
pub fn import_data(app: AppHandle, path: String) -> Res<usize> {
    let st = state(&app);
    let raw = std::fs::read(&path).map_err(err)?;
    let f: ExportFile = serde_json::from_slice(&raw).map_err(|_| "This is not a Pastora export file")?;
    let mut n = 0;
    for p in f.pins.iter().rev() {
        pins::add(&st.db(), p.label.as_deref(), &p.kind, &p.content, None, (None, None)).map_err(err)?;
        n += 1;
    }
    let mut s = f.settings;
    s.onboarded = true;
    apply_settings(&app, s)?;
    emit(&app, "pins-changed");
    emit(&app, "history-changed");
    Ok(n)
}

/// Erases all data (history, pins, Secure Clips, settings) and restarts.
#[tauri::command]
pub fn reset_all(app: AppHandle) {
    let st = state(&app);
    actions::lock_vault(&app);
    {
        let db = st.db();
        let _ = db.execute_batch(
            "DELETE FROM clips; DELETE FROM pins; DELETE FROM vault_entries; DELETE FROM vault_meta; DELETE FROM settings;",
        );
    }
    keychain::delete();
    let _ = std::fs::remove_dir_all(&st.images_dir);
    let _ = app.autolaunch().disable();
    app.restart();
}

#[tauri::command]
pub fn finish_onboarding(app: AppHandle) -> Res<Settings> {
    let mut s = state(&app).settings();
    s.onboarded = true;
    apply_settings(&app, s)
}
