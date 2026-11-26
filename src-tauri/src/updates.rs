//! Keeps Pastora up to date. Once a day (while "Check for updates" is on) it fetches
//! `latest.json` from the latest GitHub release: the only network request the app makes,
//! and it sends nothing but the request itself. A newer version is downloaded in the
//! background (signature checked against the key built into the app) and installed when
//! the user clicks "Restart to update". Where that isn't possible, the banner links to the
//! download instead.
use crate::state::{emit, state};
use crate::{settings, tray};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const RELEASES: &str = "https://github.com/pankaj1980patel/pastora/releases";
const DISMISSED: &str = "updates.dismissed";
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    /// Found; not downloaded (automatic download is off, or not started yet).
    Available,
    Downloading,
    /// Downloaded and verified: "Restart to update" installs it.
    Ready,
    Installing,
    /// Couldn't download or install here: offer the manual download.
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// Manual download for this OS (used when the update can't be installed in place).
    pub url: String,
    pub notes_url: String,
    pub stage: Stage,
    /// Why the last download/install failed.
    pub error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current: String,
    pub update: Option<UpdateInfo>,
    /// The user hid the banner for this version.
    pub dismissed: bool,
}

/// A downloaded, signature-checked package, kept on disk (it's 10–20 MB) rather than in
/// memory until the user restarts. The hash detects a file changed after verification.
#[derive(Clone)]
struct Downloaded {
    path: PathBuf,
    sha256: [u8; 32],
}

/// The update found by the last check, and its downloaded package.
static PENDING: Mutex<Option<(Update, Option<Downloaded>)>> = Mutex::new(None);

fn cache_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_cache_dir().ok().map(|d| d.join("updates"))
}

/// Deletes downloaded packages: left over from an earlier run (already installed, or
/// superseded) or replaced by a newer release.
fn clear_cache(app: &AppHandle) {
    if let Some(dir) = cache_dir(app) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn save_package(dir: &std::path::Path, version: &str, bytes: &[u8]) -> Result<Downloaded, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("Pastora-{version}.update"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(Downloaded { path, sha256: Sha256::digest(bytes).into() })
}

fn read_package(d: &Downloaded) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(&d.path).map_err(|e| e.to_string())?;
    if <[u8; 32]>::from(Sha256::digest(&bytes)) != d.sha256 {
        return Err("the downloaded file changed on disk".into());
    }
    Ok(bytes)
}
/// A download or install is running.
static BUSY: AtomicBool = AtomicBool::new(false);

/// The installer people download by hand, named as the release workflow names it.
fn download_url(version: &str) -> String {
    let file = if cfg!(target_os = "macos") {
        "macOS_universal.dmg"
    } else if cfg!(windows) {
        "Windows_x64-setup.exe"
    } else {
        "Linux_x86_64.AppImage"
    };
    format!("{RELEASES}/download/v{version}/Pastora_{version}_{file}")
}

fn changed(app: &AppHandle) {
    emit(app, "update-changed");
    tray::refresh(app);
}

fn set_stage(app: &AppHandle, stage: Stage, error: Option<String>) {
    if let Some(u) = state(app).update.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        u.stage = stage;
        u.error = error;
    }
    changed(app);
}

/// Asks for the latest release. Returns it if it's newer than this build, and starts the
/// background download when "Download updates automatically" is on.
pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let found = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let Some(update) = found else {
        if !BUSY.load(Ordering::SeqCst) {
            *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = None;
            *state(app).update.lock().unwrap_or_else(|e| e.into_inner()) = None;
            changed(app);
        }
        return Ok(None);
    };
    let version = update.version.clone();
    let known = state(app).update.lock().unwrap_or_else(|e| e.into_inner()).clone().filter(|u| u.version == version);
    let info = match known {
        // Already found earlier: keep its download and progress.
        Some(u) => u,
        None => {
            let info = UpdateInfo {
                url: download_url(&version),
                notes_url: format!("{RELEASES}/tag/v{version}"),
                version,
                stage: Stage::Available,
                error: None,
            };
            *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some((update, None));
            clear_cache(app); // a package for an older release is no longer needed
            *state(app).update.lock().unwrap_or_else(|e| e.into_inner()) = Some(info.clone());
            changed(app);
            info
        }
    };
    if state(app).settings().auto_download_updates && matches!(info.stage, Stage::Available | Stage::Failed) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = download(&app).await {
                log::warn!("update download failed: {e}");
            }
        });
    }
    Ok(Some(info))
}

/// Downloads and verifies the pending update (once).
async fn download(app: &AppHandle) -> Result<(), String> {
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("An update is already being downloaded.".into());
    }
    let res = download_inner(app).await;
    BUSY.store(false, Ordering::SeqCst);
    res
}

