use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::migrations;

pub struct DbState {
    /// Compartido con el servidor de captura, que corre en otra tarea y necesita
    /// la misma conexión —y por tanto el mismo candado— que los comandos.
    pub db: Arc<Mutex<Connection>>,
}

impl DbState {
    pub fn new(conn: Connection) -> Self {
        DbState { db: Arc::new(Mutex::new(conn)) }
    }

    /// La conexión, incluso si un comando anterior reventó mientras la tenía.
    ///
    /// Rust marca un candado como envenenado cuando el hilo que lo tenía entra
    /// en pánico, y a partir de ahí todo intento de tomarlo falla. En una
    /// aplicación de escritorio eso significa que un solo error deja la caja
    /// muerta: la ventana sigue abierta, los botones responden y ningún comando
    /// vuelve a funcionar hasta reiniciar, con la venta a medias perdida.
    ///
    /// Aquí se recupera. Lo que sí queda por limpiar es una transacción abierta
    /// que el pánico dejó a medias: sin cerrarla, la siguiente operación
    /// fallaría con "cannot start a transaction within a transaction". Se
    /// deshace, que es lo correcto —esa transacción nunca llegó a completarse.
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        recuperar(self.db.lock())
    }
}

/// Devuelve la conexión aunque el candado esté envenenado, dejándola usable.
pub fn recuperar<'a>(
    resultado: Result<
        std::sync::MutexGuard<'a, Connection>,
        std::sync::PoisonError<std::sync::MutexGuard<'a, Connection>>,
    >,
) -> std::sync::MutexGuard<'a, Connection> {
    match resultado {
        Ok(guard) => guard,
        Err(envenenado) => {
            log::error!(
                "Un comando anterior falló dejando la base tomada; se recupera y se \
                 deshace lo que quedara a medias"
            );
            let guard = envenenado.into_inner();
            // Si no había transacción abierta, esto falla y no pasa nada.
            let _ = guard.execute_batch("ROLLBACK;");
            guard
        }
    }
}

/// Get the database directory path within the app's data directory
pub fn get_db_dir() -> PathBuf {
    let app_data = dirs_next().unwrap_or_else(|| PathBuf::from("."));
    let db_dir = app_data.join("things-shop");
    if let Err(e) = fs::create_dir_all(&db_dir) {
        log::error!("No se pudo crear el directorio de datos {:?}: {}", db_dir, e);
    }
    db_dir
}

/// Get a cross-platform app data directory, using each OS's own convention.
fn dirs_next() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA").ok().map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".local").join("share"))
            })
    }
}

/// Get the database file path
pub fn get_db_path() -> PathBuf {
    get_db_dir().join("things_shop.db")
}

/// Initialize the database connection with WAL mode and run migrations
pub fn init_db() -> Result<Connection, rusqlite::Error> {
    let db_path = get_db_path();

    // Earlier builds stored the DB under ~/.local/share on macOS. Move it once so
    // existing shops keep their data after the path was corrected.
    migrate_legacy_macos_path(&db_path);

    // If a restore was staged, swap it in before opening the connection.
    apply_pending_restore(&db_path);

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

/// Nombre del archivo que `restore_backup` deja preparado para el próximo arranque.
pub const PENDING_RESTORE: &str = "things_shop.db.restore-pending";

/// Replace the main DB with a staged restore (created by `restore_backup`),
/// clearing any leftover WAL/SHM sidecar files so the restored data is used.
///
/// Se deriva del propio `db_path` en vez de consultar el directorio global para
/// que la lógica pueda probarse sobre un directorio temporal.
pub fn apply_pending_restore(db_path: &std::path::Path) {
    let pending = match db_path.parent() {
        Some(dir) => dir.join(PENDING_RESTORE),
        None => return,
    };
    if !pending.exists() {
        return;
    }
    // Remove WAL/SHM sidecars tied to the old database file.
    for ext in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", db_path.to_string_lossy(), ext));
        let _ = fs::remove_file(sidecar);
    }
    match fs::copy(&pending, db_path) {
        Ok(_) => {
            let _ = fs::remove_file(&pending);
            log::info!("Restored database from staged backup");
        }
        Err(e) => log::error!("Failed to apply pending restore: {}", e),
    }
}

