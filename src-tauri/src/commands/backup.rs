use std::fs;
use chrono::Local;
use rusqlite::Connection;
use tauri::State;

use crate::db::connection::{DbState, get_db_dir, get_db_path};
use crate::session::{require_admin, SessionState};

/// Read the configured maximum number of backups to retain (default 30).
fn max_backups(db: &Connection) -> usize {
    db.query_row(
        "SELECT value FROM system_config WHERE key = 'max_backups'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|s| s.trim().parse::<usize>().ok())
    .filter(|n| *n > 0)
    .unwrap_or(30)
}

/// Flush the WAL and copy the DB file into the backups folder. Shared by the
/// manual backup command and the automatic backup on register close.
pub fn perform_backup(db: &Connection) -> Result<String, String> {
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").map_err(|e| e.to_string())?;

    let backup_dir = get_db_dir().join("backups");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_file = backup_dir.join(format!("backup_{}.db", timestamp));

    let db_path = get_db_path();
    fs::copy(&db_path, &backup_file).map_err(|e| format!("Error al crear backup: {}", e))?;

    rotate_backups(&backup_dir, max_backups(db)).ok();

    log::info!("Backup created: {:?}", backup_file);
    Ok(backup_file.file_name().unwrap().to_string_lossy().to_string())
}

#[tauri::command]
pub fn create_backup(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<String, String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    perform_backup(&db)
}

#[tauri::command]
pub fn export_database(state: State<DbState>, sessions: State<SessionState>, token: String, path: String) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").map_err(|e| e.to_string())?;

    let db_path = get_db_path();
    fs::copy(&db_path, &path).map_err(|e| format!("Error al exportar: {}", e))?;

    Ok(())
}

/// Stage a backup to be restored on the next app launch. Restoring in place while
/// the connection is open is unsafe, so we copy the chosen backup to a pending
/// file that `init_db` swaps in before opening the connection.
#[tauri::command]
pub fn restore_backup(state: State<DbState>, sessions: State<SessionState>, token: String, filename: String) -> Result<String, String> {
    require_admin(&sessions, &token)?;
    // Guard against path traversal — only plain filenames from the backups dir.
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("Nombre de respaldo inválido".to_string());
    }

    let backup_dir = get_db_dir().join("backups");
    let source = backup_dir.join(&filename);
    if !source.exists() {
        return Err("El respaldo seleccionado no existe".to_string());
    }

    // Flush current WAL so the pending copy is complete/consistent.
    {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").ok();
    }

    let pending = get_db_dir().join("things_shop.db.restore-pending");
    fs::copy(&source, &pending).map_err(|e| format!("Error al preparar restauración: {}", e))?;

    Ok("Respaldo listo. Reinicia la aplicación para completar la restauración.".to_string())
}

/// Location of the app log file, so Settings can show it and open it.
#[tauri::command]
pub fn get_log_path(sessions: State<SessionState>, token: String) -> Result<String, String> {
    require_admin(&sessions, &token)?;
    Ok(crate::logging::log_path().to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_backup_list(sessions: State<SessionState>, token: String) -> Result<Vec<String>, String> {
    require_admin(&sessions, &token)?;
    let backup_dir = get_db_dir().join("backups");

    if !backup_dir.exists() {
        return Ok(vec![]);
    }

    let mut backups: Vec<String> = fs::read_dir(&backup_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".db") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    backups.sort_by(|a, b| b.cmp(a)); // Most recent first
    Ok(backups)
}

fn rotate_backups(backup_dir: &std::path::Path, max: usize) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(backup_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "db").unwrap_or(false))
        .collect();

    entries.sort_by_key(|e| e.file_name());

    if entries.len() > max {
        for entry in entries.iter().take(entries.len() - max) {
            fs::remove_file(entry.path()).ok();
        }
    }

    Ok(())
}
