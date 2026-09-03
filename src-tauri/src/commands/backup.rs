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
    backup_into(db, &get_db_path(), &get_db_dir().join("backups"))
}

/// Núcleo del respaldo, con rutas explícitas para poder ejercitarlo en pruebas
/// sobre un directorio temporal en lugar del de la aplicación.
pub fn backup_into(
    db: &Connection,
    db_path: &std::path::Path,
    backup_dir: &std::path::Path,
) -> Result<String, String> {
    // Sin esto el respaldo se lleva la base sin los cambios que siguen en el WAL.
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").map_err(|e| e.to_string())?;

    fs::create_dir_all(backup_dir).map_err(|e| e.to_string())?;

    // Si en el mismo segundo se piden dos respaldos, el sufijo evita pisar uno.
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let mut backup_file = backup_dir.join(format!("backup_{}.db", timestamp));
    let mut n = 1;
    while backup_file.exists() {
        backup_file = backup_dir.join(format!("backup_{}_{}.db", timestamp, n));
        n += 1;
    }

    fs::copy(db_path, &backup_file).map_err(|e| format!("Error al crear backup: {}", e))?;

    rotate_backups(backup_dir, max_backups(db)).ok();

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

    let pending = get_db_dir().join(crate::db::connection::PENDING_RESTORE);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::{apply_pending_restore, PENDING_RESTORE};
    use crate::db::migrations::run_migrations;

    /// Base real en disco (no en memoria): el respaldo copia archivos, así que
    /// una prueba en memoria no ejercitaría nada de lo que puede fallar.
    fn open_db(path: &std::path::Path) -> Connection {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    fn count_products(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0)).unwrap()
    }

    fn add_product(conn: &Connection, sku: &str) {
        conn.execute(
            "INSERT INTO products (sku, name, purchase_price, sale_price, stock)
             VALUES (?1, ?1, 10.0, 20.0, 5)",
            rusqlite::params![sku],
        )
        .unwrap();
    }

    /// El ciclo completo tal como lo vive la tienda: respaldar, seguir vendiendo,
    /// restaurar y reiniciar la aplicación.
    #[test]
    fn a_backup_can_be_restored_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("things_shop.db");
        let backups = dir.path().join("backups");

        let conn = open_db(&db_path);
        add_product(&conn, "ANTES-1");
        add_product(&conn, "ANTES-2");
        let at_backup = count_products(&conn);

        let name = backup_into(&conn, &db_path, &backups).unwrap();
        assert!(backups.join(&name).exists(), "el archivo de respaldo debe existir");

        // La tienda sigue operando después del respaldo.
        add_product(&conn, "DESPUES-1");
        assert_eq!(count_products(&conn), at_backup + 1);
        drop(conn);

        // Restaurar deja el archivo preparado; el cambio ocurre al arrancar.
        fs::copy(backups.join(&name), db_path.parent().unwrap().join(PENDING_RESTORE)).unwrap();
        apply_pending_restore(&db_path);

        let conn = Connection::open(&db_path).unwrap();
        assert_eq!(count_products(&conn), at_backup, "debe volver al momento del respaldo");

        let survives: i64 = conn
            .query_row("SELECT COUNT(*) FROM products WHERE sku = 'DESPUES-1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(survives, 0, "lo posterior al respaldo no debe sobrevivir");
    }

    #[test]
    fn the_pending_file_is_consumed_so_it_only_restores_once() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("things_shop.db");
        let backups = dir.path().join("backups");

        let conn = open_db(&db_path);
        add_product(&conn, "UNO");
        let name = backup_into(&conn, &db_path, &backups).unwrap();
        drop(conn);

        let pending = dir.path().join(PENDING_RESTORE);
        fs::copy(backups.join(&name), &pending).unwrap();
        apply_pending_restore(&db_path);

        assert!(!pending.exists(), "el archivo preparado debe borrarse tras aplicarse");

        // Un segundo arranque no debe deshacer lo que se hizo después.
        let conn = Connection::open(&db_path).unwrap();
        add_product(&conn, "DOS");
        drop(conn);
        apply_pending_restore(&db_path);

        let conn = Connection::open(&db_path).unwrap();
        assert_eq!(count_products(&conn), 2);
    }

    #[test]
    fn stale_wal_files_do_not_resurrect_the_replaced_database() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("things_shop.db");
        let backups = dir.path().join("backups");

        let conn = open_db(&db_path);
        add_product(&conn, "ANTES");
        let name = backup_into(&conn, &db_path, &backups).unwrap();
        add_product(&conn, "DESPUES");
        drop(conn);

        // Un WAL suelto del archivo anterior arruinaría la restauración.
        let wal = dir.path().join("things_shop.db-wal");
        fs::write(&wal, b"basura del wal anterior").unwrap();

        fs::copy(backups.join(&name), dir.path().join(PENDING_RESTORE)).unwrap();
        apply_pending_restore(&db_path);

        assert!(!wal.exists(), "los archivos WAL/SHM viejos deben eliminarse");

        let conn = Connection::open(&db_path).unwrap();
        assert_eq!(count_products(&conn), 1);
    }

    #[test]
    fn nothing_happens_when_there_is_no_pending_restore() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("things_shop.db");

        let conn = open_db(&db_path);
        add_product(&conn, "UNO");
        drop(conn);

        apply_pending_restore(&db_path);

        let conn = Connection::open(&db_path).unwrap();
        assert_eq!(count_products(&conn), 1);
    }

    #[test]
    fn rotation_keeps_only_the_newest_backups() {
        let dir = tempfile::tempdir().unwrap();
        for n in 1..=5 {
            fs::write(dir.path().join(format!("backup_2026010{}_120000.db", n)), b"x").unwrap();
        }
        fs::write(dir.path().join("no-es-respaldo.txt"), b"x").unwrap();

        rotate_backups(dir.path(), 3).unwrap();

        let left: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".db"))
            .collect();

        assert_eq!(left.len(), 3);
        assert!(left.contains(&"backup_20260105_120000.db".to_string()));
        assert!(!left.contains(&"backup_20260101_120000.db".to_string()));
        assert!(dir.path().join("no-es-respaldo.txt").exists(), "no debe tocar otros archivos");
    }

    #[test]
    fn two_backups_in_the_same_second_do_not_overwrite_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("things_shop.db");
        let backups = dir.path().join("backups");

        let conn = open_db(&db_path);
        add_product(&conn, "UNO");

        let first = backup_into(&conn, &db_path, &backups).unwrap();
        let second = backup_into(&conn, &db_path, &backups).unwrap();

        assert_ne!(first, second);
        assert!(backups.join(&first).exists());
        assert!(backups.join(&second).exists());
    }
}
