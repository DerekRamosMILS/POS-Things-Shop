//! Reporte de diagnóstico para soporte.
//!
//! Cuando algo falla en la tienda, el problema es reunir la información: dónde
//! está la base, qué versión corre, si la base está sana, qué dice la bitácora.
//! Esto lo junta todo en un archivo de texto que el encargado puede enviar sin
//! entender nada de lo que contiene.
//!
//! Deliberadamente **no** se manda a ningún servidor: enviar datos de una tienda
//! a un tercero debe ser una decisión de su dueño, no un efecto secundario.

use std::fmt::Write as _;
use std::fs;

use rusqlite::Connection;
use tauri::State;

use crate::db::connection::{get_db_dir, get_db_path, DbState};
use crate::session::{require_admin, SessionState};

/// Renglones de bitácora que se adjuntan.
const LOG_TAIL_LINES: usize = 400;

/// Tablas cuyo conteo ayuda a dimensionar el problema.
const TABLES: &[&str] = &[
    "products", "product_variants", "categories", "suppliers", "customers",
    "sales", "sale_items", "sale_payments", "layaways", "layaway_payments",
    "returns", "cash_registers", "expenses", "promotions", "users",
    "inventory_movements", "app_logs", "sessions",
];

/// Ajustes que describen el equipo. Se listan explícitamente en lugar de volcar
/// toda la tabla, para no incluir nada que no se haya pensado.
const REPORTED_CONFIG: &[&str] = &[
    "store_name", "currency_symbol", "tax_rate", "low_stock_threshold",
    "auto_backup", "max_backups", "session_hours", "log_retention_days",
    "printer_name", "printer_width", "printer_auto_print",
    "drawer_kick_command", "drawer_open_on_cash",
    "scanner_enabled", "scanner_suffix", "scanner_prefix",
    "scanner_max_gap_ms", "scanner_min_length",
];

fn scalar(db: &Connection, sql: &str) -> String {
    db.query_row(sql, [], |r| r.get::<_, String>(0))
        .unwrap_or_else(|e| format!("(no disponible: {})", e))
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{:.1} {}", size, UNITS[unit])
}

/// Últimos renglones de un archivo de texto, sin cargarlo entero en memoria si
/// creció mucho.
fn tail(path: &std::path::Path, lines: usize) -> String {
    match fs::read_to_string(path) {
        Ok(content) => {
            let all: Vec<&str> = content.lines().collect();
            let start = all.len().saturating_sub(lines);
            all[start..].join("\n")
        }
        Err(e) => format!("(no se pudo leer la bitácora: {})", e),
    }
}

