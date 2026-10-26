//! Watches the system clipboard and records new items in history.
use crate::db::now_ms;
use crate::history::{self, NewClip};
use crate::platform::{self, CONCEALED_TYPES, INTERNAL_TYPE};
use crate::state::{emit, state, AppState};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::AppHandle;

const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("clipboard-watcher".into())
        .spawn(move || {
            let mut last = platform::change_count();
            loop {
                std::thread::sleep(Duration::from_millis(350));
                let cc = platform::change_count();
                if cc == last {
                    continue;
                }
                last = cc;
                let st = state(&app);
                if cc == st.self_change.load(Ordering::SeqCst) {
                    continue;
                }
                match capture(&st) {
                    Ok(true) => emit(&app, "history-changed"),
                    Ok(false) => {}
                    Err(e) => log::warn!("clipboard capture failed: {e}"),
                }
            }
        })
        .expect("spawn watcher");
}

fn capture(st: &AppState) -> Result<bool, String> {
    let s = st.settings();
    if s.is_paused(now_ms()) {
        return Ok(false);
    }
    let snap = platform::read_snapshot();
    if snap.types.iter().any(|t| t == INTERNAL_TYPE) {
        return Ok(false);
    }
    if s.ignore_password_managers && snap.types.iter().any(|t| CONCEALED_TYPES.contains(&t.as_str())) {
        return Ok(false);
    }
    let front = platform::frontmost_app();
    if let Some((bundle, _)) = &front {
        if s.ignored_apps.iter().any(|a| &a.bundle_id == bundle) {
            return Ok(false);
        }
    }
    let source_app = front.map(|f| f.1);

    let clip = if !snap.files.is_empty() {
        let content = snap.files.join("\n");
        NewClip {
            kind: "files",
            hash: history::hash("files", content.as_bytes()),
            content,
            image_path: None,
            source_app,
            width: None,
            height: None,
        }
    } else if let Some(text) = snap.text {
        if text.trim().is_empty() || text.len() > MAX_TEXT_BYTES || matches_ignore_pattern(&s.ignore_patterns, &text) {
            return Ok(false);
        }
        NewClip {
            kind: "text",
            hash: history::hash("text", text.as_bytes()),
            content: text,
            image_path: None,
            source_app,
            width: None,
            height: None,
        }
    } else if snap.has_image && s.store_images {
        match capture_image(st, s.max_image_mb)? {
            Some(mut c) => {
                c.source_app = source_app;
                c
            }
            None => return Ok(false),
        }
    } else {
        return Ok(false);
    };

    let db = st.db();
    history::insert(&db, &clip).map_err(|e| e.to_string())?;
    history::prune(&db, s.history_limit, s.history_days, &st.images_dir).map_err(|e| e.to_string())?;
    Ok(true)
}

pub fn matches_ignore_pattern(patterns: &[String], text: &str) -> bool {
    patterns
        .iter()
        .filter(|p| !p.trim().is_empty())
        .filter_map(|p| regex::Regex::new(p).ok())
        .any(|re| re.is_match(text))
}

fn capture_image(st: &AppState, max_mb: u32) -> Result<Option<NewClip>, String> {
    let img = match arboard::Clipboard::new().and_then(|mut c| c.get_image()) {
        Ok(i) => i,
        Err(_) => return Ok(None),
    };
    let hash = history::hash("image", &img.bytes);
    let name = format!("{hash}.png");
    let path = st.images_dir.join(&name);
    if !path.exists() {
        let buf = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
            .ok_or("bad image data")?;
        let mut png = std::io::Cursor::new(Vec::new());
        buf.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        let png = png.into_inner();
        if max_mb > 0 && png.len() > max_mb as usize * 1024 * 1024 {
            return Ok(None);
        }
        std::fs::create_dir_all(&st.images_dir).map_err(|e| e.to_string())?;
        std::fs::write(&path, png).map_err(|e| e.to_string())?;
    }
    Ok(Some(NewClip {
        kind: "image",
        content: name,
        image_path: Some(path.to_string_lossy().to_string()),
        hash,
        source_app: None,
        width: Some(img.width as i64),
        height: Some(img.height as i64),
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn ignore_patterns() {
        let p = vec![r"^\d{6}$".to_string(), "(".to_string(), "".to_string()];
        assert!(super::matches_ignore_pattern(&p, "123456"));
        assert!(!super::matches_ignore_pattern(&p, "hello"));
    }
}
