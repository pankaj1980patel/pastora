//! Time-based one-time codes (RFC 6238).
use hmac::{Hmac, Mac};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    Sha1,
    Sha256,
    Sha512,
}

/// A TOTP secret and the parameters an `otpauth://` link can set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Totp {
    pub secret: Vec<u8>,
    pub digits: u32,
    pub period: u64,
    pub algorithm: Algorithm,
}

/// Minimal %XX decoding for query values (e.g. padding written as `%3D`).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(v) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Accepts a raw base32 secret (SHA-1, 6 digits, 30 s) or an `otpauth://` URI, whose
/// `digits`, `period` and `algorithm` are honoured. Returns None for anything unsupported,
/// rather than silently showing wrong codes.
pub fn parse(input: &str) -> Option<Totp> {
    let s = input.trim();
    let mut t = Totp { secret: vec![], digits: 6, period: 30, algorithm: Algorithm::Sha1 };
    let raw = if s.to_ascii_lowercase().starts_with("otpauth://") {
        let query = s.split_once('?')?.1;
        let mut secret = None;
        for pair in query.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let v = percent_decode(v);
            match k.to_ascii_lowercase().as_str() {
                "secret" => secret = Some(v),
                "digits" => t.digits = v.parse().ok().filter(|d| (6..=8).contains(d))?,
                "period" => t.period = v.parse().ok().filter(|p| (1..=300).contains(p))?,
                "algorithm" => {
                    t.algorithm = match v.to_ascii_uppercase().as_str() {
                        "SHA1" => Algorithm::Sha1,
                        "SHA256" => Algorithm::Sha256,
                        "SHA512" => Algorithm::Sha512,
                        _ => return None,
                    }
                }
                _ => {}
            }
        }
        secret?
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
    t.secret = data_encoding::BASE32_NOPAD.decode(cleaned.as_bytes()).ok().filter(|k| !k.is_empty())?;
    Some(t)
}

fn hmac<M: Mac + hmac::digest::KeyInit>(secret: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = <M as hmac::digest::KeyInit>::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

/// Returns (code, seconds remaining in the current period).
pub fn code(t: &Totp, unix_secs: u64) -> (String, u64) {
    let counter = (unix_secs / t.period).to_be_bytes();
    let h = match t.algorithm {
        Algorithm::Sha1 => hmac::<Hmac<sha1::Sha1>>(&t.secret, &counter),
        Algorithm::Sha256 => hmac::<Hmac<sha2::Sha256>>(&t.secret, &counter),
        Algorithm::Sha512 => hmac::<Hmac<sha2::Sha512>>(&t.secret, &counter),
    };
    let off = (h[h.len() - 1] & 0x0f) as usize;
    let bin = ((h[off] as u32 & 0x7f) << 24)
        | ((h[off + 1] as u32) << 16)
        | ((h[off + 2] as u32) << 8)
        | (h[off + 3] as u32);
    let code = bin % 10u32.pow(t.digits);
    (format!("{:0width$}", code, width = t.digits as usize), t.period - unix_secs % t.period)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn totp(secret: &[u8], digits: u32, algorithm: Algorithm) -> Totp {
        Totp { secret: secret.to_vec(), digits, period: 30, algorithm }
    }

    #[test]
    fn rfc6238_vectors() {
        // RFC 6238 appendix B (8 digits).
        let s1 = b"12345678901234567890";
        let s256 = b"12345678901234567890123456789012";
        let s512 = b"1234567890123456789012345678901234567890123456789012345678901234";
        for (t, want) in [(59, ["94287082", "46119246", "90693936"]), (1111111109, ["07081804", "68084774", "25091201"])] {
            assert_eq!(code(&totp(s1, 8, Algorithm::Sha1), t).0, want[0]);
            assert_eq!(code(&totp(s256, 8, Algorithm::Sha256), t).0, want[1]);
            assert_eq!(code(&totp(s512, 8, Algorithm::Sha512), t).0, want[2]);
        }
        // 6 digits is the last 6 of the 8-digit code.
        assert_eq!(code(&totp(s1, 6, Algorithm::Sha1), 59), ("287082".into(), 1));
    }

    #[test]
    fn parses_raw_and_otpauth() {
        let secret = b"12345678901234567890";
        let b32 = data_encoding::BASE32_NOPAD.encode(secret);
        let raw = parse(&b32.to_lowercase()).unwrap();
        assert_eq!((raw.secret.as_slice(), raw.digits, raw.period, raw.algorithm), (&secret[..], 6, 30, Algorithm::Sha1));

        let uri = format!("otpauth://totp/x?secret={b32}&issuer=y");
        assert_eq!(parse(&uri).unwrap(), raw);

        let uri = format!("otpauth://totp/x?secret={b32}%3D%3D&digits=8&period=60&algorithm=sha256");
        let t = parse(&uri).unwrap();
        assert_eq!((t.digits, t.period, t.algorithm), (8, 60, Algorithm::Sha256));
        assert_eq!(code(&t, 119).1, 1);

        // Unsupported parameters are rejected instead of producing wrong codes.
        assert!(parse(&format!("otpauth://totp/x?secret={b32}&algorithm=MD5")).is_none());
        assert!(parse(&format!("otpauth://totp/x?secret={b32}&digits=10")).is_none());
        assert!(parse(&format!("otpauth://totp/x?secret={b32}&period=0")).is_none());
        assert!(parse("otpauth://totp/x?issuer=y").is_none());
    }
}
