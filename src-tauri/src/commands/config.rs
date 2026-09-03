use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::config::SystemConfig;
use crate::session::{require_admin, require_auth, SessionState};

#[tauri::command]
pub fn get_all_config(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<SystemConfig>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db.prepare("SELECT key, value, description FROM system_config ORDER BY key")
        .map_err(|e| e.to_string())?;

    let configs = stmt
        .query_map([], |row| {
            Ok(SystemConfig {
                key: row.get(0)?,
                value: row.get(1)?,
                description: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(configs)
}

#[tauri::command]
pub fn get_config(state: State<DbState>, sessions: State<SessionState>, token: String, key: String) -> Result<String, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.query_row(
        "SELECT value FROM system_config WHERE key = ?1",
        params![key],
        |row| row.get(0),
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_config(state: State<DbState>, sessions: State<SessionState>, token: String, key: String, value: String) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute(
        "INSERT OR REPLACE INTO system_config (key, value, updated_at) VALUES (?1, ?2, datetime('now','localtime'))",
        params![key, value],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
