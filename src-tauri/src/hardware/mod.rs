//! Integración con el hardware del mostrador: impresora de tickets y cajón.
//!
//! El diseño evita depender de marcas concretas: los tickets se emiten en
//! ESC/POS (el lenguaje común de las impresoras térmicas), los bytes van al
//! spooler del sistema en modo RAW (funciona con cualquier driver instalado) y
//! el comando que abre el cajón es configurable, porque es lo único que varía
//! de verdad entre modelos.

pub mod escpos;
pub mod spooler;

use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::session::{require_admin, require_auth, SessionState};

/// Lee un valor de `system_config`, con respaldo cuando falta o está vacío.
fn config(db: &rusqlite::Connection, key: &str, default: &str) -> String {
    db.query_row(
        "SELECT value FROM system_config WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
    .unwrap_or_else(|| default.to_string())
}

fn config_flag(db: &rusqlite::Connection, key: &str, default: bool) -> bool {
    matches!(
        config(db, key, if default { "1" } else { "0" }).as_str(),
        "1" | "true" | "si" | "sí"
    )
}

/// Ajustes de la impresora tal como quedaron configurados.
struct PrinterSetup {
    name: String,
    width: usize,
}

fn printer_setup(db: &rusqlite::Connection) -> PrinterSetup {
    let width = config(db, "printer_width", "32")
        .parse::<usize>()
        .ok()
        .filter(|w| (20..=64).contains(w))
        .unwrap_or(escpos::DEFAULT_WIDTH);

    PrinterSetup { name: config(db, "printer_name", ""), width }
}

fn money(symbol: &str, amount: f64) -> String {
    format!("{}{:.2}", symbol, amount)
}

/// Bytes que abren el cajón, según el comando configurado.
fn drawer_bytes(db: &rusqlite::Connection) -> Result<Vec<u8>, String> {
    let spec = config(db, "drawer_kick_command", escpos::DEFAULT_DRAWER_KICK);
    escpos::parse_hex_command(&spec).map_err(|e| {
        format!("El comando de apertura del cajón no es válido ({}). Revísalo en Ajustes.", e)
    })
}

// ─── Comandos expuestos al frontend ──────────────────────────────────────────

/// Impresoras instaladas, para el selector de Ajustes.
#[tauri::command]
pub fn list_printers(sessions: State<SessionState>, token: String) -> Result<Vec<String>, String> {
    require_admin(&sessions, &token)?;
    spooler::list_printers()
}

/// Abre el cajón de dinero. El cajón cuelga de la impresora, así que esto es un
/// trabajo de impresión que solo contiene el comando de apertura.
#[tauri::command]
pub fn open_cash_drawer(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<(), String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let setup = printer_setup(&db);
    if setup.name.is_empty() {
        return Err(
            "No hay impresora configurada. El cajón se abre a través de la impresora de tickets."
                .to_string(),
        );
    }

    spooler::print_raw(&setup.name, "Abrir cajon", &drawer_bytes(&db)?)
}

/// Imprime un ticket de prueba y, opcionalmente, abre el cajón. Es lo que se usa
/// al conectar hardware nuevo para confirmar que todo responde.
#[tauri::command]
pub fn test_printer(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    printer: Option<String>,
    open_drawer: bool,
) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let setup = printer_setup(&db);
    let target = printer.filter(|p| !p.trim().is_empty()).unwrap_or(setup.name);
    if target.trim().is_empty() {
        return Err("Elige una impresora antes de probar".to_string());
    }

    let mut b = escpos::Builder::new(setup.width);
    b.align_center().bold(true).line("PRUEBA DE IMPRESION").bold(false);
    b.line(&config(&db, "store_name", "Things Shop"));
    b.align_left().separator();
    b.pair("Ancho del papel", &format!("{} car.", setup.width));
    b.pair("Impresora", &target);
    b.line("Acentos: Nino, Cafe, Pantalon");
    b.separator();
    b.align_center().line("Si lees esto, la impresora");
    b.line("esta lista para cobrar.");
    b.feed(3).cut();

    if open_drawer {
        b.raw(&drawer_bytes(&db)?);
    }

    spooler::print_raw(&target, "Prueba Things Shop", &b.finish())
}

