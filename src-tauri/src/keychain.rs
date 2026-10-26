//! Stores the vault key in the OS keychain so Touch ID can unlock without the password.
//! The key is only read after a successful biometric check.
use crate::vault::crypto::{key_from_slice, Key};

const SERVICE: &str = "com.pastora.app";
const ACCOUNT: &str = "secure-clips-key";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
}

pub fn store(key: &Key) -> Result<(), String> {
    entry()?.set_secret(key.as_ref()).map_err(|e| e.to_string())
}

pub fn load() -> Result<Key, String> {
    let bytes = zeroize::Zeroizing::new(entry()?.get_secret().map_err(|e| e.to_string())?);
    key_from_slice(&bytes)
}

pub fn delete() {
    if let Ok(e) = entry() {
        let _ = e.delete_credential();
    }
}
