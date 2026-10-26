use hmac::{Hmac, Mac};
use sha1::Sha1;

/// Accepts a raw base32 secret or an otpauth:// URI.
pub fn parse_secret(input: &str) -> Option<Vec<u8>> {
    let s = input.trim();
    let raw = if s.starts_with("otpauth://") {
        s.split(['?', '&']).find_map(|p| p.strip_prefix("secret="))?.to_string()
    } else {
        s.to_string()
    };
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect::<String>()
        .trim_end_matches('=')
        .to_string();
    data_encoding::BASE32_NOPAD.decode(cleaned.as_bytes()).ok().filter(|k| !k.is_empty())
}

/// Returns (6-digit code, seconds remaining in the 30 s window).
pub fn code(secret: &[u8], unix_secs: u64) -> (String, u64) {
    let counter = unix_secs / 30;
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(&counter.to_be_bytes());
    let h = mac.finalize().into_bytes();
    let off = (h[19] & 0x0f) as usize;
    let bin = ((h[off] as u32 & 0x7f) << 24)
        | ((h[off + 1] as u32) << 16)
        | ((h[off + 2] as u32) << 8)
        | (h[off + 3] as u32);
    (format!("{:06}", bin % 1_000_000), 30 - unix_secs % 30)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6238_vector() {
        // RFC 6238 SHA1 secret "12345678901234567890" at T=59 -> 94287082 (8 digits) -> 287082
        let secret = b"12345678901234567890";
        assert_eq!(code(secret, 59).0, "287082");
        let b32 = data_encoding::BASE32_NOPAD.encode(secret);
        assert_eq!(parse_secret(&b32.to_lowercase()).unwrap(), secret);
        let uri = format!("otpauth://totp/x?secret={b32}&issuer=y");
        assert_eq!(parse_secret(&uri).unwrap(), secret);
    }
}