/// Imprime el ticket de una venta ya registrada.
///
/// Se arma desde lo que quedó guardado en la base, no desde lo que el frontend
/// tenga en pantalla: el papel dice exactamente lo que se cobró.
#[tauri::command]
pub fn print_sale_receipt(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    sale_id: i64,
    open_drawer: Option<bool>,
) -> Result<(), String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let setup = printer_setup(&db);
    if setup.name.is_empty() {
        return Err("SIN_IMPRESORA".to_string());
    }

    let symbol = config(&db, "currency_symbol", "$");
    let width = setup.width;

    let (folio, subtotal, discount_total, tax, total, payment_method, amount_paid, change, created_at, cashier):
        (String, f64, f64, f64, f64, String, f64, f64, String, Option<String>) = db
        .query_row(
            "SELECT s.folio, s.subtotal, s.discount_total, s.tax, s.total, s.payment_method,
                    s.amount_paid, s.change_amount, s.created_at, u.full_name
             FROM sales s LEFT JOIN users u ON s.user_id = u.id
             WHERE s.id = ?1",
            params![sale_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)),
        )
        .map_err(|_| "No se encontró la venta".to_string())?;

    let mut b = escpos::Builder::new(width);

    // Encabezado de la tienda
    b.align_center().bold(true).line(&config(&db, "store_name", "Things Shop")).bold(false);
    for key in ["store_address", "store_phone"] {
        let value = config(&db, key, "");
        if !value.is_empty() {
            for line in escpos::wrap(&value, width) {
                b.line(&line);
            }
        }
    }

    b.align_left().separator();
    b.pair("Folio", &folio);
    b.pair("Fecha", &created_at);
    if let Some(name) = cashier {
        b.pair("Atendio", &name);
    }
    b.separator();

    // Partidas
    let mut stmt = db
        .prepare(
            "SELECT product_name, variant_label, quantity, unit_price, discount, subtotal
             FROM sale_items WHERE sale_id = ?1 ORDER BY id",
        )
        .map_err(|e| e.to_string())?;

    let items = stmt
        .query_map(params![sale_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i32>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    for (name, variant, qty, unit_price, discount, line_total) in items {
        let title = match variant {
            Some(v) if !v.is_empty() => format!("{} ({})", name, v),
            _ => name,
        };
        for line in escpos::wrap(&title, width) {
            b.line(&line);
        }
        b.pair(
            &format!("  {} x {}", qty, money(&symbol, unit_price)),
            &money(&symbol, line_total),
        );
        if discount > 0.0 {
            b.pair("  Descuento", &format!("-{}", money(&symbol, discount)));
        }
    }

    b.separator();
    b.pair("Subtotal", &money(&symbol, subtotal));
    if discount_total > 0.0 {
        b.pair("Descuentos", &format!("-{}", money(&symbol, discount_total)));
    }
    if tax > 0.0 {
        b.pair("Impuesto", &money(&symbol, tax));
    }

    b.bold(true).double_size(true);
    b.pair("TOTAL", &money(&symbol, total));
    b.double_size(false).bold(false);

    // Desglose del cobro
    let mut stmt = db
        .prepare("SELECT method, amount FROM sale_payments WHERE sale_id = ?1 ORDER BY id")
        .map_err(|e| e.to_string())?;
    let legs = stmt
        .query_map(params![sale_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    b.separator();
    if legs.len() > 1 {
        for (method, amount) in &legs {
            b.pair(method_label(method), &money(&symbol, *amount));
        }
    } else {
        b.pair("Forma de pago", method_label(&payment_method));
    }
    if change > 0.0 {
        b.pair("Recibido", &money(&symbol, amount_paid));
        b.pair("Cambio", &money(&symbol, change));
    }

    // Pie
    let footer = config(&db, "ticket_footer", "");
    if !footer.is_empty() {
        b.align_center().line("");
        for line in escpos::wrap(&footer, width) {
            b.line(&line);
        }
    }
    b.feed(3).cut();

    // El cajón solo se abre si entró efectivo: no tiene sentido abrirlo en una
    // venta con tarjeta.
    // Un pago mixto puede traer efectivo aunque `payment_method` diga "mixed",
    // así que hay que mirar el desglose además del método principal.
    let cash_involved =
        legs.iter().any(|(m, _)| m == "cash") || payment_method == "cash";
    let should_open = open_drawer.unwrap_or_else(|| {
        config_flag(&db, "drawer_open_on_cash", true) && cash_involved
    });
    if should_open {
        b.raw(&drawer_bytes(&db)?);
    }

    spooler::print_raw(&setup.name, &format!("Ticket {}", folio), &b.finish())
}

/// Comprobante de un apartado: lo que el cliente se lleva con su saldo.
///
/// Se imprime al crearlo y en cada abono, porque el saldo pendiente es
/// justamente el dato que el cliente necesita conservar.
#[tauri::command]
pub fn print_layaway_receipt(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    layaway_id: i64,
) -> Result<(), String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let setup = printer_setup(&db);
    if setup.name.is_empty() {
        return Err("SIN_IMPRESORA".to_string());
    }

    let symbol = config(&db, "currency_symbol", "$");
    let width = setup.width;

    let (folio, total, paid, status, due_date, created_at, customer):
        (String, f64, f64, String, Option<String>, String, Option<String>) = db
        .query_row(
            "SELECT l.folio, l.total, l.paid, l.status, l.due_date, l.created_at, c.name
             FROM layaways l LEFT JOIN customers c ON l.customer_id = c.id
             WHERE l.id = ?1",
            params![layaway_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )
        .map_err(|_| "No se encontró el apartado".to_string())?;

    let balance = crate::money::Cents::from_pesos(total) - crate::money::Cents::from_pesos(paid);

    let mut b = escpos::Builder::new(width);
    b.align_center().bold(true).line(&config(&db, "store_name", "Things Shop")).bold(false);
    b.line("COMPROBANTE DE APARTADO");
    b.align_left().separator();
    b.pair("Folio", &folio);
    b.pair("Fecha", &created_at);
    if let Some(name) = customer {
        b.pair("Cliente", &name);
    }
    if let Some(due) = due_date {
        b.pair("Vence", &due);
    }
    b.separator();

    let mut stmt = db
        .prepare(
            "SELECT product_name, variant_label, quantity, subtotal
             FROM layaway_items WHERE layaway_id = ?1 ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let items = stmt
        .query_map(params![layaway_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i32>(2)?,
                r.get::<_, f64>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    for (name, variant, qty, line_total) in items {
        let title = match variant {
            Some(v) if !v.is_empty() => format!("{} ({})", name, v),
            _ => name,
        };
        for line in escpos::wrap(&title, width) {
            b.line(&line);
        }
        b.pair(&format!("  {} pz", qty), &money(&symbol, line_total));
    }

    b.separator();
    b.pair("Total del apartado", &money(&symbol, total));
    b.pair("Abonado", &money(&symbol, paid));

    b.bold(true).double_size(true);
    b.pair("RESTA", &money(&symbol, balance.to_pesos()));
    b.double_size(false).bold(false);

    // Historial de abonos: el cliente puede cotejar lo que ya entregó.
    let mut stmt = db
        .prepare(
            "SELECT created_at, amount, payment_method FROM layaway_payments
             WHERE layaway_id = ?1 ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let payments = stmt
        .query_map(params![layaway_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, String>(2)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    if !payments.is_empty() {
        b.separator().line("Abonos");
        for (date, amount, method) in payments {
            b.pair(
                &format!("{} {}", date.chars().take(10).collect::<String>(), method_label(&method)),
                &money(&symbol, amount),
            );
        }
    }

    if status == "completed" {
        b.separator().align_center().bold(true).line("LIQUIDADO").bold(false).align_left();
    }

    let footer = config(&db, "ticket_footer", "");
    if !footer.is_empty() {
        b.align_center().line("");
        for line in escpos::wrap(&footer, width) {
            b.line(&line);
        }
    }
    b.feed(3).cut();

    spooler::print_raw(&setup.name, &format!("Apartado {}", folio), &b.finish())
}

fn method_label(method: &str) -> &'static str {
    match method {
        "cash" => "Efectivo",
        "card" => "Tarjeta",
        "transfer" => "Transferencia",
        "mixed" => "Pago mixto",
        _ => "Otro",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn printing_without_a_configured_printer_is_refused_up_front() {
        assert!(spooler::print_raw("", "x", b"hola").is_err());
        assert!(spooler::print_raw("Epson", "x", b"").is_err());
    }

    #[test]
    fn the_paper_width_falls_back_when_the_setting_is_nonsense() {
        let conn = db();
        conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES ('printer_width', 'ancho')",
            [],
        )
        .unwrap();
        assert_eq!(printer_setup(&conn).width, escpos::DEFAULT_WIDTH);

        conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES ('printer_width', '999')",
            [],
        )
        .unwrap();
        assert_eq!(printer_setup(&conn).width, escpos::DEFAULT_WIDTH);
    }

    #[test]
    fn an_80mm_paper_width_is_honoured() {
        let conn = db();
        conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES ('printer_width', '48')",
            [],
        )
        .unwrap();
        assert_eq!(printer_setup(&conn).width, 48);
    }

    #[test]
    fn the_drawer_falls_back_to_the_standard_command() {
        let conn = db();
        assert_eq!(drawer_bytes(&conn).unwrap(), vec![0x1B, 0x70, 0x00, 0x19, 0xFA]);
    }

    #[test]
    fn a_pin_5_drawer_can_be_configured() {
        let conn = db();
        conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES ('drawer_kick_command', '1B 70 01 19 FA')",
            [],
        )
        .unwrap();
        assert_eq!(drawer_bytes(&conn).unwrap(), vec![0x1B, 0x70, 0x01, 0x19, 0xFA]);
    }

    #[test]
    fn a_broken_drawer_command_reports_instead_of_sending_garbage() {
        let conn = db();
        conn.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES ('drawer_kick_command', 'no soy hex')",
            [],
        )
        .unwrap();
        assert!(drawer_bytes(&conn).is_err());
    }

    #[test]
    fn money_always_carries_two_decimals() {
        assert_eq!(money("$", 249.0), "$249.00");
        assert_eq!(money("MXN", 1.5), "MXN1.50");
    }

    #[test]
    fn every_payment_method_has_a_label_for_the_ticket() {
        for m in ["cash", "card", "transfer", "mixed"] {
            assert_ne!(method_label(m), "Otro");
        }
    }
}
