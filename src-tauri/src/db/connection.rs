use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use super::migrations;

pub struct DbState {
    pub db: Mutex<Connection>,
}

/// Get the database directory path within the app's data directory
pub fn get_db_dir() -> PathBuf {
    let app_data = dirs_next().unwrap_or_else(|| PathBuf::from("."));
    let db_dir = app_data.join("things-shop");
    fs::create_dir_all(&db_dir).expect("Failed to create database directory");
    db_dir
}

/// Get a cross-platform app data directory
fn dirs_next() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA").ok().map(PathBuf::from)
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".local").join("share"))
    }
}

/// Get the database file path
pub fn get_db_path() -> PathBuf {
    get_db_dir().join("things_shop.db")
}

/// Initialize the database connection with WAL mode and run migrations
pub fn init_db() -> Result<Connection, rusqlite::Error> {
    let db_path = get_db_path();
    let conn = Connection::open(&db_path)?;

    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    // Enable foreign keys
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    // Optimize for speed
    conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
    conn.execute_batch("PRAGMA cache_size=-8000;")?; // 8MB cache
    conn.execute_batch("PRAGMA temp_store=MEMORY;")?;

    // Run migrations
    migrations::run_migrations(&conn)?;

    log::info!("Database initialized at {:?}", db_path);

    Ok(conn)
}
