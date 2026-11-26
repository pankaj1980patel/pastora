//! Stores the vault key in the OS keychain so Touch ID can unlock without the password.
//!
//! macOS, two ways, best first:
//! 1. **Biometry-bound** (data protection keychain, `BiometryCurrentSet` access control):
//!    macOS itself refuses to return the key without a Touch ID match from the currently
//!    enrolled fingers. Needs the app to be signed with a Developer ID and a
//!    `keychain-access-groups` entitlement; ad-hoc signed builds get `errSecMissingEntitlement`.
//! 2. **App-gated** (login keychain via `keyring`): the item is tied to this app's code signature
//!    and Pastora only reads it after its own Touch ID check succeeds.
//!
//! `store` uses (1) when the system allows it and falls back to (2); `load` reads whichever exists.
use crate::platform::AuthContext;
use crate::vault::crypto::{key_from_slice, Key};

const SERVICE: &str = "com.pastora.app";
const ACCOUNT: &str = "secure-clips-key";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
}

fn store_app_gated(key: &Key) -> Result<(), String> {
    entry()?.set_secret(key.as_ref()).map_err(|e| e.to_string())
}

fn load_app_gated() -> Result<Key, String> {
    let bytes = zeroize::Zeroizing::new(entry()?.get_secret().map_err(|e| e.to_string())?);
    key_from_slice(&bytes)
}

fn delete_app_gated() {
    if let Ok(e) = entry() {
        let _ = e.delete_credential();
    }
}

#[cfg(target_os = "macos")]
mod bound {
    use super::{ACCOUNT, SERVICE};
    use crate::platform::AuthContext;
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use core_foundation::string::CFString;
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework::passwords::{self, AccessControlOptions, PasswordOptions};

    pub const MISSING_ENTITLEMENT: i32 = -34018;
    pub const NOT_FOUND: i32 = -25300;
    pub const USER_CANCELED: i32 = -128;

    fn options() -> PasswordOptions {
        let mut o = PasswordOptions::new_generic_password(SERVICE, ACCOUNT);
        o.use_protected_keychain();
        o
    }

    /// Ok(true) stored, Ok(false) not supported for this build (falls back), Err otherwise.
    pub fn store(key: &[u8]) -> Result<bool, String> {
        let _ = passwords::delete_generic_password_options(options());
        let ac = SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenPasscodeSetThisDeviceOnly),
            AccessControlOptions::BIOMETRY_CURRENT_SET.bits(),
        )
        .map_err(|e| e.to_string())?;
        let mut o = options();
        o.set_access_control(ac);
        match passwords::set_generic_password_options(key, o) {
            Ok(()) => Ok(true),
            Err(e) if e.code() == MISSING_ENTITLEMENT => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Reads with the context of the Touch ID check that just succeeded, so macOS
    /// doesn't ask a second time. Err(code) carries the OSStatus.
    pub fn load(ctx: &AuthContext) -> Result<Vec<u8>, i32> {
        let mut o = options();
        let ctx_ref = objc2::rc::Retained::as_ptr(ctx) as CFTypeRef;
        #[allow(deprecated)] // the crate has no setter for kSecUseAuthenticationContext
        o.query.push((
            unsafe { CFString::wrap_under_get_rule(security_framework_sys::item::kSecUseAuthenticationContext) },
            unsafe { CFType::wrap_under_get_rule(ctx_ref) },
        ));
        passwords::generic_password(o).map_err(|e| e.code())
    }

    pub fn delete() {
        let _ = passwords::delete_generic_password_options(options());
    }
}

/// Saves the vault key for Touch ID unlock. Returns true when macOS itself enforces
/// the biometric check (biometry-bound item), false when Pastora's own check does.
pub fn store(key: &Key) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    if bound::store(key.as_ref())? {
        delete_app_gated();
        return Ok(true);
    }
    store_app_gated(key)?;
    Ok(false)
}

/// Reads the vault key after a successful biometric check (`ctx` comes from it).
pub fn load(ctx: &AuthContext) -> Result<Key, String> {
    #[cfg(target_os = "macos")]
    match bound::load(ctx) {
        Ok(bytes) => return key_from_slice(&zeroize::Zeroizing::new(bytes)),
        Err(bound::USER_CANCELED) => return Err("cancelled".into()),
        // No biometry-bound item (app-gated mode, or invalidated because the enrolled
        // fingers changed): try the app-gated one.
        Err(bound::NOT_FOUND) | Err(bound::MISSING_ENTITLEMENT) => {}
        Err(code) => return Err(format!("Keychain error {code}")),
    }
    let _ = ctx;
    load_app_gated().map_err(|_| {
        "The Touch ID key is no longer available (for example, fingerprints changed). \
         Unlock with your password, then turn Touch ID on again in Settings."
            .to_string()
    })
}

pub fn delete() {
    #[cfg(target_os = "macos")]
    bound::delete();
    delete_app_gated();
}
