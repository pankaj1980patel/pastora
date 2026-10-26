//! Placeholder for Linux and Windows until their phases are implemented.
use super::Snapshot;

pub fn change_count() -> i64 { 0 }
pub fn read_snapshot() -> Snapshot { Snapshot::default() }
pub fn write_text(_text: &str, _concealed: bool) -> i64 { 0 }
pub fn write_files(_paths: &[String]) -> i64 { 0 }
pub fn write_png(_png: &[u8]) -> i64 { 0 }
pub fn clear_clipboard() -> i64 { 0 }
pub fn frontmost_app() -> Option<(String, String)> { None }
pub fn running_apps() -> Vec<(String, String)> { vec![] }
pub fn send_paste() -> Result<(), String> { Err("not supported on this platform yet".into()) }
pub fn accessibility_trusted(_prompt: bool) -> bool { true }
pub fn open_accessibility_settings() {}
pub fn reset_accessibility(_bundle_id: &str) {}
pub fn screen_locked() -> bool { false }
pub fn biometrics_available() -> bool { false }
pub fn biometric_name() -> &'static str { "Biometrics" }
pub fn biometric_auth(_reason: &str) -> Result<(), String> { Err("not supported on this platform yet".into()) }
pub fn open_path(_path: &std::path::Path) {}
pub fn frontmost_pid() -> Option<i32> { None }
pub fn activate_pid(_pid: i32) -> bool { false }
pub unsafe fn animate_in(_ns_window: *mut std::ffi::c_void) {}
pub unsafe fn animate_out(_ns_window: *mut std::ffi::c_void, done: impl FnOnce() -> bool + 'static) { done(); }
pub fn write_image_file(_path: &str) -> Option<i64> { None }
pub fn image_size(_path: &str) -> Option<(i64, i64)> { None }
