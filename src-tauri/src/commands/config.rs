use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::config::SystemConfig;
use crate::session::{require_admin, require_auth, SessionState};

/// Claves que no salen por la configuración genérica.
///
/// `get_all_config` y `get_config` las puede leer cualquiera con sesión, cajeras
/// incluidas. El secreto del relevo deja subir productos y conteos que cambian
/// el inventario desde cualquier parte del mundo; por esta puerta se lo llevaba
/// cualquiera, y el candado de administrador de `relevo_estado` no servía de nada.
fn es_privada(key: &str) -> bool {
    key.trim().starts_with("relevo_secreto")
}

pub(crate) fn configuracion_publica(db: &rusqlite::Connection) -> Result<Vec<SystemConfig>, String> {
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

    Ok(configs.into_iter().filter(|c| !es_privada(&c.key)).collect())
}

/// Revisa los ajustes numéricos antes de guardarlos.
///
/// Quien los lee cae a un valor por omisión cuando no entiende lo guardado, así
/// que un "abc" o un "-5" no rompía nada a la vista: simplemente se ignoraba sin
/// avisar, y un umbral negativo de stock bajo apagaba las alertas. Vacío sí se
/// acepta: significa "el de siempre".
fn validar_valor(key: &str, value: &str) -> Result<String, String> {
    let v = value.trim();
    let rango: Option<(f64, f64, &str)> = match key {
        "tax_rate" => Some((0.0, 100.0, "La tasa de impuesto va de 0 a 100")),
        "low_stock_threshold" => Some((0.0, 1_000_000.0, "El umbral de stock bajo no puede ser negativo")),
        "session_hours" => Some((1.0, 24.0 * 30.0, "La sesión dura de 1 hora a 30 días")),
        "max_backups" => Some((1.0, 1000.0, "Hay que conservar al menos un respaldo")),
        "log_retention_days" => Some((1.0, 3650.0, "La bitácora se conserva de 1 día a 10 años")),
        "scanner_min_length" => Some((1.0, 100.0, "La longitud mínima del código va de 1 a 100")),
        "scanner_max_gap_ms" => Some((1.0, 5000.0, "El tiempo entre teclas del lector va de 1 a 5000 ms")),
        _ => None,
    };
    let Some((min, max, mensaje)) = rango else {
        return Ok(value.to_string());
    };
    if v.is_empty() {
        return Ok(String::new());
    }
    let n: f64 = v.parse().map_err(|_| format!("{}: '{}' no es un número", mensaje, v))?;
    // Los que no son la tasa se leen como enteros.
    let entero = key != "tax_rate";
    if !n.is_finite() || n < min || n > max || (entero && n.fract() != 0.0) {
        return Err(format!("{} (se recibió '{}')", mensaje, v));
    }
    Ok(v.to_string())
}

#[tauri::command]
pub fn get_all_config(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<SystemConfig>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();
    configuracion_publica(&db)
}

