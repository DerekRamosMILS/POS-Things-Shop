use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::cash_register::{CashRegister, CloseRegisterDto, OpenRegisterDto};
use crate::money::Cents;
use crate::session::{require_auth, SessionState};

/// Explicit column list: `cr.*` would silently shift every index the moment a
/// migration adds a column to the table.
const REGISTER_COLUMNS: &str = "cr.id, cr.user_id, u.full_name, cr.opening_amount, cr.closing_amount,
     cr.expected_amount, cr.difference, cr.total_sales, cr.total_cash_sales, cr.total_card_sales,
     cr.total_transfer_sales, cr.total_layaway_cash, cr.total_layaway_card, cr.total_layaway_transfer,
     cr.total_refunds_cash, cr.total_expenses, cr.sale_count, cr.status, cr.opened_at, cr.closed_at";

fn map_register(row: &rusqlite::Row) -> rusqlite::Result<CashRegister> {
    Ok(CashRegister {
        id: row.get(0)?,
        user_id: row.get(1)?,
        user_name: row.get(2)?,
        opening_amount: row.get(3)?,
        closing_amount: row.get(4)?,
        expected_amount: row.get(5)?,
        difference: row.get(6)?,
        total_sales: row.get(7)?,
        total_cash_sales: row.get(8)?,
        total_card_sales: row.get(9)?,
        total_transfer_sales: row.get(10)?,
        total_layaway_cash: row.get(11)?,
        total_layaway_card: row.get(12)?,
        total_layaway_transfer: row.get(13)?,
        total_refunds_cash: row.get(14)?,
        total_expenses: row.get(15)?,
        sale_count: row.get(16)?,
        status: row.get(17)?,
        opened_at: row.get(18)?,
        closed_at: row.get(19)?,
    })
}

/// Cash that should physically be in the drawer right now.
///
/// Every movement that adds or removes bills has to appear here, or the close-out
/// reports a phantom surplus/shortfall: sales in cash come in, layaway deposits in
/// cash come in, cash refunds go out, and petty-cash expenses go out.
pub fn expected_cash(r: &CashRegister) -> f64 {
    let p = Cents::from_pesos;
    // En centavos enteros: un turno con cientos de movimientos no acumula el
    // error de redondeo que arrastraría sumar f64 uno tras otro.
    let expected = p(r.opening_amount) + p(r.total_cash_sales) + p(r.total_layaway_cash)
        - p(r.total_refunds_cash)
        - p(r.total_expenses);
    expected.to_pesos()
}

