use rusqlite::Connection;
use std::path::Path;

const SCHEMA_V1: &str = r#"
CREATE TABLE IF NOT EXISTS clips (
    id           INTEGER PRIMARY KEY,
    kind         TEXT    NOT NULL,           -- text | image | files
    content      TEXT    NOT NULL,           -- text, newline-separated paths, or image file name
    image_path   TEXT,
    hash         TEXT    NOT NULL UNIQUE,
    source_app   TEXT,
    width        INTEGER,
    height       INTEGER,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    use_count    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS clips_last_used ON clips(last_used_at DESC);

CREATE TABLE IF NOT EXISTS pins (
    id         INTEGER PRIMARY KEY,
    label      TEXT,
    kind       TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    image_path TEXT,
    width      INTEGER,
    height     INTEGER,
    position   REAL    NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS vault_meta (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    salt        BLOB    NOT NULL,
    m_cost      INTEGER NOT NULL,
    t_cost      INTEGER NOT NULL,
    p_cost      INTEGER NOT NULL,
    wrap_nonce  BLOB    NOT NULL,
    wrapped_key BLOB    NOT NULL,
    created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS vault_entries (
    id         TEXT    PRIMARY KEY,
    template   TEXT    NOT NULL,
    nonce      BLOB    NOT NULL,
    payload    BLOB    NOT NULL,             -- encrypted JSON: title, fields, notes
    position   REAL    NOT NULL,
    favorite   INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    // WAL + FULL sync: every change is on disk before the call returns, so
    // nothing is lost on crash, quit or reboot.
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
        conn.execute_batch("PRAGMA user_version = 1")?;
    }
    Ok(())
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
pub fn memory() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn
}