#[tauri::command]
pub fn get_config(state: State<DbState>, sessions: State<SessionState>, token: String, key: String) -> Result<String, String> {
    require_auth(&sessions, &token)?;
    if es_privada(&key) {
        return Err("Ese valor no se puede consultar".to_string());
    }
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
    if es_privada(&key) {
        return Err("Ese valor no se puede cambiar desde aquí".to_string());
    }
    let value = validar_valor(&key, &value)?;
    let db = state.conn();

    db.execute(
        "INSERT OR REPLACE INTO system_config (key, value, updated_at) VALUES (?1, ?2, datetime('now','localtime'))",
        params![key, value],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Guarda varios ajustes juntos: todos o ninguno.
///
/// La pantalla los mandaba uno por uno; si el tercero no pasaba la validación,
/// los dos primeros ya estaban guardados y el mensaje no decía cuáles.
pub(crate) fn guardar_ajustes(db: &rusqlite::Connection, cambios: &[(String, String)]) -> Result<(), String> {
    let mut limpios = Vec::with_capacity(cambios.len());
    for (key, value) in cambios {
        if es_privada(key) {
            return Err("Ese valor no se puede cambiar desde aquí".to_string());
        }
        limpios.push((key.as_str(), validar_valor(key, value)?));
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let resultado = (|| -> Result<(), String> {
        for (key, value) in &limpios {
            db.execute(
                "INSERT INTO system_config (key, value, updated_at) VALUES (?1, ?2, datetime('now','localtime'))
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![key, value],
            ).map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    match resultado {
        Ok(()) => crate::db::connection::confirmar(db),
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

#[tauri::command]
pub fn set_configs(state: State<DbState>, sessions: State<SessionState>, token: String, cambios: Vec<(String, String)>) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    guardar_ajustes(&db, &cambios)
}

/// Deja un renglón en la bitácora sobre lo que hizo el actualizador.
///
/// Sin esto, una actualización que no llega es indistinguible de una que llegó y
/// se quedó esperando, y de una que falló al descargar. Las tres se ven igual
/// desde lejos: nada. Con la tienda a 2000 km y sin nadie técnico enfrente, eso
/// convierte cualquier problema en una conversación de horas.
///
/// Sale en el reporte de diagnóstico junto al resto de la bitácora.
#[tauri::command]
pub fn registrar_evento_actualizacion(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    mensaje: String,
) -> Result<(), String> {
    let user_id = require_auth(&sessions, &token)?;
    let db = state.conn();

    // Se recorta: el mensaje puede traer el error del sistema, que a veces es una
    // página entera, y la bitácora tiene que seguir siendo legible.
    let mensaje: String = mensaje.chars().take(400).collect();
    db.execute(
        "INSERT INTO app_logs (level, module, message, user_id) VALUES ('info', 'actualizacion', ?1, ?2)",
        params![mensaje, user_id],
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
    fn un_mensaje_larguisimo_no_hace_ilegible_la_bitacora() {
        // El error del sistema a veces es una página entera.
        let recortado: String = "x".repeat(5000).chars().take(400).collect();
        assert_eq!(recortado.chars().count(), 400);
    }

    #[test]
    fn el_secreto_del_relevo_no_sale_por_la_configuracion() {
        let conn = db();
        let secreto = crate::capture::relevo::secreto_de_la_tienda(&conn).unwrap();
        conn.execute(
            "INSERT INTO system_config (key, value) VALUES ('relevo_secreto_anterior', 'otro-secreto-cualquiera')",
            [],
        ).unwrap();

        let todo = configuracion_publica(&conn).unwrap();

        assert!(!todo.is_empty(), "lo demás sí tiene que salir");
        assert!(todo.iter().all(|c| c.value != secreto && !c.key.starts_with("relevo_secreto")));
        assert!(es_privada("relevo_secreto") && es_privada(" relevo_secreto_anterior"));
        assert!(!es_privada("relevo_url") && !es_privada("currency_symbol"));
    }

    fn valor(conn: &rusqlite::Connection, key: &str) -> String {
        conn.query_row("SELECT value FROM system_config WHERE key = ?1", params![key], |r| r.get(0)).unwrap()
    }

    #[test]
    fn un_ajuste_invalido_no_deja_guardados_los_demas() {
        let conn = db();
        let antes = valor(&conn, "store_name");

        let err = guardar_ajustes(&conn, &[
            ("store_name".into(), "Otra tienda".into()),
            ("tax_rate".into(), "16".into()),
            ("low_stock_threshold".into(), "-5".into()),
        ]).unwrap_err();

        assert!(err.contains("stock bajo"), "{}", err);
        assert_eq!(valor(&conn, "store_name"), antes, "nada se guardó");
        assert_eq!(valor(&conn, "tax_rate"), "0");
    }

    #[test]
    fn los_ajustes_validos_se_guardan_juntos() {
        let conn = db();
        guardar_ajustes(&conn, &[
            ("store_name".into(), "Things".into()),
            ("tax_rate".into(), " 16 ".into()),
        ]).unwrap();
        assert_eq!(valor(&conn, "store_name"), "Things");
        assert_eq!(valor(&conn, "tax_rate"), "16");
    }

    #[test]
    fn los_ajustes_numericos_imposibles_se_rechazan() {
        for (k, v) in [
            ("low_stock_threshold", "-5"), ("tax_rate", "150"), ("tax_rate", "-1"),
            ("session_hours", "0"), ("max_backups", "0"), ("log_retention_days", "abc"),
            ("scanner_min_length", "2.5"), ("tax_rate", "NaN"),
        ] {
            assert!(validar_valor(k, v).is_err(), "aceptó {} = {}", k, v);
        }
        assert_eq!(validar_valor("tax_rate", " 16 ").unwrap(), "16");
        assert_eq!(validar_valor("tax_rate", "8.5").unwrap(), "8.5");
        assert_eq!(validar_valor("low_stock_threshold", "0").unwrap(), "0");
        assert_eq!(validar_valor("max_backups", "").unwrap(), "", "vacío es el de siempre");
        assert_eq!(validar_valor("store_name", "  Things  ").unwrap(), "  Things  ", "lo demás pasa tal cual");
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
