use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use zeroize::Zeroizing;

pub type Key = Zeroizing<[u8; 32]>;

/// Argon2id cost: 64 MiB memory, 3 passes, 1 lane.
pub const M_COST: u32 = 64 * 1024;
pub const T_COST: u32 = 3;
pub const P_COST: u32 = 1;

pub fn random_key() -> Key {
    let mut k = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(k.as_mut());
    k
}

pub fn random_salt() -> [u8; 16] {
    let mut s = [0u8; 16];
    OsRng.fill_bytes(&mut s);
    s
}

pub fn derive_kek(password: &str, salt: &[u8], m: u32, t: u32, p: u32) -> Result<Key, String> {
    let params = Params::new(m, t, p, Some(32)).map_err(|e| e.to_string())?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(password.as_bytes(), salt, out.as_mut())
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Encrypts with XChaCha20-Poly1305. Returns (nonce, ciphertext).
pub fn seal(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ct = cipher
        .encrypt(&nonce, Payload { msg: plaintext, aad })
        .expect("encryption cannot fail for valid key");
    (nonce.to_vec(), ct)
}

pub fn open(key: &[u8; 32], nonce: &[u8], ct: &[u8], aad: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    if nonce.len() != 24 {
        return Err("corrupt data".into());
    }
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
        .map(Zeroizing::new)
        .map_err(|_| "decryption failed".to_string())
}

pub fn key_from_slice(b: &[u8]) -> Result<Key, String> {
    if b.len() != 32 {
        return Err("invalid key".into());
    }
    let mut k = Zeroizing::new([0u8; 32]);
    k.copy_from_slice(b);
    Ok(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_and_tamper() {
        let k = random_key();
        let (n, ct) = seal(&k, b"hello", b"id1");
        assert_eq!(&open(&k, &n, &ct, b"id1").unwrap()[..], b"hello");
        assert!(open(&k, &n, &ct, b"id2").is_err());
        let other = random_key();
        assert!(open(&other, &n, &ct, b"id1").is_err());
    }
}
