use rusqlite::{Connection, Result as SqlResult};
use std::path::Path;

pub fn open_db(app_data_dir: &str) -> SqlResult<Connection> {
    let db_path = Path::new(app_data_dir).join("tiktok_now.db");
    let conn = Connection::open(db_path)?;

    // Enable WAL mode for high performance
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_cache (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;

    Ok(conn)
}
