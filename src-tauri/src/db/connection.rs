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

/// Cierra una transacción y, si el cierre falla, la deshace.
///
/// `COMMIT` puede fallar —el disco lleno es el caso real— y cuando falla la
/// transacción **sigue abierta**: la siguiente operación moría con "cannot start
/// a transaction within a transaction" y la caja quedaba inservible hasta
/// reiniciar. Deshacerla pierde esa operación, que de todos modos no se guardó,
/// y deja la base lista para la siguiente.
pub fn confirmar(db: &Connection) -> Result<(), String> {
    match db.execute_batch("COMMIT;") {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = db.execute_batch("ROLLBACK;");
            Err(e.to_string())
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

/// Junto al preparado: cuándo se preparó, en segundos desde 1970.
pub const PENDING_RESTORE_CUANDO: &str = "things_shop.db.restore-pending.cuando";

/// Una restauración preparada hace más que esto ya no se aplica.
///
/// La pantalla reinicia la aplicación en cuanto la prepara. Si el preparado sigue
/// ahí mucho después, el reinicio no ocurrió y la tienda siguió vendiendo:
/// aplicarlo en el siguiente arranque —días después, o cuando se instale una
/// actualización— dejaba fuera de la vista todo lo vendido entretanto.
pub const VIGENCIA_RESTAURACION_SEGUNDOS: i64 = 15 * 60;

/// Comprueba que un archivo sirve como respaldo de esta aplicación.
///
/// Sin esto solo se miraba el nombre. Un archivo dañado o que no era una base se
/// preparaba igual, se ponía en su lugar al arrancar y entonces `init_db` fallaba:
/// la aplicación abría un aviso de error y se cerraba, en cada arranque, con la
/// tienda parada y la única salida por línea de comandos a 2000 km. Se revisa
/// aquí, que es donde todavía hay alguien enfrente a quien decírselo.
pub fn revisar_respaldo(archivo: &std::path::Path) -> Result<(), String> {
    revisar(archivo).map_err(|_| {
        "Ese archivo no se puede leer como base de datos: está dañado o no es un respaldo.".to_string()
    })?;

    let conn = Connection::open(archivo).map_err(|e| e.to_string())?;
    let migraciones: i64 = conn
        .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
        .map_err(|_| {
            "Ese archivo es una base de datos, pero no de Things Shop.".to_string()
        })?;
    if migraciones == 0 {
        return Err("Ese respaldo está vacío: no tiene nada que restaurar.".to_string());
    }
    Ok(())
}

/// Deja un respaldo preparado para aplicarse al reiniciar, con su hora.
pub fn preparar_restauracion(db_dir: &std::path::Path, origen: &std::path::Path) -> Result<(), String> {
    fs::copy(origen, db_dir.join(PENDING_RESTORE))
        .map_err(|e| format!("Error al preparar restauración: {}", e))?;
    fs::write(db_dir.join(PENDING_RESTORE_CUANDO), chrono::Utc::now().timestamp().to_string())
        .map_err(|e| format!("Error al preparar restauración: {}", e))
}

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

    let marca = pending.with_file_name(PENDING_RESTORE_CUANDO);
    let preparada = fs::read_to_string(&marca).ok().and_then(|t| t.trim().parse::<i64>().ok());
    let vigente = preparada
        .map(|t| (chrono::Utc::now().timestamp() - t).abs() <= VIGENCIA_RESTAURACION_SEGUNDOS)
        .unwrap_or(false);
    if !vigente {
        let apartada = pending.with_file_name(format!(
            "things_shop.db.restauracion-no-aplicada_{}",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        ));
        let _ = fs::rename(&pending, &apartada);
        let _ = fs::remove_file(&marca);
        log::error!(
            "Había una restauración preparada que no se aplicó a tiempo; se apartó en {:?} sin tocar la base",
            apartada
        );
        return;
    }

    // Antes de pisar la base se guarda entera. Restaurar un respaldo de hace una
    // semana por error borraba la semana: sin copia no había forma de volver. Si
    // la copia no se puede hacer, no se restaura.
    match guardar_antes_de_restaurar(db_path) {
        Ok(Some(copia)) => log::info!("Base actual guardada antes de restaurar en {:?}", copia),
        Ok(None) => {}
        Err(e) => {
            log::error!("No se restauró el respaldo: no se pudo guardar antes la base actual: {}", e);
            return;
        }
    }

    // Remove WAL/SHM sidecars tied to the old database file.
    for ext in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", db_path.to_string_lossy(), ext));
        let _ = fs::remove_file(sidecar);
    }
    // Se copia a un archivo aparte y se renombra encima. Renombrar es atómico
    // dentro del mismo disco: o queda la base vieja entera o la nueva entera,
    // nunca una copia a medias porque se acabó el espacio o se fue la luz.
    let temporal = PathBuf::from(format!("{}.restaurando", db_path.to_string_lossy()));
    if let Err(e) = fs::copy(&pending, &temporal) {
        log::error!("Failed to apply pending restore: {}", e);
        let _ = fs::remove_file(&temporal);
        return;
    }
    match fs::rename(&temporal, db_path) {
        Ok(_) => {
            let _ = fs::remove_file(&pending);
            let _ = fs::remove_file(&marca);
            log::info!("Restored database from staged backup");
        }
        Err(e) => {
            log::error!("Failed to apply pending restore: {}", e);
            let _ = fs::remove_file(&temporal);
        }
    }
}

