use crate::db::now_ms;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    pub id: i64,
    pub label: Option<String>,
    pub kind: String,
    pub content: String,
    pub image_path: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub position: f64,
    pub created_at: i64,
}

const SELECT: &str =
    "SELECT id, label, kind, content, image_path, width, height, position, created_at FROM pins";

fn row(r: &Row) -> rusqlite::Result<Pin> {
    Ok(Pin {
        id: r.get(0)?,
        label: r.get(1)?,
        kind: r.get(2)?,
        content: r.get(3)?,
        image_path: r.get(4)?,
        width: r.get(5)?,
        height: r.get(6)?,
        position: r.get(7)?,
        created_at: r.get(8)?,
    })
}

pub fn list(conn: &Connection, query: &str) -> rusqlite::Result<Vec<Pin>> {
    let like = format!("%{}%", crate::history::escape_like(query));
    let mut stmt = conn.prepare(&format!(
        "{SELECT} WHERE (?1 = '' OR content LIKE ?2 ESCAPE '\\' OR label LIKE ?2 ESCAPE '\\')
         ORDER BY position ASC"
    ))?;
    let rows = stmt.query_map(params![query, like], row)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Option<Pin>> {
    conn.query_row(&format!("{SELECT} WHERE id = ?1"), [id], row).optional()
}

/// Adds a pin at the top of the list. Returns the existing pin if the same content is already pinned.
pub fn add(
    conn: &Connection,
    label: Option<&str>,
    kind: &str,
    content: &str,
    image_path: Option<&str>,
    size: (Option<i64>, Option<i64>),
) -> rusqlite::Result<i64> {
    if let Some(id) = conn
        .query_row("SELECT id FROM pins WHERE kind = ?1 AND content = ?2", params![kind, content], |r| r.get(0))
        .optional()?
    {
        return Ok(id);
    }
    let top: f64 = conn.query_row("SELECT COALESCE(MIN(position), 1.0) FROM pins", [], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO pins(label, kind, content, image_path, width, height, position, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![label, kind, content, image_path, size.0, size.1, top - 1.0, now_ms()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, label: Option<&str>, content: Option<&str>) -> rusqlite::Result<()> {
    let label = label.map(str::trim).filter(|s| !s.is_empty());
    conn.execute("UPDATE pins SET label = ?1 WHERE id = ?2", params![label, id])?;
    if let Some(c) = content {
        conn.execute("UPDATE pins SET content = ?1 WHERE id = ?2 AND kind = 'text'", params![c, id])?;
    }
    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM pins WHERE id = ?1", [id])?;
    Ok(())
}

pub fn delete_matching(conn: &Connection, kind: &str, content: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM pins WHERE kind = ?1 AND content = ?2", params![kind, content])?;
    Ok(())
}

/// Saves a new order. `ids` is the full list of pin ids from top to bottom.
pub fn reorder(conn: &mut Connection, ids: &[i64]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute("UPDATE pins SET position = ?1 WHERE id = ?2", params![i as f64, id])?;
    }
    tx.commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_puts_new_on_top_and_reorder_persists() {
        let mut conn = crate::db::memory();
        let a = add(&conn, None, "text", "a", None, (None, None)).unwrap();
        let b = add(&conn, None, "text", "b", None, (None, None)).unwrap();
        let c = add(&conn, Some("C"), "text", "c", None, (None, None)).unwrap();
        assert_eq!(add(&conn, None, "text", "a", None, (None, None)).unwrap(), a);
        let order = |conn: &Connection| list(conn, "").unwrap().iter().map(|p| p.id).collect::<Vec<_>>();
        assert_eq!(order(&conn), vec![c, b, a]);
        reorder(&mut conn, &[a, c, b]).unwrap();
        assert_eq!(order(&conn), vec![a, c, b]);
        assert_eq!(list(&conn, "C").unwrap().len(), 1);
    }
}