/// Id of the shift currently open, if any. Shared by sales, layaways and returns
/// so every money movement lands on the right cut.
pub fn open_register_id(db: &rusqlite::Connection) -> Option<i64> {
    db.query_row(
        "SELECT id FROM cash_registers WHERE status = 'open' ORDER BY opened_at DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .ok()
}

#[tauri::command]
pub fn open_register(state: State<DbState>, sessions: State<SessionState>, token: String, data: OpenRegisterDto) -> Result<CashRegister, String> {
    // The cashier on record is the authenticated user, never a client-sent id.
    let user_id = require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    if data.opening_amount < 0.0 {
        return Err("El fondo de apertura no puede ser negativo".to_string());
    }

    // Check if there's already an open register
    let open_count: i64 = db.query_row(
        "SELECT COUNT(*) FROM cash_registers WHERE status = 'open'",
        [],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    if open_count > 0 {
        return Err("Ya hay una caja abierta. Ciérrela primero.".to_string());
    }

    db.execute(
        "INSERT INTO cash_registers (user_id, opening_amount) VALUES (?1, ?2)",
        params![user_id, data.opening_amount],
    ).map_err(|e| e.to_string())?;

    let id = db.last_insert_rowid();
    db.execute(
        "INSERT INTO app_logs (level, module, message, user_id) VALUES ('info', 'caja', ?1, ?2)",
        params![format!("Caja abierta con fondo de {}", data.opening_amount), user_id],
    ).ok();

    get_register_by_id(&db, id)
}

#[tauri::command]
pub fn close_register(state: State<DbState>, sessions: State<SessionState>, token: String, data: CloseRegisterDto) -> Result<CashRegister, String> {
    let user_id = require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let register = get_open_register_internal(&db)?;

    let expected = expected_cash(&register);
    let difference =
        (Cents::from_pesos(data.closing_amount) - Cents::from_pesos(expected)).to_pesos();

    db.execute(
        "UPDATE cash_registers SET closing_amount=?1, expected_amount=?2, difference=?3, status='closed', closed_at=datetime('now','localtime') WHERE id=?4",
        params![data.closing_amount, expected, difference, register.id],
    ).map_err(|e| e.to_string())?;

    db.execute(
        "INSERT INTO app_logs (level, module, message, user_id) VALUES (?1, 'caja', ?2, ?3)",
        params![
            if Cents::from_pesos(difference).abs().is_positive() { "warn" } else { "info" },
            format!(
                "Caja cerrada: esperado {}, contado {}, diferencia {}",
                expected, data.closing_amount, difference
            ),
            user_id
        ],
    ).ok();

    get_register_by_id(&db, register.id)
}

#[tauri::command]
pub fn get_open_register(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Option<CashRegister>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    match get_open_register_internal(&db) {
        Ok(reg) => Ok(Some(reg)),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub fn get_register_history(state: State<DbState>, sessions: State<SessionState>, token: String, limit: Option<i32>) -> Result<Vec<CashRegister>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(30);

    let mut stmt = db.prepare(&format!(
        "SELECT {} FROM cash_registers cr
         LEFT JOIN users u ON cr.user_id = u.id
         ORDER BY cr.opened_at DESC LIMIT ?1",
        REGISTER_COLUMNS
    )).map_err(|e| e.to_string())?;

    let registers = stmt
        .query_map(params![limit], map_register)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(registers)
}

fn get_open_register_internal(db: &rusqlite::Connection) -> Result<CashRegister, String> {
    db.query_row(
        &format!(
            "SELECT {} FROM cash_registers cr
             LEFT JOIN users u ON cr.user_id = u.id
             WHERE cr.status = 'open' LIMIT 1",
            REGISTER_COLUMNS
        ),
        [],
        map_register,
    ).map_err(|_| "No hay caja abierta".to_string())
}

fn get_register_by_id(db: &rusqlite::Connection, id: i64) -> Result<CashRegister, String> {
    db.query_row(
        &format!(
            "SELECT {} FROM cash_registers cr
             LEFT JOIN users u ON cr.user_id = u.id
             WHERE cr.id = ?1",
            REGISTER_COLUMNS
        ),
        params![id],
        map_register,
    ).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register() -> CashRegister {
        CashRegister {
            id: 1, user_id: 1, user_name: None,
            opening_amount: 1000.0, closing_amount: None, expected_amount: None, difference: None,
            total_sales: 0.0, total_cash_sales: 0.0, total_card_sales: 0.0, total_transfer_sales: 0.0,
            total_layaway_cash: 0.0, total_layaway_card: 0.0, total_layaway_transfer: 0.0,
            total_refunds_cash: 0.0, total_expenses: 0.0, sale_count: 0,
            status: "open".to_string(), opened_at: String::new(), closed_at: None,
        }
    }

    #[test]
    fn an_untouched_shift_expects_its_opening_float() {
        assert_eq!(expected_cash(&register()), 1000.0);
    }

    #[test]
    fn card_and_transfer_sales_never_reach_the_drawer() {
        let r = CashRegister { total_card_sales: 500.0, total_transfer_sales: 300.0, ..register() };
        assert_eq!(expected_cash(&r), 1000.0);
    }

    #[test]
    fn layaway_deposits_in_cash_are_counted() {
        // El caso que antes aparecía como sobrante en el corte.
        let r = CashRegister { total_layaway_cash: 500.0, ..register() };
        assert_eq!(expected_cash(&r), 1500.0);
    }

    #[test]
    fn layaway_deposits_by_card_are_not() {
        let r = CashRegister { total_layaway_card: 500.0, total_layaway_transfer: 200.0, ..register() };
        assert_eq!(expected_cash(&r), 1000.0);
    }

    #[test]
    fn cash_refunds_leave_the_drawer() {
        // El caso que antes aparecía como faltante en el corte.
        let r = CashRegister { total_cash_sales: 800.0, total_refunds_cash: 300.0, ..register() };
        assert_eq!(expected_cash(&r), 1500.0);
    }

    #[test]
    fn every_movement_adds_up_together() {
        let r = CashRegister {
            total_cash_sales: 2500.0,
            total_card_sales: 900.0,
            total_layaway_cash: 400.0,
            total_layaway_card: 150.0,
            total_refunds_cash: 250.0,
            total_expenses: 180.0,
            ..register()
        };
        // 1000 + 2500 + 400 - 250 - 180
        assert_eq!(expected_cash(&r), 3470.0);
    }

    #[test]
    fn the_result_is_rounded_to_cents() {
        let r = CashRegister { total_cash_sales: 0.1, total_layaway_cash: 0.2, ..register() };
        assert_eq!(expected_cash(&r), 1000.30);
    }

    #[test]
    fn many_small_movements_do_not_drift_a_single_cent() {
        // Sumar 0.1 + 0.2 en f64 da 0.30000000000000004; en centavos, 0.30.
        let r = CashRegister {
            opening_amount: 0.0,
            total_cash_sales: 0.1,
            total_layaway_cash: 0.2,
            total_expenses: 0.3,
            ..register()
        };
        assert_eq!(expected_cash(&r), 0.0);
    }
}
