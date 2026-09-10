use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::config::SystemConfig;
use crate::session::{require_admin, require_auth, SessionState};

#[tauri::command]
pub fn get_all_config(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<SystemConfig>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

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
    let db = state.conn();

    db.query_row(
        "SELECT value FROM system_config WHERE key = ?1",
        params![key],
        |row| row.get(0),
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_config(state: State<DbState>, sessions: State<SessionState>, token: String, key: String, value: String) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    db.execute(
        "INSERT OR REPLACE INTO system_config (key, value, updated_at) VALUES (?1, ?2, datetime('now','localtime'))",
        params![key, value],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Clave donde se recuerda con qué versión se abrió la aplicación la última vez.
const CLAVE_VERSION: &str = "version_instalada";

/// Deja anotado en la bitácora cuándo la aplicación cambió de versión.
///
/// La tienda está lejos y de allá nadie va a avisar si una actualización entró o
/// no. Comparar la versión del binario contra la última que se vio detecta el
/// cambio **después** de que ocurrió, que es la única forma fiable: durante la
/// instalación el instalador de Windows mata la aplicación, así que cualquier
/// cosa que se escribiera antes de reiniciar podría no llegar a guardarse.
///
/// El renglón sale en el reporte de diagnóstico, que es lo que el encargado
/// puede mandar sin entender nada de lo que contiene.
pub fn registrar_version_instalada(conn: &rusqlite::Connection) {
    let actual = env!("CARGO_PKG_VERSION");

    let anterior: Option<String> = conn
        .query_row(
            "SELECT value FROM system_config WHERE key = ?1",
            params![CLAVE_VERSION],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    match anterior.as_deref() {
        Some(previa) if previa == actual => return,
        Some(previa) => {
            log::info!("Actualizada de {} a {}", previa, actual);
            conn.execute(
                "INSERT INTO app_logs (level, module, message) VALUES ('info', 'actualizacion', ?1)",
                params![format!("Actualizada de la versión {} a la {}", previa, actual)],
            ).ok();
        }
        None => {
            // Primer arranque, o una base de antes de que esto existiera.
            conn.execute(
                "INSERT INTO app_logs (level, module, message) VALUES ('info', 'actualizacion', ?1)",
                params![format!("Primer arranque registrado con la versión {}", actual)],
            ).ok();
        }
    }

    conn.execute(
        "INSERT OR REPLACE INTO system_config (key, value, description, updated_at)
         VALUES (?1, ?2, 'Versión con la que se abrió la aplicación por última vez', datetime('now','localtime'))",
        params![CLAVE_VERSION, actual],
    ).ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    fn bitacora(conn: &rusqlite::Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT message FROM app_logs WHERE module = 'actualizacion' ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| r.get(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap()
    }

    fn guardada(conn: &rusqlite::Connection) -> Option<String> {
        conn.query_row(
            "SELECT value FROM system_config WHERE key = ?1",
            params![CLAVE_VERSION],
            |r| r.get(0),
        ).ok()
    }

    #[test]
    fn el_primer_arranque_queda_anotado() {
        let conn = db();
        registrar_version_instalada(&conn);

        assert_eq!(bitacora(&conn).len(), 1);
        assert!(bitacora(&conn)[0].contains("Primer arranque"));
        assert_eq!(guardada(&conn).as_deref(), Some(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn abrir_otra_vez_la_misma_version_no_ensucia_la_bitacora() {
        // Se abre la caja todos los días; un renglón por arranque haría inútil
        // la bitácora justo cuando hay que leerla.
        let conn = db();
        for _ in 0..5 {
            registrar_version_instalada(&conn);
        }
        assert_eq!(bitacora(&conn).len(), 1);
    }

    #[test]
    fn un_cambio_de_version_queda_anotado_con_las_dos() {
        // Es lo que permite saber a distancia si la actualización de verdad entró.
        let conn = db();
        conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES (?1, '0.0.1')",
            params![CLAVE_VERSION],
        ).unwrap();

        registrar_version_instalada(&conn);

        let renglones = bitacora(&conn);
        assert_eq!(renglones.len(), 1);
        assert!(renglones[0].contains("0.0.1"), "falta la versión anterior: {}", renglones[0]);
        assert!(renglones[0].contains(env!("CARGO_PKG_VERSION")), "falta la nueva: {}", renglones[0]);
        assert_eq!(guardada(&conn).as_deref(), Some(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn una_version_guardada_en_blanco_se_trata_como_primer_arranque() {
        let conn = db();
        conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES (?1, '  ')",
            params![CLAVE_VERSION],
        ).unwrap();

        registrar_version_instalada(&conn);

        assert!(bitacora(&conn)[0].contains("Primer arranque"));
    }
}