/// One-time move of a pre-existing database from the old macOS location.
#[cfg(target_os = "macos")]
fn migrate_legacy_macos_path(db_path: &std::path::Path) {
    if db_path.exists() {
        return;
    }
    let legacy = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h).join(".local/share/things-shop/things_shop.db"),
        Err(_) => return,
    };
    if !legacy.exists() {
        return;
    }
    for ext in ["", "-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{}", legacy.to_string_lossy(), ext));
        let to = PathBuf::from(format!("{}{}", db_path.to_string_lossy(), ext));
        if from.exists() {
            let _ = fs::copy(&from, &to);
        }
    }
    log::info!("Base de datos migrada desde la ruta anterior de macOS");
}

#[cfg(not(target_os = "macos"))]
fn migrate_legacy_macos_path(_db_path: &std::path::Path) {}

/// Delete `app_logs` rows older than the configured retention window so the
/// database does not grow without bound.
pub fn purge_old_logs(conn: &Connection) {
    let days: i64 = conn
        .query_row(
            "SELECT value FROM system_config WHERE key = 'log_retention_days'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .filter(|d| *d > 0)
        .unwrap_or(90);

    match conn.execute(
        "DELETE FROM app_logs WHERE created_at < datetime('now', 'localtime', ?1)",
        [format!("-{} days", days)],
    ) {
        Ok(n) if n > 0 => log::info!("Bitácora purgada: {} registros eliminados", n),
        Ok(_) => {}
        Err(e) => log::warn!("No se pudo purgar la bitácora: {}", e),
    }
}

#[cfg(test)]
mod tests_candado {
    use super::*;

    #[test]
    fn un_panic_no_deja_la_base_inservible() {
        // Sin esto, un solo error dejaba la caja muerta hasta reiniciar: el
        // candado quedaba envenenado y ningún comando volvía a funcionar.
        let estado = DbState::new(Connection::open_in_memory().unwrap());
        estado
            .conn()
            .execute_batch("CREATE TABLE t (id INTEGER); INSERT INTO t VALUES (1);")
            .unwrap();

        let db = Arc::clone(&estado.db);
        let reventado = std::thread::spawn(move || {
            let _guard = db.lock().unwrap();
            panic!("algo falló con la base tomada");
        })
        .join();
        assert!(reventado.is_err(), "el hilo debía entrar en pánico");

        let cuantos: i64 = estado
            .conn()
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .expect("la base debe seguir sirviendo");
        assert_eq!(cuantos, 1);
    }

    #[test]
    fn una_transaccion_a_medias_se_deshace_al_recuperar() {
        let estado = DbState::new(Connection::open_in_memory().unwrap());
        estado
            .conn()
            .execute_batch("CREATE TABLE t (id INTEGER);")
            .unwrap();

        let db = Arc::clone(&estado.db);
        let _ = std::thread::spawn(move || {
            let guard = db.lock().unwrap();
            guard
                .execute_batch("BEGIN TRANSACTION; INSERT INTO t VALUES (99);")
                .unwrap();
            panic!("se cayó a media transacción");
        })
        .join();

        // La conexión vuelve utilizable y lo que quedó a medias no se guardó.
        let cuantos: i64 = estado
            .conn()
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cuantos, 0, "la transacción incompleta no debe quedar escrita");

        // Y se puede abrir una nueva sin chocar con la anterior.
        estado
            .conn()
            .execute_batch("BEGIN TRANSACTION; INSERT INTO t VALUES (1); COMMIT;")
            .expect("debe poder abrirse una transacción nueva");
    }
}