/// Con esto empieza el nombre de la copia que se guarda antes de restaurar. La
/// rotación de respaldos no la toca nunca.
pub const PREFIJO_ANTES_DE_RESTAURAR: &str = "antes-de-restaurar_";

/// Deja en `destino` una copia consistente de la base abierta, ya revisada.
///
/// Con `VACUUM INTO` y no copiando el archivo. Copiar el `.db` se lleva solo lo
/// que ya bajó a él y deja fuera lo que todavía vive en el `-wal`, que es
/// justamente lo último que se vendió. Un `wal_checkpoint` antes de copiar no
/// alcanza: cuando no puede, lo dice en un renglón y no con un error, así que el
/// aviso se pierde y la copia sale incompleta sin que nadie se entere.
///
/// La copia nace además compactada, y se revisa antes de darla por buena: un
/// respaldo dañado tiene que doler hoy, no el día que hace falta.
pub fn copia_consistente(db: &Connection, destino: &std::path::Path) -> Result<(), String> {
    db.execute("VACUUM INTO ?1", [destino.to_string_lossy().to_string()])
        .map_err(|e| e.to_string())?;
    revisar(destino)
}

/// Abre un archivo de base y comprueba que está sano.
pub fn revisar(archivo: &std::path::Path) -> Result<(), String> {
    let revision: String = Connection::open(archivo)
        .and_then(|c| c.query_row("PRAGMA quick_check", [], |r| r.get(0)))
        .map_err(|e| e.to_string())?;
    if revision != "ok" {
        return Err(format!("la copia salió dañada: {}", revision));
    }
    Ok(())
}

/// Guarda la base actual en `backups/` antes de que un respaldo la reemplace.
///
/// Con `VACUUM INTO` y no copiando el archivo: lo último que se vendió puede
/// estar todavía en el `-wal`, que la restauración borra enseguida. Copiar solo
/// el `.db` dejaba fuera justo eso.
fn guardar_antes_de_restaurar(db_path: &std::path::Path) -> Result<Option<PathBuf>, String> {
    if !db_path.exists() {
        return Ok(None);
    }
    let carpeta = db_path.parent().ok_or("La base no tiene carpeta")?.join("backups");
    fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;

    let sello = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let mut destino = carpeta.join(format!("{}{}.db", PREFIJO_ANTES_DE_RESTAURAR, sello));
    let mut n = 1;
    while destino.exists() {
        n += 1;
        destino = carpeta.join(format!("{}{}_{}.db", PREFIJO_ANTES_DE_RESTAURAR, sello, n));
    }

    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    copia_consistente(&conn, &destino)?;
    Ok(Some(destino))
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

    /// Una transacción cuyo `COMMIT` falla tiene que quedar deshecha, no abierta.
    #[test]
    fn si_el_cierre_falla_la_transaccion_no_se_queda_abierta() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE dueños (id INTEGER PRIMARY KEY);
             CREATE TABLE cosas (id INTEGER PRIMARY KEY, dueño INTEGER
                 REFERENCES dueños(id) DEFERRABLE INITIALLY DEFERRED);",
        ).unwrap();

        // Una llave foránea diferida revienta justo al cerrar, como el disco lleno.
        db.execute_batch("BEGIN TRANSACTION;").unwrap();
        db.execute("INSERT INTO cosas (id, dueño) VALUES (1, 99)", []).unwrap();
        assert!(confirmar(&db).is_err());

        // Lo importante: la base sigue usable y no quedó nada a medias.
        db.execute_batch("BEGIN TRANSACTION;").expect("la transacción anterior quedó abierta");
        db.execute("INSERT INTO dueños (id) VALUES (99)", []).unwrap();
        confirmar(&db).unwrap();
        let cosas: i64 = db.query_row("SELECT COUNT(*) FROM cosas", [], |r| r.get(0)).unwrap();
        assert_eq!(cosas, 0, "lo que no se pudo cerrar no se guardó");
    }

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
