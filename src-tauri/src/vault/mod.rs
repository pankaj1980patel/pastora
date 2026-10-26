//! Secure Clips: an encrypted vault for credentials.
//!
//! A random 256-bit vault key encrypts every entry (XChaCha20-Poly1305, entry id as AAD).
//! The vault key is wrapped by a key derived from the master password with Argon2id.

pub mod crypto;
pub mod totp;

use crate::db::now_ms;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use crypto::Key;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub id: String,
    pub label: String,
    pub value: String,
    /// text | email | url | secret | totp | note (multi-line secret)
    pub kind: String,
}

impl Field {
    pub fn is_secret(&self) -> bool {
        matches!(self.kind.as_str(), "secret" | "totp" | "note")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Payload {
    pub title: String,
    pub fields: Vec<Field>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    #[serde(default)]
    pub id: String,
    pub template: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(flatten)]
    pub payload: Payload,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSummary {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub has_value: bool,
    /// Value of non-secret fields only. Secret values never leave Rust in a summary.
    pub preview: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrySummary {
    pub id: String,
    pub template: String,
    pub title: String,
    pub subtitle: String,
    pub favorite: bool,
    pub updated_at: i64,
    pub fields: Vec<FieldSummary>,
    pub has_notes: bool,
}

pub fn summarize(e: &Entry) -> EntrySummary {
    let subtitle = e
        .payload
        .fields
        .iter()
        .find(|f| matches!(f.kind.as_str(), "text" | "email") && !f.value.is_empty())
        .or_else(|| e.payload.fields.iter().find(|f| f.kind == "url" && !f.value.is_empty()))
        .map(|f| f.value.clone())
        .unwrap_or_default();
    EntrySummary {
        id: e.id.clone(),
        template: e.template.clone(),
        title: e.payload.title.clone(),
        subtitle,
        favorite: e.favorite,
        updated_at: e.updated_at,
        fields: e
            .payload
            .fields
            .iter()
            .map(|f| FieldSummary {
                id: f.id.clone(),
                label: f.label.clone(),
                kind: f.kind.clone(),
                has_value: !f.value.is_empty(),
                preview: (!f.is_secret()).then(|| f.value.clone()),
            })
            .collect(),
        has_notes: !e.payload.notes.trim().is_empty(),
    }
}

pub fn is_initialized(conn: &Connection) -> bool {
    conn.query_row("SELECT 1 FROM vault_meta WHERE id = 1", [], |_| Ok(()))
        .optional()
        .ok()
        .flatten()
        .is_some()
}

struct Meta {
    salt: Vec<u8>,
    m: u32,
    t: u32,
    p: u32,
    nonce: Vec<u8>,
    wrapped: Vec<u8>,
}

fn load_meta(conn: &Connection) -> Result<Meta, String> {
    conn.query_row(
        "SELECT salt, m_cost, t_cost, p_cost, wrap_nonce, wrapped_key FROM vault_meta WHERE id = 1",
        [],
        |r| {
            Ok(Meta { salt: r.get(0)?, m: r.get(1)?, t: r.get(2)?, p: r.get(3)?, nonce: r.get(4)?, wrapped: r.get(5)? })
        },
    )
    .optional()
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Secure Clips is not set up yet".to_string())
}

fn write_meta(conn: &Connection, password: &str, key: &Key) -> Result<(), String> {
    let salt = crypto::random_salt();
    let kek = crypto::derive_kek(password, &salt, crypto::M_COST, crypto::T_COST, crypto::P_COST)?;
    let (nonce, wrapped) = crypto::seal(&kek, key.as_ref(), b"vault-key");
    conn.execute(
        "INSERT INTO vault_meta(id, salt, m_cost, t_cost, p_cost, wrap_nonce, wrapped_key, created_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET salt = excluded.salt, m_cost = excluded.m_cost,
           t_cost = excluded.t_cost, p_cost = excluded.p_cost, wrap_nonce = excluded.wrap_nonce,
           wrapped_key = excluded.wrapped_key",
        params![salt.to_vec(), crypto::M_COST, crypto::T_COST, crypto::P_COST, nonce, wrapped, now_ms()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < 8 {
        return Err("Use at least 8 characters".into());
    }
    Ok(())
}

pub fn setup(conn: &Connection, password: &str) -> Result<Key, String> {
    if is_initialized(conn) {
        return Err("Secure Clips is already set up".into());
    }
    validate_password(password)?;
    let key = crypto::random_key();
    write_meta(conn, password, &key)?;
    Ok(key)
}

pub fn unlock(conn: &Connection, password: &str) -> Result<Key, String> {
    let m = load_meta(conn)?;
    let kek = crypto::derive_kek(password, &m.salt, m.m, m.t, m.p)?;
    let raw = crypto::open(&kek, &m.nonce, &m.wrapped, b"vault-key").map_err(|_| "Wrong password".to_string())?;
    crypto::key_from_slice(&raw)
}

pub fn change_password(conn: &Connection, old: &str, new: &str) -> Result<Key, String> {
    validate_password(new)?;
    let key = unlock(conn, old).map_err(|_| "Current password is wrong".to_string())?;
    write_meta(conn, new, &key)?;
    Ok(key)
}

fn decrypt_row(key: &Key, id: &str, template: String, nonce: &[u8], ct: &[u8], fav: bool, updated: i64) -> Result<Entry, String> {
    let plain = crypto::open(key, nonce, ct, id.as_bytes())?;
    let payload: Payload = serde_json::from_slice(&plain).map_err(|e| e.to_string())?;
    Ok(Entry { id: id.to_string(), template, favorite: fav, updated_at: updated, payload })
}

pub fn list(conn: &Connection, key: &Key) -> Result<Vec<Entry>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, template, nonce, payload, favorite, updated_at FROM vault_entries
             ORDER BY favorite DESC, position ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, Vec<u8>>(3)?,
                r.get::<_, bool>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = vec![];
    for r in rows {
        let (id, t, n, ct, fav, up) = r.map_err(|e| e.to_string())?;
        out.push(decrypt_row(key, &id, t, &n, &ct, fav, up)?);
    }
    Ok(out)
}

pub fn get(conn: &Connection, key: &Key, id: &str) -> Result<Entry, String> {
    let (t, n, ct, fav, up) = conn
        .query_row(
            "SELECT template, nonce, payload, favorite, updated_at FROM vault_entries WHERE id = ?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?, r.get::<_, Vec<u8>>(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Entry not found".to_string())?;
    decrypt_row(key, id, t, &n, &ct, fav, up)
}

/// Creates (empty id) or updates an entry. Returns its id.
pub fn save(conn: &Connection, key: &Key, mut e: Entry) -> Result<String, String> {
    let is_new = e.id.is_empty();
    if is_new {
        e.id = uuid::Uuid::new_v4().to_string();
    }
    for f in e.payload.fields.iter_mut() {
        if f.id.is_empty() {
            f.id = uuid::Uuid::new_v4().to_string();
        }
    }
    if e.payload.title.trim().is_empty() {
        return Err("Title is required".into());
    }
    let json = zeroize::Zeroizing::new(serde_json::to_vec(&e.payload).map_err(|err| err.to_string())?);
    let (nonce, ct) = crypto::seal(key, &json, e.id.as_bytes());
    let now = now_ms();
    if is_new {
        let top: f64 = conn
            .query_row("SELECT COALESCE(MIN(position), 1.0) FROM vault_entries", [], |r| r.get(0))
            .map_err(|err| err.to_string())?;
        conn.execute(
            "INSERT INTO vault_entries(id, template, nonce, payload, position, favorite, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![e.id, e.template, nonce, ct, top - 1.0, e.favorite, now],
        )
    } else {
        conn.execute(
            "UPDATE vault_entries SET template = ?2, nonce = ?3, payload = ?4, favorite = ?5, updated_at = ?6
             WHERE id = ?1",
            params![e.id, e.template, nonce, ct, e.favorite, now],
        )
    }
    .map_err(|err| err.to_string())?;
    Ok(e.id)
}

pub fn set_favorite(conn: &Connection, id: &str, fav: bool) -> Result<(), String> {
    conn.execute("UPDATE vault_entries SET favorite = ?2 WHERE id = ?1", params![id, fav])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM vault_entries WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn reorder(conn: &mut Connection, ids: &[String]) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute("UPDATE vault_entries SET position = ?1 WHERE id = ?2", params![i as f64, id])
            .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn reset(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("DELETE FROM vault_entries; DELETE FROM vault_meta;").map_err(|e| e.to_string())
}

// ---------- Encrypted backup ----------

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub format: String,
    pub salt: String,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    pub wrap_nonce: String,
    pub wrapped_key: String,
    pub entries: Vec<BackupEntry>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub id: String,
    pub template: String,
    pub nonce: String,
    pub payload: String,
    pub favorite: bool,
    pub updated_at: i64,
}

/// Exports the vault as-is: everything stays encrypted under the master password.
pub fn export(conn: &Connection) -> Result<Backup, String> {
    let m = load_meta(conn)?;
    let mut stmt = conn
        .prepare("SELECT id, template, nonce, payload, favorite, updated_at FROM vault_entries ORDER BY position")
        .map_err(|e| e.to_string())?;
    let entries = stmt
        .query_map([], |r| {
            Ok(BackupEntry {
                id: r.get(0)?,
                template: r.get(1)?,
                nonce: B64.encode(r.get::<_, Vec<u8>>(2)?),
                payload: B64.encode(r.get::<_, Vec<u8>>(3)?),
                favorite: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(Backup {
        format: "pastora-vault-v1".into(),
        salt: B64.encode(m.salt),
        m_cost: m.m,
        t_cost: m.t,
        p_cost: m.p,
        wrap_nonce: B64.encode(m.nonce),
        wrapped_key: B64.encode(m.wrapped),
        entries,
    })
}

/// Decrypts a backup with its own password and adds its entries to the current (unlocked) vault.
pub fn import(conn: &Connection, key: &Key, backup: &Backup, password: &str) -> Result<usize, String> {
    let dec = |s: &str| B64.decode(s).map_err(|_| "Backup file is corrupt".to_string());
    let kek = crypto::derive_kek(password, &dec(&backup.salt)?, backup.m_cost, backup.t_cost, backup.p_cost)?;
    let raw = crypto::open(&kek, &dec(&backup.wrap_nonce)?, &dec(&backup.wrapped_key)?, b"vault-key")
        .map_err(|_| "Wrong password for this backup".to_string())?;
    let bkey = crypto::key_from_slice(&raw)?;
    let mut n = 0;
    for be in backup.entries.iter().rev() {
        let mut e = decrypt_row(&bkey, &be.id, be.template.clone(), &dec(&be.nonce)?, &dec(&be.payload)?, be.favorite, be.updated_at)?;
        e.id = String::new(); // always import as a new entry
        save(conn, key, e)?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login(title: &str) -> Entry {
        Entry {
            id: String::new(),
            template: "login".into(),
            favorite: false,
            updated_at: 0,
            payload: Payload {
                title: title.into(),
                fields: vec![
                    Field { id: String::new(), label: "Username".into(), value: "me@x.com".into(), kind: "email".into() },
                    Field { id: String::new(), label: "Password".into(), value: "hunter2".into(), kind: "secret".into() },
                ],
                notes: String::new(),
            },
        }
    }

    #[test]
    fn setup_unlock_change_password_and_crud() {
        let conn = crate::db::memory();
        assert!(!is_initialized(&conn));
        assert!(setup(&conn, "short").is_err());
        let key = setup(&conn, "correct horse").unwrap();
        assert!(unlock(&conn, "wrong password").is_err());
        let id = save(&conn, &key, login("GitHub")).unwrap();

        let key2 = change_password(&conn, "correct horse", "battery staple").unwrap();
        assert_eq!(key.as_ref(), key2.as_ref());
        assert!(unlock(&conn, "correct horse").is_err());
        let key3 = unlock(&conn, "battery staple").unwrap();
        let e = get(&conn, &key3, &id).unwrap();
        assert_eq!(e.payload.fields[1].value, "hunter2");

        let s = summarize(&e);
        assert_eq!(s.subtitle, "me@x.com");
        assert!(s.fields[1].preview.is_none(), "secret must not be in summary");

        // nothing readable in the DB
        let blob: Vec<u8> = conn.query_row("SELECT payload FROM vault_entries", [], |r| r.get(0)).unwrap();
        assert!(!String::from_utf8_lossy(&blob).contains("hunter2"));
        assert!(!String::from_utf8_lossy(&blob).contains("GitHub"));
    }

    #[test]
    fn export_import_between_vaults() {
        let a = crate::db::memory();
        let ka = setup(&a, "password-a").unwrap();
        save(&a, &ka, login("One")).unwrap();
        save(&a, &ka, login("Two")).unwrap();
        let backup = export(&a).unwrap();
        let json = serde_json::to_string(&backup).unwrap();

        let b = crate::db::memory();
        let kb = setup(&b, "password-b").unwrap();
        let parsed: Backup = serde_json::from_str(&json).unwrap();
        assert!(import(&b, &kb, &parsed, "password-b").is_err());
        assert_eq!(import(&b, &kb, &parsed, "password-a").unwrap(), 2);
        let titles: Vec<_> = list(&b, &kb).unwrap().into_iter().map(|e| e.payload.title).collect();
        assert_eq!(titles, vec!["Two", "One"]); // same order as the source vault
    }
}
