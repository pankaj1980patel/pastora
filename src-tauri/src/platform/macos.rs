use super::{Snapshot, INTERNAL_TYPE};
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use objc2::rc::Retained;
use objc2::AnyThread;
use objc2::runtime::{Bool, ProtocolObject};
use objc2_app_kit::{
    NSAnimatablePropertyContainer, NSAnimationContext, NSBitmapImageFileType, NSBitmapImageRep, NSImage, NSWindow,
    NSApplicationActivationOptions, NSApplicationActivationPolicy, NSPasteboard, NSPasteboardContentsOptions, NSRunningApplication, NSPasteboardItem, NSPasteboardTypeFileURL,
    NSPasteboardTypePNG, NSPasteboardTypeString, NSPasteboardTypeTIFF, NSPasteboardWriting, NSWorkspace,
};
use objc2_foundation::{NSArray, NSData, NSDictionary, NSError, NSString, NSURL};
use objc2_local_authentication::{LAContext, LAPolicy};
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::time::Duration;

fn pb() -> Retained<NSPasteboard> {
    NSPasteboard::generalPasteboard()
}

pub fn change_count() -> i64 {
    pb().changeCount() as i64
}

pub fn read_snapshot() -> Snapshot {
    let pb = pb();
    let types: Vec<String> = pb.types().map(|a| a.iter().map(|t| t.to_string()).collect()).unwrap_or_default();
    let file_type = unsafe { NSPasteboardTypeFileURL }.to_string();
    let mut files = vec![];
    if types.contains(&file_type) {
        if let Some(items) = pb.pasteboardItems() {
            for item in items.iter() {
                if let Some(url) = item.stringForType(unsafe { NSPasteboardTypeFileURL }) {
                    if let Some(path) = NSURL::URLWithString(&url).and_then(|u| u.path()) {
                        files.push(path.to_string());
                    }
                }
            }
        }
    }
    let text = if files.is_empty() {
        pb.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string())
    } else {
        None
    };
    let png = unsafe { NSPasteboardTypePNG }.to_string();
    let tiff = unsafe { NSPasteboardTypeTIFF }.to_string();
    let has_image = types.iter().any(|t| *t == png || *t == tiff);
    Snapshot { types, text, files, has_image }
}

fn write_items(items: Vec<Retained<NSPasteboardItem>>, current_host_only: bool) -> i64 {
    let pb = pb();
    if current_host_only {
        // Keeps secrets off Universal Clipboard (other Apple devices).
        pb.prepareForNewContentsWithOptions(NSPasteboardContentsOptions::CurrentHostOnly);
    } else {
        pb.clearContents();
    }
    let objs: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> =
        items.into_iter().map(ProtocolObject::from_retained).collect();
    pb.writeObjects(&NSArray::from_retained_slice(&objs));
    pb.changeCount() as i64
}

fn marker(item: &NSPasteboardItem, name: &str) {
    item.setData_forType(&NSData::with_bytes(b""), &NSString::from_str(name));
}

/// Writes plain text. `concealed` marks it so clipboard managers (and macOS
/// Universal Clipboard) skip it. Returns the new change count.
pub fn write_text(text: &str, concealed: bool) -> i64 {
    let item = NSPasteboardItem::new();
    item.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString });
    marker(&item, INTERNAL_TYPE);
    if concealed {
        marker(&item, "org.nspasteboard.ConcealedType");
        marker(&item, "org.nspasteboard.TransientType");
    }
    write_items(vec![item], concealed)
}

pub fn write_files(paths: &[String]) -> i64 {
    let items = paths
        .iter()
        .map(|p| {
            let item = NSPasteboardItem::new();
            let url = NSURL::fileURLWithPath(&NSString::from_str(p));
            if let Some(s) = url.absoluteString() {
                item.setString_forType(&s, unsafe { NSPasteboardTypeFileURL });
            }
            marker(&item, INTERNAL_TYPE);
            item
        })
        .collect();
    write_items(items, false)
}

