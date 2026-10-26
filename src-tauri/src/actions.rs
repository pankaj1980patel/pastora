use crate::db::now_ms;
use crate::platform;
use crate::state::{emit, state, toast, PendingSequence};
use crate::{tray, vault, window};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter};

pub fn lock_vault(app: &AppHandle) {
    let st = state(app);
    let was_unlocked = st.vault_key.lock().unwrap().take().is_some();
    *st.pending_sequence.lock().unwrap() = None;
    // Don't leave a copied secret on the clipboard once the vault is locked.
    let sc = st.secret_change.load(Ordering::SeqCst);
    if sc >= 0 && platform::change_count() == sc {
        st.self_change.store(platform::clear_clipboard(), Ordering::SeqCst);
    }
    if was_unlocked {
        emit(app, "vault-changed");
    }
}

pub fn set_paused(app: &AppHandle, until: Option<i64>) {
    let st = state(app);
    let mut s = st.settings();
    s.paused_until = until;
    if st.replace_settings(s.clone()).is_ok() {
        tray::refresh(app);
        let _ = app.emit("settings-changed", &s);
    }
}

pub fn toggle_pause(app: &AppHandle) {
    let paused = state(app).settings().is_paused(now_ms());
    set_paused(app, if paused { None } else { Some(i64::MAX) });
    toast(app, if paused { "Recording resumed" } else { "Recording paused" });
}

/// Writes plain (non-secret) text and records the change as our own.
pub fn write_plain(app: &AppHandle, text: &str) {
    let cc = platform::write_text(text, false);
    state(app).self_change.store(cc, Ordering::SeqCst);
}

/// Copies a secret: hidden from clipboard managers and cleared after the configured delay.
/// Returns the number of seconds until it is cleared (0 = never).
pub fn write_secret(app: &AppHandle, text: &str) -> u32 {
    let st = state(app);
    let cc = platform::write_text(text, true);
    st.self_change.store(cc, Ordering::SeqCst);
    st.secret_change.store(cc, Ordering::SeqCst);
    let secs = st.settings().clear_clipboard_seconds;
    if secs > 0 {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(secs as u64));
            if platform::change_count() == cc {
                let st = state(&app);
                st.self_change.store(platform::clear_clipboard(), Ordering::SeqCst);
                let _ = app.emit("clipboard-cleared", ());
            }
        });
    }
    secs
}

/// After the clipboard is set: hide the popup, give focus back and (optionally) press Cmd+V.
pub fn deliver(app: &AppHandle, paste: bool) {
    // Without Accessibility we can't press Cmd+V. Keep the popup open so the user
    // actually sees why, and let macOS show its permission prompt.
    if paste && !platform::accessibility_trusted(false) {
        toast(app, "Copied. Allow Accessibility in Settings to paste automatically.");
        platform::accessibility_trusted(true);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        window::return_focus(&app);
        if !paste {
            return;
        }
        window::wait_for_previous_app(Duration::from_millis(600));
        std::thread::sleep(Duration::from_millis(50));
        if let Err(e) = platform::send_paste() {
            log::warn!("paste failed: {e}");
        }
    });
}

/// Pastes the username of a login now and queues the password for the next hotkey press.
pub fn start_sequence(app: &AppHandle, entry_id: &str, user_field: &str, pass_field: &str) -> Result<(), String> {
    let st = state(app);
    let user = st.with_key(|k| {
        let e = vault::get(&st.db(), k, entry_id)?;
        e.payload.fields.iter().find(|f| f.id == user_field).map(|f| f.value.clone()).ok_or("Field not found".into())
    })?;
    write_plain(app, &user);
    *st.pending_sequence.lock().unwrap() =
        Some(PendingSequence { entry_id: entry_id.into(), field_id: pass_field.into(), created: Instant::now() });
    deliver(app, true);
    Ok(())
}

/// Called on the open-popup hotkey. Returns true if it pasted a queued password instead.
pub fn continue_sequence(app: &AppHandle) -> bool {
    let st = state(app);
    let Some(seq) = st.pending_sequence.lock().unwrap().take() else { return false };
    if seq.created.elapsed() > Duration::from_secs(120) {
        return false;
    }
    let secret = st.with_key(|k| {
        let e = vault::get(&st.db(), k, &seq.entry_id)?;
        e.payload.fields.iter().find(|f| f.id == seq.field_id).map(|f| f.value.clone()).ok_or("Field not found".into())
    });
    match secret {
        Ok(v) => {
            write_secret(app, &v);
            if platform::accessibility_trusted(false) {
                let _ = platform::send_paste();
            }
            true
        }
        Err(_) => false,
    }
}

/// Background checks: auto-lock (idle, sleep, screen lock) and pause expiry.
pub fn spawn_background(app: AppHandle) {
    std::thread::Builder::new()
        .name("background".into())
        .spawn(move || {
            let mut last_wall = SystemTime::now();
            let mut was_paused = state(&app).settings().is_paused(now_ms());
            let mut was_trusted = platform::accessibility_trusted(false);
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let st = state(&app);
                let s = st.settings();
                let wall = SystemTime::now();
                // A large jump in wall-clock time between ticks means the Mac was asleep.
                let slept = wall.duration_since(last_wall).map(|d| d > Duration::from_secs(20)).unwrap_or(false);
                last_wall = wall;

                let unlocked = st.vault_key.lock().unwrap().is_some();
                if unlocked {
                    let idle = st.vault_activity.lock().unwrap().elapsed();
                    let idle_lock = s.auto_lock_minutes > 0 && idle > Duration::from_secs(s.auto_lock_minutes as u64 * 60);
                    let sleep_lock = s.lock_on_sleep && (slept || platform::screen_locked());
                    if idle_lock || sleep_lock {
                        lock_vault(&app);
                    }
                }

                let paused = s.is_paused(now_ms());
                if was_paused && !paused {
                    tray::refresh(&app);
                    let _ = app.emit("settings-changed", &s);
                }
                was_paused = paused;

                // Keep the tray's "Allow Accessibility" item in step with System Settings.
                let trusted = platform::accessibility_trusted(false);
                if trusted != was_trusted {
                    tray::refresh(&app);
                    let _ = app.emit("accessibility-changed", trusted);
                }
                was_trusted = trusted;
            }
        })
        .expect("spawn background");
}