async fn download_inner(app: &AppHandle) -> Result<(), String> {
    let update = match PENDING.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        None => return Err("No update to download.".into()),
        Some((_, Some(_))) => return Ok(()), // already downloaded
        Some((u, None)) => u.clone(),
    };
    set_stage(app, Stage::Downloading, None);
    match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => {
            let saved = cache_dir(app).ok_or_else(|| "no cache folder".to_string())
                .and_then(|dir| save_package(&dir, &update.version, &bytes));
            drop(bytes);
            match saved {
                Ok(d) => {
                    if let Some(p) = PENDING.lock().unwrap_or_else(|e| e.into_inner()).as_mut().filter(|(u, _)| u.version == update.version) {
                        p.1 = Some(d);
                    }
                    set_stage(app, Stage::Ready, None);
                    Ok(())
                }
                Err(e) => {
                    let e = format!("Couldn't save the update: {e}");
                    set_stage(app, Stage::Failed, Some(e.clone()));
                    Err(e)
                }
            }
        }
        Err(e) => {
            set_stage(app, Stage::Failed, Some(e.to_string()));
            Err(e.to_string())
        }
    }
}

/// Installs the pending update (downloading it first if needed) and restarts Pastora.
/// On Windows the installer closes the app and opens it again when it's done.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    download(app).await?;
    let Some((update, Some(downloaded))) = PENDING.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
        return Err("The update isn't downloaded.".into());
    };
    let bytes = match read_package(&downloaded) {
        Ok(b) => b,
        Err(e) => {
            // Forget it, so the next attempt downloads (and verifies) it again.
            if let Some(p) = PENDING.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                p.1 = None;
            }
            set_stage(app, Stage::Failed, Some(e.clone()));
            return Err(format!("Couldn't install the update: {e}. Try again."));
        }
    };
    set_stage(app, Stage::Installing, None);
    let app2 = app.clone();
    let res = tauri::async_runtime::spawn_blocking(move || update.install(bytes)).await;
    match res {
        Ok(Ok(())) => {
            app2.restart();
        }
        Ok(Err(e)) => {
            set_stage(app, Stage::Failed, Some(e.to_string()));
            Err(format!("Couldn't install the update: {e}"))
        }
        Err(e) => {
            set_stage(app, Stage::Failed, Some(e.to_string()));
            Err(format!("Couldn't install the update: {e}"))
        }
    }
}

/// Tray menu "Update…": install when possible, otherwise open the download.
pub fn tray_clicked(app: &AppHandle) {
    let info = state(app).update.lock().unwrap_or_else(|e| e.into_inner()).clone();
    match info {
        Some(u) if u.stage == Stage::Failed => crate::platform::open_path(std::path::Path::new(&u.url)),
        Some(_) => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = install(&app).await {
                    crate::state::toast(&app, &e);
                }
            });
        }
        None => {}
    }
}

pub fn status(app: &AppHandle) -> UpdateStatus {
    let st = state(app);
    let update = st.update.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let dismissed = update
        .as_ref()
        .map(|u| settings::get_meta(&st.db(), DISMISSED).as_deref() == Some(u.version.as_str()))
        .unwrap_or(false);
    UpdateStatus { current: app.package_info().version.to_string(), update, dismissed }
}

pub fn dismiss(app: &AppHandle, version: &str) {
    let _ = settings::set_meta(&state(app).db(), DISMISSED, version);
}

/// Checks shortly after launch, then once a day, while "Check for updates" is on.
pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("updates".into())
        .spawn(move || {
            clear_cache(&app);
            std::thread::sleep(Duration::from_secs(10));
            loop {
                if state(&app).settings().check_updates {
                    // A plain thread, not a runtime worker, so blocking on the check is fine.
                    if let Err(e) = tauri::async_runtime::block_on(check(&app)) {
                        log::warn!("update check failed: {e}");
                    }
                }
                std::thread::sleep(DAY);
            }
        })
        .expect("spawn update checker");
}

#[cfg(test)]
mod tests {
    #[test]
    fn manual_download_matches_release_names() {
        let url = super::download_url("1.2.3");
        assert!(url.starts_with("https://github.com/pankaj1980patel/pastora/releases/download/v1.2.3/Pastora_1.2.3_"));
    }

    #[test]
    fn package_on_disk_round_trips_and_detects_changes() {
        let dir = std::env::temp_dir().join(format!("pastora-update-test-{}", std::process::id()));
        let d = super::save_package(&dir, "9.9.9", b"signed package bytes").unwrap();
        assert_eq!(super::read_package(&d).unwrap(), b"signed package bytes");
        std::fs::write(&d.path, b"swapped after verification").unwrap();
        assert!(super::read_package(&d).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
