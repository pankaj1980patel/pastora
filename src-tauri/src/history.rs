use crate::db::now_ms;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: i64,
    pub kind: String,
    pub content: String,
    pub image_path: Option<String>,
    pub source_app: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub created_at: i64,
    pub last_used_at: i64,
    pub use_count: i64,
    pub pinned: bool,
}

pub struct NewClip {
    pub kind: &'static str,
    pub content: String,
    pub image_path: Option<String>,
    pub hash: String,
    pub source_app: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

pub fn hash(kind: &str, bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(kind.as_bytes());
    h.update([0]);
    h.update(bytes);
    hex::encode(h.finalize())
}

const SELECT: &str = "SELECT c.id, c.kind, c.content, c.image_path, c.source_app, c.width, c.height,
        c.created_at, c.last_used_at, c.use_count,
        EXISTS(SELECT 1 FROM pins p WHERE p.kind = c.kind AND p.content = c.content)
    FROM clips c";

fn row(r: &Row) -> rusqlite::Result<Clip> {
    Ok(Clip {
        id: r.get(0)?,
        kind: r.get(1)?,
        content: r.get(2)?,
        image_path: r.get(3)?,
        source_app: r.get(4)?,
        width: r.get(5)?,
        height: r.get(6)?,
        created_at: r.get(7)?,
        last_used_at: r.get(8)?,
        use_count: r.get(9)?,
        pinned: r.get(10)?,
    })
}

/// Inserts a clip, or moves an identical existing one to the top.
pub fn insert(conn: &Connection, c: &NewClip) -> rusqlite::Result<()> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO clips(kind, content, image_path, hash, source_app, width, height, created_at, last_used_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
         ON CONFLICT(hash) DO UPDATE SET last_used_at = excluded.last_used_at,
                                         source_app = excluded.source_app",
        params![c.kind, c.content, c.image_path, c.hash, c.source_app, c.width, c.height, now],
    )?;
    Ok(())
}

pub fn list(conn: &Connection, query: &str, limit: i64, offset: i64) -> rusqlite::Result<Vec<Clip>> {
    let like = format!("%{}%", escape_like(query));
    let mut stmt = conn.prepare(&format!(
        "{SELECT} WHERE (?1 = '' OR c.content LIKE ?2 ESCAPE '\\' OR c.source_app LIKE ?2 ESCAPE '\\')
         ORDER BY c.last_used_at DESC LIMIT ?3 OFFSET ?4"
    ))?;
    let rows = stmt.query_map(params![query, like, limit, offset], row)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Clip>> {
    conn.query_row(&format!("{SELECT} WHERE c.id = ?1"), [id], row).optional()
}

pub fn touch(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE clips SET last_used_at = ?1, use_count = use_count + 1 WHERE id = ?2",
        params![now_ms(), id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: i64, images_dir: &Path) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM clips WHERE id = ?1", [id])?;
    remove_orphan_images(conn, images_dir)
}

pub fn clear(conn: &Connection, images_dir: &Path) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM clips", [])?;
    remove_orphan_images(conn, images_dir)
}

/// Applies the history size and age limits.
pub fn prune(conn: &Connection, limit: u32, days: u32, images_dir: &Path) -> rusqlite::Result<usize> {
    let mut removed = 0;
    if days > 0 {
        let cutoff = now_ms() - days as i64 * 86_400_000;
        removed += conn.execute("DELETE FROM clips WHERE last_used_at < ?1", [cutoff])?;
    }
    if limit > 0 {
        removed += conn.execute(
            "DELETE FROM clips WHERE id NOT IN
               (SELECT id FROM clips ORDER BY last_used_at DESC LIMIT ?1)",
            [limit],
        )?;
    }
    if removed > 0 {
        remove_orphan_images(conn, images_dir)?;
    }
    Ok(removed)
}

/// Deletes image files no longer referenced by any clip or pin.
pub fn remove_orphan_images(conn: &Connection, images_dir: &Path) -> rusqlite::Result<()> {
    let mut used = HashSet::new();
    for sql in [
        "SELECT image_path FROM clips WHERE image_path IS NOT NULL",
        "SELECT image_path FROM pins WHERE image_path IS NOT NULL",
    ] {
        let mut stmt = conn.prepare(sql)?;
        for p in stmt.query_map([], |r| r.get::<_, String>(0))? {
            used.insert(p?);
        }
    }
    if let Ok(entries) = std::fs::read_dir(images_dir) {
        for e in entries.flatten() {
            let p = e.path().to_string_lossy().to_string();
            if !used.contains(&p) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(())
}

pub fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(conn: &Connection, s: &str) {
        insert(conn, &NewClip {
            kind: "text",
            content: s.into(),
            image_path: None,
            hash: hash("text", s.as_bytes()),
            source_app: None,
            width: None,
            height: None,
        })
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    #[test]
    fn dedupe_moves_to_top() {
        let conn = crate::db::memory();
        text(&conn, "a");
        text(&conn, "b");
        text(&conn, "a");
        let l = list(&conn, "", 10, 0).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].content, "a");
    }

    #[test]
    fn prune_by_limit_and_search() {
        let conn = crate::db::memory();
        let dir = std::env::temp_dir().join("bc-test-images");
        for i in 0..10 {
            text(&conn, &format!("item {i}"));
        }
        prune(&conn, 3, 0, &dir).unwrap();
        let l = list(&conn, "", 100, 0).unwrap();
        assert_eq!(l.iter().map(|c| c.content.as_str()).collect::<Vec<_>>(), ["item 9", "item 8", "item 7"]);
        assert_eq!(list(&conn, "8", 100, 0).unwrap().len(), 1);
        assert_eq!(list(&conn, "%", 100, 0).unwrap().len(), 0);
    }
}
