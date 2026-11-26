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

    // Image bytes still to be written. They are written under the database lock, together
    // with the insert, so a concurrent delete/clear can't remove the file in between.
    let mut image_bytes: Option<Vec<u8>> = None;
    let mut thumb: Option<Vec<u8>> = None;
    let image_file = match snap.files.as_slice() {
        [f] if s.store_images && is_image_file(f) => capture_image_file(st, f, s.max_image_mb),
        _ => None,
    };
    let clip = if let Some(img) = image_file {
        let mut c = img.clip;
        c.source_app = source_app;
        image_bytes = img.bytes;
        c
    } else if !snap.files.is_empty() {
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
            Some(img) => {
                let mut c = img.clip;
                c.source_app = source_app;
                image_bytes = img.bytes;
                thumb = img.thumb;
                c
            }
            None => return Ok(false),
        }
    } else {
        return Ok(false);
    };

    let db = st.db();
    let mut written = None;
    if let Some(path) = &clip.image_path {
        let path = std::path::Path::new(path);
        if !path.exists() {
            // Either new, or the identical image was deleted since we looked.
            let Some(bytes) = image_bytes else { return Ok(false) };
            std::fs::create_dir_all(&st.images_dir).map_err(|e| e.to_string())?;
            std::fs::write(path, &bytes).map_err(|e| e.to_string())?;
            written = Some((path.to_path_buf(), bytes));
        }
    }
    history::insert(&db, &clip).map_err(|e| e.to_string())?;
    history::prune(&db, s.history_limit, s.history_days, &st.images_dir).map_err(|e| e.to_string())?;
    drop(db);
    // Making the preview is slow: do it without holding the database.
    if let Some((path, bytes)) = written {
        match thumb {
            Some(png) => {
                let _ = std::fs::write(history::thumb_path(&path), png);
            }
            None => history::write_thumb(&path, &bytes),
        }
    }
    Ok(true)
}

/// Compiled ignore patterns, rebuilt only when the pattern list changes.
static PATTERN_CACHE: std::sync::Mutex<Option<(Vec<String>, Vec<regex::Regex>)>> = std::sync::Mutex::new(None);

pub fn matches_ignore_pattern(patterns: &[String], text: &str) -> bool {
    if patterns.iter().all(|p| p.trim().is_empty()) {
        return false;
    }
    let mut cache = PATTERN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if cache.as_ref().map(|(src, _)| src.as_slice()) != Some(patterns) {
        let compiled = patterns
            .iter()
            .filter(|p| !p.trim().is_empty())
            .filter_map(|p| match regex::Regex::new(p) {
                Ok(re) => Some(re),
                Err(e) => {
                    log::warn!("ignoring invalid ignore pattern {p:?}: {e}");
                    None
                }
            })
            .collect();
        *cache = Some((patterns.to_vec(), compiled));
    }
    cache.as_ref().is_some_and(|(_, res)| res.iter().any(|re| re.is_match(text)))
}

/// An image clip, the bytes still to be stored (None when the file already exists) and,
/// when already made from pixels in memory, its preview PNG.
struct ImageClip {
    clip: NewClip,
    bytes: Option<Vec<u8>>,
    thumb: Option<Vec<u8>>,
}

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "heic", "heif", "tiff", "tif", "webp", "bmp"];

fn image_ext(path: &str) -> Option<String> {
    let ext = std::path::Path::new(path).extension()?.to_str()?.to_lowercase();
    IMAGE_EXTS.contains(&ext.as_str()).then_some(ext)
}

fn is_image_file(path: &str) -> bool {
    image_ext(path).is_some()
}