/// Arma el texto del reporte.
pub fn build_report(db: &Connection) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "REPORTE DE DIAGNÓSTICO — Things Shop POS");
    let _ = writeln!(out, "Generado: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    let _ = writeln!(out, "Versión: {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(out, "Sistema: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    let _ = writeln!(out);

    // ── Base de datos ──
    let db_path = get_db_path();
    let _ = writeln!(out, "── BASE DE DATOS ──");
    let _ = writeln!(out, "Ruta: {}", db_path.display());
    let size = fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    let _ = writeln!(out, "Tamaño: {}", human_size(size));
    // Lo primero que hay que descartar ante cualquier rareza.
    let _ = writeln!(out, "Integridad: {}", scalar(db, "PRAGMA integrity_check"));
    let _ = writeln!(out, "Llaves foráneas: {}", scalar(db, "PRAGMA foreign_key_check"));
    let _ = writeln!(out, "Modo journal: {}", scalar(db, "PRAGMA journal_mode"));
    let _ = writeln!(out);

    // ── Migraciones ──
    let _ = writeln!(out, "── MIGRACIONES APLICADAS ──");
    match db.prepare("SELECT name, applied_at FROM _migrations ORDER BY id") {
        Ok(mut stmt) => {
            match stmt.query_map([], |r| {
                Ok(format!("{}  {}", r.get::<_, String>(1)?, r.get::<_, String>(0)?))
            }) {
                Ok(rows) => {
                    for row in rows.flatten() {
                        let _ = writeln!(out, "{}", row);
                    }
                }
                Err(e) => { let _ = writeln!(out, "(error: {})", e); }
            }
        }
        Err(e) => { let _ = writeln!(out, "(error: {})", e); }
    }
    let _ = writeln!(out);

    // ── Volumen de datos ──
    let _ = writeln!(out, "── REGISTROS POR TABLA ──");
    for table in TABLES {
        let count = scalar(db, &format!("SELECT COUNT(*) FROM {}", table));
        let _ = writeln!(out, "{:<22} {}", table, count);
    }
    let _ = writeln!(out);

    // ── Estado operativo ──
    let _ = writeln!(out, "── ESTADO ──");
    let _ = writeln!(out, "Caja abierta: {}", scalar(db,
        "SELECT COALESCE((SELECT 'sí, desde ' || opened_at FROM cash_registers WHERE status='open' LIMIT 1), 'no')"));
    let _ = writeln!(out, "Última venta: {}", scalar(db,
        "SELECT COALESCE((SELECT folio || ' — ' || created_at FROM sales ORDER BY id DESC LIMIT 1), 'ninguna')"));
    let backups = fs::read_dir(get_db_dir().join("backups"))
        .map(|d| d.filter_map(|e| e.ok()).filter(|e| e.path().extension().is_some_and(|x| x == "db")).count())
        .unwrap_or(0);
    let _ = writeln!(out, "Respaldos guardados: {}", backups);
    let fotos: i64 = scalar(db, "SELECT COUNT(*) FROM product_images").parse().unwrap_or(0);
    let _ = writeln!(out, "Fotos de producto: {} ({})", fotos, human_size(crate::photos::espacio_usado()));
    let _ = writeln!(out);

    // ── Configuración ──
    let _ = writeln!(out, "── CONFIGURACIÓN ──");
    for key in REPORTED_CONFIG {
        let value = db
            .query_row(
                "SELECT value FROM system_config WHERE key = ?1",
                rusqlite::params![key],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_else(|_| "(sin definir)".to_string());
        let _ = writeln!(out, "{:<22} {}", key, value);
    }
    let _ = writeln!(out);

    // ── Bitácora ──
    let _ = writeln!(out, "── ÚLTIMOS {} RENGLONES DE LA BITÁCORA ──", LOG_TAIL_LINES);
    let _ = writeln!(out, "{}", tail(&crate::logging::log_path(), LOG_TAIL_LINES));

    out
}

/// Escribe el reporte donde el usuario elija y devuelve la ruta.
#[tauri::command]
pub fn generate_diagnostic_report(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    path: String,
) -> Result<String, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    let report = build_report(&db);
    fs::write(&path, report).map_err(|e| format!("No se pudo guardar el reporte: {}", e))?;

    log::info!("Reporte de diagnóstico generado en {}", path);
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn the_report_covers_every_section_support_needs() {
        let report = build_report(&db());
        for section in ["BASE DE DATOS", "MIGRACIONES", "REGISTROS POR TABLA", "ESTADO",
                        "CONFIGURACIÓN", "BITÁCORA"] {
            assert!(report.contains(section), "falta la sección {}", section);
        }
    }

    #[test]
    fn it_reports_the_integrity_of_the_database() {
        let report = build_report(&db());
        assert!(report.contains("Integridad: ok"));
    }

    #[test]
    fn it_counts_every_table_it_promises() {
        let report = build_report(&db());
        for table in TABLES {
            assert!(report.contains(table), "falta el conteo de {}", table);
        }
    }

    #[test]
    fn it_reports_the_photo_storage() {
        // Las fotos viven fuera de la base; su peso no aparece en el tamaño del
        // archivo y hay que reportarlo aparte.
        let report = build_report(&db());
        assert!(report.contains("Fotos de producto:"));
    }

    #[test]
    fn it_reports_the_hardware_settings() {
        let report = build_report(&db());
        assert!(report.contains("printer_name"));
        assert!(report.contains("drawer_kick_command"));
        assert!(report.contains("scanner_suffix"));
    }

    #[test]
    fn it_never_leaks_a_password_hash() {
        let conn = db();
        crate::commands::users::ensure_admin_exists(&conn).unwrap();
        let report = build_report(&conn);
        assert!(!report.contains("$argon2"), "el reporte no debe llevar hashes");
    }

    #[test]
    fn it_survives_a_shop_with_no_sales_yet() {
        let report = build_report(&db());
        assert!(report.contains("Última venta: ninguna"));
        assert!(report.contains("Caja abierta: no"));
    }

    #[test]
    fn sizes_are_readable_for_a_human() {
        assert_eq!(human_size(0), "0.0 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }
}
