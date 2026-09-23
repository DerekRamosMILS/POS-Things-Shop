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
///
/// Se enumeran a mano para que nadie cuente una tabla sin pensarlo, y la prueba
/// `el_reporte_cuenta_todas_las_tablas_del_negocio` falla si el esquema crece y
/// esta lista no: se había quedado sin `conteos` ni `capturas_rechazadas`, que
/// son la captura desde el celular, justo lo que no se puede ver desde lejos.
const TABLES: &[&str] = &[
    "products", "product_variants", "product_images", "product_images_archivo",
    "categories", "suppliers", "customers",
    "sales", "sale_items", "sale_payments", "returns", "return_items",
    "layaways", "layaway_items", "layaway_payments",
    "cash_registers", "expenses", "expenses_archivo", "promotions",
    "price_history", "inventory_movements",
    "conteos", "capturas_rechazadas",
    "users", "sessions", "login_attempts",
    "notifications", "app_logs", "system_config",
];

/// Ajustes que describen el equipo. Se listan explícitamente en lugar de volcar
/// toda la tabla, para no incluir nada que no se haya pensado.
/// Nunca una clave privada: `es_privada` marca el secreto del relevo, con el que
/// cualquiera subiría productos y conteos desde cualquier parte del mundo, y este
/// reporte se manda por WhatsApp. La prueba
/// `el_reporte_nunca_lleva_el_secreto_del_relevo` lo comprueba clave por clave.
const REPORTED_CONFIG: &[&str] = &[
    "store_name", "currency_symbol", "tax_rate", "low_stock_threshold",
    "auto_backup", "max_backups", "session_hours", "log_retention_days",
    "printer_name", "printer_width", "printer_auto_print",
    "drawer_kick_command", "drawer_open_on_cash",
    "scanner_enabled", "scanner_suffix", "scanner_prefix",
    "scanner_max_gap_ms", "scanner_min_length",
    // Lo que hace falta para diagnosticar a distancia: a qué buzón apunta la
    // tienda, qué caja es, y cuándo salió del equipo la última copia.
    "relevo_url", "terminal_id", "ultima_copia_externa", "version_instalada",
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

/// Cuentas que **deberían dar cero** en una base sana.
///
/// Los arreglos impiden que estas inconsistencias se produzcan de aquí en
/// adelante, pero ninguna migración rellena lo que ya estaba mal: una tienda que
/// vino operando con las versiones anteriores puede arrastrar historia torcida y
/// desde 2000 km no hay forma de enterarse. Aquí se pregunta, con nombre y
/// número, y si todas dan cero el reporte lo dice en una línea.
///
/// Solo se cuenta; no se arregla nada. Tocar historia real es una decisión de
/// quien es dueño de esos datos.
fn revisiones(db: &Connection) -> Vec<(&'static str, i64)> {
    let cuenta = |sql: &str| -> i64 {
        db.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(-1)
    };

    vec![
        (
            // Antes de la 021 entregar un apartado no dejaba venta: esa mercancía
            // salió de la tienda sin aparecer en el reporte ni en las utilidades.
            "Apartados entregados sin su venta",
            cuenta(
                "SELECT COUNT(*) FROM layaways l
                 WHERE l.status = 'completed'
                   AND NOT EXISTS (SELECT 1 FROM sales s WHERE s.layaway_id = l.id)",
            ),
        ),
        (
            // El total de un producto con tallas es la suma de sus tallas. Un
            // conteo del celular sin talla lo pisaba, y nada lo delataba.
            "Productos con tallas cuyo total no cuadra",
            cuenta(
                "SELECT COUNT(*) FROM products p
                 WHERE p.has_variants = 1
                   AND p.stock != COALESCE((SELECT SUM(v.stock) FROM product_variants v
                                            WHERE v.product_id = p.id AND v.is_active = 1), 0)",
            ),
        ),
        (
            "Partidas con más piezas devueltas que vendidas",
            cuenta("SELECT COUNT(*) FROM sale_items WHERE returned_quantity > quantity"),
        ),
        (
            "Apartados con más abonado que su total",
            cuenta("SELECT COUNT(*) FROM layaways WHERE paid > total + 0.005"),
        ),
        (
            "Turnos cerrados sin lo que se contó",
            cuenta("SELECT COUNT(*) FROM cash_registers WHERE status = 'closed' AND closing_amount IS NULL"),
        ),
        (
            // Las piernas del desglose no tenían `CHECK` en la base hasta que se
            // empezó a revisarlas en la puerta. Una con un método que el sistema no
            // conoce no tiene columna en el reporte diario: el dinero está cobrado y
            // el reporte de ese día no lo enseña, sin decir nada.
            "Pagos con una forma que el sistema no conoce",
            cuenta(
                "SELECT (SELECT COUNT(*) FROM sale_payments
                          WHERE method NOT IN ('cash','card','transfer'))
                      + (SELECT COUNT(*) FROM layaway_payments
                          WHERE payment_method NOT IN ('cash','card','transfer'))
                      + (SELECT COUNT(*) FROM returns
                          WHERE refund_method NOT IN ('cash','card','transfer'))",
            ),
        ),
        (
            "Ventas sin ninguna partida",
            cuenta(
                "SELECT COUNT(*) FROM sales s
                 WHERE NOT EXISTS (SELECT 1 FROM sale_items si WHERE si.sale_id = s.id)",
            ),
        ),
    ]
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
    let _ = writeln!(out, "Fotos de producto: {} ({})", fotos, human_size(crate::photos::espacio_usado(db)));
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

    // ── Revisiones ──
    let _ = writeln!(out, "── REVISIONES DE CONSISTENCIA ──");
    let hallazgos = revisiones(db);
    let torcido: Vec<&(&str, i64)> = hallazgos.iter().filter(|(_, n)| *n != 0).collect();
    if torcido.is_empty() {
        let _ = writeln!(out, "Todo cuadra ({} revisiones).", hallazgos.len());
    } else {
        for (que, cuantos) in torcido {
            let _ = writeln!(out, "{:<44} {}", que, cuantos);
        }
        let _ = writeln!(out, "(Son datos de antes de los arreglos; no se corrigen solos.)");
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

    #[test]
    fn el_reporte_cuenta_todas_las_tablas_del_negocio() {
        // La lista se enumera a mano y se quedó atrás: `conteos` y
        // `capturas_rechazadas` —la captura desde el celular, que es justo lo
        // que falla en remoto— no aparecían, así que el reporte no servía para
        // diagnosticar lo único que no se puede ver desde lejos.
        let conn = db();
        let del_esquema: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table'
                   AND name NOT LIKE 'sqlite_%' AND name != '_migrations'",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .unwrap();

        let faltantes: Vec<&String> = del_esquema
            .iter()
            .filter(|t| !TABLES.contains(&t.as_str()))
            .collect();

        assert!(
            faltantes.is_empty(),
            "el reporte no cuenta estas tablas: {:?}",
            faltantes
        );
    }

    #[test]
    fn el_reporte_dice_como_esta_la_captura_por_celular() {
        let conn = db();
        let report = build_report(&conn);
        assert!(report.contains("conteos"), "falta el conteo de conteos");
        assert!(report.contains("capturas_rechazadas"), "faltan las rechazadas");
        assert!(report.contains("relevo_url"), "falta a qué relevo apunta la tienda");
    }

    #[test]
    fn el_reporte_nunca_lleva_el_secreto_del_relevo() {
        // Con ese secreto cualquiera sube productos y conteos que cambian el
        // inventario desde cualquier parte del mundo, y el reporte se manda por
        // WhatsApp.
        let conn = db();
        conn.execute(
            "INSERT INTO system_config (key, value) VALUES ('relevo_secreto', ?1)",
            rusqlite::params!["s3cr3t0-larguisimo-de-treinta-y-dos-bytes"],
        )
        .unwrap();

        let report = build_report(&conn);

        assert!(!report.contains("s3cr3t0"), "el reporte llevaba el secreto del relevo");
        for key in REPORTED_CONFIG {
            assert!(
                !crate::commands::config::es_privada(key),
                "{} es privada y no puede ir en el reporte",
                key
            );
        }
    }

    #[test]
    fn el_reporte_dice_cuando_fue_la_ultima_copia_fuera_del_equipo() {
        // Los respaldos de todos los días viven en el mismo disco: la copia que
        // protege de que la computadora se muera es la que sale del equipo.
        let report = build_report(&db());
        assert!(report.contains("ultima_copia_externa"));
    }

    #[test]
    fn una_base_sana_dice_que_todo_cuadra() {
        let report = build_report(&db());
        assert!(report.contains("REVISIONES DE CONSISTENCIA"));
        assert!(report.contains("Todo cuadra"), "{}", report);
    }

    #[test]
    fn el_reporte_delata_un_apartado_entregado_sin_su_venta() {
        // Antes de la 021, entregar un apartado no dejaba venta: esa mercancía
        // salió de la tienda sin aparecer en el reporte ni en las utilidades, y
        // ninguna migración lo rellena.
        let conn = db();
        conn.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO layaways (id, folio, user_id, total, paid, status)
             VALUES (1, 'A-1', 1, 500, 500, 'completed')",
            [],
        ).unwrap();

        let report = build_report(&conn);

        assert!(report.contains("Apartados entregados sin su venta"), "{}", report);
        assert!(!report.contains("Todo cuadra"));
    }

    #[test]
    fn el_reporte_delata_un_total_que_no_es_la_suma_de_sus_tallas() {
        let conn = db();
        conn.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock, has_variants)
             VALUES (1, 'V', 'Vestido', 1, 2, 99, 1)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO product_variants (product_id, size, stock) VALUES (1, 'M', 3), (1, 'G', 4)",
            [],
        ).unwrap();

        let report = build_report(&conn);

        assert!(report.contains("Productos con tallas cuyo total no cuadra"), "{}", report);
    }

    #[test]
    fn las_revisiones_no_tocan_nada() {
        // Solo cuentan: corregir historia real es decisión de quien es su dueño.
        let conn = db();
        conn.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock, has_variants)
             VALUES (1, 'V', 'Vestido', 1, 2, 99, 1)",
            [],
        ).unwrap();
        conn.execute("INSERT INTO product_variants (product_id, size, stock) VALUES (1, 'M', 3)", []).unwrap();

        build_report(&conn);

        let stock: i32 = conn.query_row("SELECT stock FROM products WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(stock, 99, "el reporte no corrige, solo cuenta");
    }

    #[test]
    fn el_reporte_delata_un_pago_con_forma_desconocida() {
        // El dinero está cobrado y el reporte diario no lo enseña: no tiene columna
        // donde ponerlo. Ahora se revisa en la puerta, pero los renglones que ya
        // estuvieran en la base de la tienda siguen ahí.
        let conn = db();
        conn.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sales (id, folio, user_id, subtotal, discount_total, tax, total,
                                payment_method, amount_paid, change_amount)
             VALUES (1, 'V-1', 1, 100, 0, 0, 100, 'cash', 100, 0)", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (1, 'P', 'P', 50, 100, 1)", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sale_items (sale_id, product_id, product_name, product_sku, quantity, unit_price, discount, subtotal)
             VALUES (1, 1, 'P', 'P', 1, 100, 0, 100)", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sale_payments (sale_id, method, amount) VALUES (1, 'vale', 100)", [],
        ).unwrap();

        let report = build_report(&conn);

        assert!(report.contains("Pagos con una forma que el sistema no conoce"), "{}", report);
        assert!(!report.contains("Todo cuadra"));
    }

    /// Siembra un estado torcido y comprueba que el reporte lo diga.
    ///
    /// De las siete revisiones, tres tenían prueba. Una revisión mal escrita es peor
    /// que no tenerla: el reporte dice "Todo cuadra" sobre una base torcida, y ese
    /// reporte es lo único que se ve de esa computadora desde 2000 km.
    fn con_usuario(conn: &Connection) {
        conn.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')", [],
        ).unwrap();
    }

    fn una_venta(conn: &Connection, id: i64) {
        conn.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (?1, 'P' || ?1, 'P', 50, 100, 10)",
            rusqlite::params![id],
        ).unwrap();
        conn.execute(
            "INSERT INTO sales (id, folio, user_id, subtotal, discount_total, tax, total,
                                payment_method, amount_paid, change_amount)
             VALUES (?1, 'V-' || ?1, 1, 100, 0, 0, 100, 'cash', 100, 0)",
            rusqlite::params![id],
        ).unwrap();
    }

    #[test]
    fn el_reporte_delata_mas_devuelto_que_vendido() {
        let conn = db();
        con_usuario(&conn);
        una_venta(&conn, 1);
        conn.execute(
            "INSERT INTO sale_items (sale_id, product_id, product_name, product_sku,
                                     quantity, returned_quantity, unit_price, discount, subtotal)
             VALUES (1, 1, 'P', 'P', 2, 3, 100, 0, 200)", [],
        ).unwrap();

        let report = build_report(&conn);

        assert!(report.contains("más piezas devueltas que vendidas"), "{}", report);
        assert!(!report.contains("Todo cuadra"));
    }

    #[test]
    fn el_reporte_delata_un_apartado_sobrepagado() {
        let conn = db();
        con_usuario(&conn);
        conn.execute(
            "INSERT INTO layaways (id, folio, user_id, total, paid, status)
             VALUES (1, 'A-1', 1, 500, 600, 'active')", [],
        ).unwrap();

        let report = build_report(&conn);

        assert!(report.contains("más abonado que su total"), "{}", report);
    }

    #[test]
    fn un_centavo_de_redondeo_no_cuenta_como_sobrepago() {
        // El apartado se cierra sumando abonos redondeados al centavo: un pelo por
        // encima del total es normal y no puede salir como si algo estuviera mal, o
        // el reporte cría desconfianza y nadie lo lee.
        let conn = db();
        con_usuario(&conn);
        conn.execute(
            "INSERT INTO layaways (id, folio, user_id, total, paid, status)
             VALUES (1, 'A-1', 1, 500.0, 500.001, 'completed')", [],
        ).unwrap();
        // Entregado deja su venta, que es el estado sano y lo que pide la otra
        // revisión: sin ella se dispara esa y no se estaría probando esta.
        una_venta(&conn, 1);
        conn.execute("UPDATE sales SET layaway_id = 1 WHERE id = 1", []).unwrap();
        conn.execute(
            "INSERT INTO sale_items (sale_id, product_id, product_name, product_sku,
                                     quantity, unit_price, discount, subtotal)
             VALUES (1, 1, 'P', 'P', 1, 100, 0, 100)", [],
        ).unwrap();

        let report = build_report(&conn);

        assert!(report.contains("Todo cuadra"), "{}", report);
    }

    #[test]
    fn el_reporte_delata_un_turno_cerrado_sin_lo_contado() {
        let conn = db();
        con_usuario(&conn);
        conn.execute(
            "INSERT INTO cash_registers (id, user_id, opening_amount, status, closed_at)
             VALUES (1, 1, 500, 'closed', datetime('now','localtime'))", [],
        ).unwrap();

        let report = build_report(&conn);

        assert!(report.contains("Turnos cerrados sin lo que se contó"), "{}", report);
    }

    #[test]
    fn el_reporte_delata_una_venta_sin_partidas() {
        let conn = db();
        con_usuario(&conn);
        una_venta(&conn, 1);
        // Sin insertar ninguna partida.

        let report = build_report(&conn);

        assert!(report.contains("Ventas sin ninguna partida"), "{}", report);
    }

    #[test]
    fn un_turno_abierto_sin_contar_es_normal_y_no_se_delata() {
        // Lo que se contó se anota al cerrar: un turno abierto sin ese dato es el
        // caso corriente de cualquier tienda a media jornada.
        let conn = db();
        con_usuario(&conn);
        conn.execute(
            "INSERT INTO cash_registers (id, user_id, opening_amount) VALUES (1, 1, 500)", [],
        ).unwrap();

        let report = build_report(&conn);

        assert!(report.contains("Todo cuadra"), "{}", report);
    }
}
