use std::fs;
use chrono::Local;
use tauri::State;

use crate::db::connection::{DbState, get_db_dir, get_db_path};

#[tauri::command]
pub fn create_backup(state: State<DbState>) -> Result<String, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Force checkpoint to ensure WAL is flushed
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").map_err(|e| e.to_string())?;

    let backup_dir = get_db_dir().join("backups");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_file = backup_dir.join(format!("backup_{}.db", timestamp));

    let db_path = get_db_path();
    fs::copy(&db_path, &backup_file).map_err(|e| format!("Error al crear backup: {}", e))?;

    // Rotate: keep only last 30 backups
    rotate_backups(&backup_dir, 30).ok();

    log::info!("Backup created: {:?}", backup_file);

    Ok(backup_file.to_string_lossy().to_string())
}

#[tauri::command]
pub fn export_database(state: State<DbState>, path: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").map_err(|e| e.to_string())?;

    let db_path = get_db_path();
    fs::copy(&db_path, &path).map_err(|e| format!("Error al exportar: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn get_backup_list() -> Result<Vec<String>, String> {
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