/// A single copied image file (e.g. a screenshot in Finder) is kept as a picture, not a path,
/// so it pastes as an image and doesn't make Finder ask to replace the original.
/// Returns None to fall back to a plain files clip; otherwise the clip and the bytes to store.
fn capture_image_file(st: &AppState, src: &str, max_mb: u32) -> Option<ImageClip> {
    let ext = image_ext(src)?;
    // Check the size before reading, so a huge file isn't loaded just to be skipped.
    let len = std::fs::metadata(src).ok()?.len();
    if max_mb > 0 && len > max_mb as u64 * 1024 * 1024 {
        return None;
    }
    let bytes = std::fs::read(src).ok()?;
    let hash = history::hash("image", &bytes);
    let path = st.images_dir.join(format!("{hash}.{ext}")).to_string_lossy().to_string();
    // Read the size from the original: the copy isn't written yet.
    let size = platform::image_size(src);
    let name = std::path::Path::new(src).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Some(ImageClip {
        clip: NewClip {
            kind: "image",
            content: name,
            image_path: Some(path),
            hash,
            source_app: None,
            width: size.map(|s| s.0),
            height: size.map(|s| s.1),
        },
        bytes: Some(bytes),
        thumb: None,
    })
}

fn capture_image(st: &AppState, max_mb: u32) -> Result<Option<ImageClip>, String> {
    let img = match arboard::Clipboard::new().and_then(|mut c| c.get_image()) {
        Ok(i) => i,
        Err(_) => return Ok(None),
    };
    let hash = history::hash("image", &img.bytes);
    let name = format!("{hash}.png");
    let path = st.images_dir.join(&name);
    let (mut bytes, mut thumb) = (None, None);
    if !path.exists() {
        let limit = if max_mb > 0 { max_mb as usize * 1024 * 1024 } else { usize::MAX };
        let (w, h) = (img.width as u32, img.height as u32);
        match encode_png_capped(&img.bytes, w, h, limit) {
            Ok(Some(png)) => bytes = Some(png),
            Ok(None) => return Ok(None), // over the size limit
            Err(e) => return Err(e),
        }
        thumb = history::thumb_from_rgba(&img.bytes, w, h);
    }
    Ok(Some(ImageClip {
        clip: NewClip {
            kind: "image",
            content: name,
            image_path: Some(path.to_string_lossy().to_string()),
            hash,
            source_app: None,
            width: Some(img.width as i64),
            height: Some(img.height as i64),
        },
        bytes,
        thumb,
    }))
}

/// A `Vec` writer that fails once more than `limit` bytes are written.
struct CappedWriter {
    buf: Vec<u8>,
    limit: usize,
    over: bool,
}

impl std::io::Write for CappedWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        if self.buf.len() + data.len() > self.limit {
            self.over = true;
            return Err(std::io::Error::other("over the image size limit"));
        }
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Encodes RGBA pixels as PNG, stopping as soon as the output passes `limit` bytes,
/// so an oversized image isn't fully encoded just to be thrown away.
/// Returns Ok(None) when it is over the limit.
fn encode_png_capped(rgba: &[u8], width: u32, height: u32, limit: usize) -> Result<Option<Vec<u8>>, String> {
    use image::ImageEncoder;
    if rgba.len() != width as usize * height as usize * 4 {
        return Err("bad image data".into());
    }
    let mut out = CappedWriter { buf: Vec::new(), limit, over: false };
    let res = image::codecs::png::PngEncoder::new(&mut out).write_image(rgba, width, height, image::ExtendedColorType::Rgba8);
    match res {
        Ok(()) => Ok(Some(out.buf)),
        Err(_) if out.over => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn png_size_cap() {
        // Noise compresses badly, so the PNG is about as big as the raw pixels.
        let mut x: u32 = 1;
        let rgba: Vec<u8> = (0..64 * 64 * 4)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (x >> 16) as u8
            })
            .collect();
        let full = super::encode_png_capped(&rgba, 64, 64, usize::MAX).unwrap().unwrap();
        assert!(image::load_from_memory(&full).is_ok());
        assert_eq!(super::encode_png_capped(&rgba, 64, 64, full.len()).unwrap().unwrap(), full);
        assert!(super::encode_png_capped(&rgba, 64, 64, 1000).unwrap().is_none());
        assert!(super::encode_png_capped(&rgba, 65, 64, usize::MAX).is_err());
    }

    #[test]
    fn ignore_patterns() {
        let p = vec![r"^\d{6}$".to_string(), "(".to_string(), "".to_string()];
        assert!(super::matches_ignore_pattern(&p, "123456"));
        assert!(!super::matches_ignore_pattern(&p, "hello"));
    }
}