pub fn write_png(png_bytes: &[u8]) -> i64 {
    let item = NSPasteboardItem::new();
    item.setData_forType(&NSData::with_bytes(png_bytes), unsafe { NSPasteboardTypePNG });
    marker(&item, INTERNAL_TYPE);
    write_items(vec![item], false)
}

fn load_image(path: &str) -> Option<Retained<NSImage>> {
    NSImage::initWithContentsOfFile(NSImage::alloc(), &NSString::from_str(path))
}

/// Writes any image file macOS can read (JPEG, HEIC, GIF, TIFF…) as PNG + TIFF.
pub fn write_image_file(path: &str) -> Option<i64> {
    let tiff = load_image(path)?.TIFFRepresentation()?;
    let png = NSBitmapImageRep::imageRepWithData(&tiff)
        .and_then(|rep| unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) });
    let item = NSPasteboardItem::new();
    if let Some(png) = png {
        item.setData_forType(&png, unsafe { NSPasteboardTypePNG });
    }
    item.setData_forType(&tiff, unsafe { NSPasteboardTypeTIFF });
    marker(&item, INTERNAL_TYPE);
    Some(write_items(vec![item], false))
}

/// Pixel size of an image file.
pub fn image_size(path: &str) -> Option<(i64, i64)> {
    let reps = load_image(path)?.representations();
    let rep = reps.iter().next()?;
    Some((rep.pixelsWide() as i64, rep.pixelsHigh() as i64))
}

pub fn clear_clipboard() -> i64 {
    let pb = pb();
    pb.clearContents();
    pb.changeCount() as i64
}

/// (bundle id, name) of the app that currently has focus.
pub fn frontmost_app() -> Option<(String, String)> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let bundle = app.bundleIdentifier().map(|s| s.to_string()).unwrap_or_default();
    let name = app.localizedName().map(|s| s.to_string()).unwrap_or_else(|| bundle.clone());
    Some((bundle, name))
}

/// Regular (Dock) apps that are running, for the "Ignored apps" picker.
pub fn running_apps() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|a| a.activationPolicy() == NSApplicationActivationPolicy::Regular)
        .filter_map(|a| {
            let b = a.bundleIdentifier()?.to_string();
            let n = a.localizedName().map(|s| s.to_string()).unwrap_or_else(|| b.clone());
            Some((b, n))
        })
        .collect();
    out.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    out.dedup();
    out
}

/// Simulates Cmd+V in the focused app. Needs Accessibility permission.
pub fn send_paste() -> Result<(), String> {
    const KEY_V: u16 = 9;
    let src = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| "event source")?;
    let down = CGEvent::new_keyboard_event(src.clone(), KEY_V, true).map_err(|_| "key event")?;
    down.set_flags(CGEventFlags::CGEventFlagCommand);
    let up = CGEvent::new_keyboard_event(src, KEY_V, false).map_err(|_| "key event")?;
    up.set_flags(CGEventFlags::CGEventFlagCommand);
    down.post(CGEventTapLocation::Session);
    std::thread::sleep(Duration::from_millis(12));
    up.post(CGEventTapLocation::Session);
    Ok(())
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
}

/// Whether we may send keystrokes. With `prompt`, macOS shows its permission dialog.
pub fn accessibility_trusted(prompt: bool) -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let val = if prompt { CFBoolean::true_value() } else { CFBoolean::false_value() };
        let opts = CFDictionary::from_CFType_pairs(&[(key, val)]);
        AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef())
    }
}

/// Drops macOS's Accessibility entry for this app. Needed when the app was rebuilt
/// (ad-hoc signed): the old entry stays ticked in System Settings but no longer applies.
pub fn reset_accessibility(bundle_id: &str) {
    let _ = std::process::Command::new("tccutil").args(["reset", "Accessibility", bundle_id]).status();
}

pub fn open_accessibility_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

pub fn screen_locked() -> bool {
    unsafe {
        let d = CGSessionCopyCurrentDictionary();
        if d.is_null() {
            return false;
        }
        let dict: CFDictionary<CFString, CFType> = CFDictionary::wrap_under_create_rule(d);
        dict.find(CFString::from_static_string("CGSSessionScreenIsLocked"))
            .and_then(|v| v.downcast::<CFBoolean>())
            .map(bool::from)
            .unwrap_or(false)
    }
}

