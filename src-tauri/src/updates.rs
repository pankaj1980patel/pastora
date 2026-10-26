//! Checks GitHub for a newer Pastora release. The only network request the app makes;
//! it sends nothing but the request itself and can be turned off in Settings.
use crate::state::{emit, state};
use crate::{settings, tray};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::AppHandle;

const LATEST: &str = "https://api.github.com/repos/pankaj1980patel/pastora/releases/latest";
const DISMISSED: &str = "updates.dismissed";
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// Download for this OS, or the release page if there's no matching file.
    pub url: String,
    pub notes_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub current: String,
    pub update: Option<UpdateInfo>,
    /// The user hid the banner for this version.
    pub dismissed: bool,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.trim().trim_start_matches('v').split(['.', '-', '+']).map(|p| p.parse::<u64>());
    Some((it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?))
}

fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(l), Some(c)) if l > c)
}

fn asset_suffix() -> &'static str {
    if cfg!(target_os = "macos") {
        ".dmg"
    } else if cfg!(windows) {
        "-setup.exe"
    } else {
        ".AppImage"
    }
}

fn fetch_latest() -> Result<Release, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build();
    agent
        .get(LATEST)
        .set("User-Agent", "Pastora update check")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .into_json::<Release>()
        .map_err(|e| e.to_string())
}

/// Asks GitHub for the latest release. Returns it if it's newer than this build.
pub fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    let rel = fetch_latest()?;
    let current = app.package_info().version.to_string();
    let update = if !rel.draft && !rel.prerelease && is_newer(&rel.tag_name, &current) {
        let url = rel
            .assets
            .iter()
            .find(|a| a.name.ends_with(asset_suffix()))
            .map(|a| a.browser_download_url.clone())
            .unwrap_or_else(|| rel.html_url.clone());
        Some(UpdateInfo { version: rel.tag_name.trim_start_matches('v').to_string(), url, notes_url: rel.html_url })
    } else {
        None
    };
    let found = update.is_some();
    *state(app).update.lock().unwrap() = update.clone();
    if found {
        emit(app, "update-available");
    }
    tray::refresh(app);
    Ok(update)
}

pub fn status(app: &AppHandle) -> UpdateStatus {
    let st = state(app);
    let update = st.update.lock().unwrap().clone();
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
            std::thread::sleep(Duration::from_secs(10));
            loop {
                if state(&app).settings().check_updates {
                    if let Err(e) = check(&app) {
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
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.2.0", "0.1.1"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("v0.1.1", "0.1.1"));
        assert!(!is_newer("v0.1.0", "0.1.1"));
        assert!(!is_newer("nightly", "0.1.1"));
    }
}