pub fn biometrics_available() -> bool {
    unsafe { LAContext::new().canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics).is_ok() }
}

pub fn biometric_name() -> &'static str {
    "Touch ID"
}

/// Shows the Touch ID prompt and blocks until the user responds. Call off the main thread.
pub fn biometric_auth(reason: &str) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let ctx = unsafe { LAContext::new() };
    unsafe { ctx.setLocalizedFallbackTitle(Some(&NSString::from_str(""))) };
    let block = block2::RcBlock::new(move |ok: Bool, err: *mut NSError| {
        let res = if ok.as_bool() {
            Ok(())
        } else if err.is_null() {
            Err("Authentication failed".to_string())
        } else {
            let err = unsafe { &*err };
            // LAError codes. User/system/app cancel stay silent in the UI.
            Err(match err.code() {
                -2 | -4 | -9 => "cancelled".to_string(),
                -7 => "No fingerprints are set up. Add one in System Settings → Touch ID & Password.".to_string(),
                -8 => "Touch ID is locked. Unlock with your password.".to_string(),
                -1004 => "Touch ID couldn't show its prompt. Click “Use Touch ID” to try again.".to_string(),
                _ => err.localizedDescription().to_string(),
            })
        };
        let _ = tx.send(res);
    });
    unsafe {
        ctx.evaluatePolicy_localizedReason_reply(
            LAPolicy::DeviceOwnerAuthenticationWithBiometrics,
            &NSString::from_str(reason),
            &block,
        )
    };
    rx.recv_timeout(Duration::from_secs(60)).map_err(|_| "Timed out".to_string())?
}

pub fn open_path(path: &std::path::Path) {
    let _ = std::process::Command::new("open").arg(path).spawn();
}

pub fn frontmost_pid() -> Option<i32> {
    NSWorkspace::sharedWorkspace().frontmostApplication().map(|a| a.processIdentifier())
}

/// Gives focus back to the app the user was in before the popup opened.
pub fn activate_pid(pid: i32) -> bool {
    match NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        Some(app) => app.activateWithOptions(NSApplicationActivationOptions::empty()),
        None => false,
    }
}

/// Fades a just-shown window in with a small downward slide. Main thread only.
pub unsafe fn animate_in(ns_window: *mut c_void) {
    let Some(w) = Retained::retain(ns_window as *mut NSWindow) else { return };
    let end = w.frame();
    let mut start = end;
    if !NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion() {
        start.origin.y += 8.0;
    }
    w.setAlphaValue(0.0);
    w.setFrame_display(start, false);
    let changes = block2::StackBlock::new(move |ctx: NonNull<NSAnimationContext>| {
        ctx.as_ref().setDuration(0.16);
        let a = w.animator();
        a.setAlphaValue(1.0);
        a.setFrame_display(end, true);
    });
    NSAnimationContext::runAnimationGroup(&changes);
}

/// Fades a window out, then calls `done` (which should hide it). Main thread only.
/// `done` returns false if the window was re-shown meanwhile, so its alpha is left alone.
pub unsafe fn animate_out(ns_window: *mut c_void, done: impl FnOnce() -> bool + 'static) {
    let Some(w) = Retained::retain(ns_window as *mut NSWindow) else {
        done();
        return;
    };
    let w2 = w.clone();
    let done = Cell::new(Some(done));
    let changes = block2::StackBlock::new(move |ctx: NonNull<NSAnimationContext>| {
        ctx.as_ref().setDuration(0.11);
        w.animator().setAlphaValue(0.0);
    });
    let completion = block2::RcBlock::new(move || {
        if let Some(f) = done.take() {
            if f() {
                w2.setAlphaValue(1.0);
            }
        }
    });
    NSAnimationContext::runAnimationGroup_completionHandler(&changes, Some(&completion));
}
