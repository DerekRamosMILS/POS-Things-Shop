use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::cash_register::{CashRegister, CloseRegisterDto, OpenRegisterDto};
use crate::session::{require_auth, SessionState};

#[tauri::command]
pub fn open_register(state: State<DbState>, sessions: State<SessionState>, token: String, data: OpenRegisterDto) -> Result<CashRegister, String> {
    // The cashier on record is the authenticated user, never a client-sent id.
    let user_id = require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

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
    get_register_by_id(&db, id)
}

#[tauri::command]
pub fn close_register(state: State<DbState>, sessions: State<SessionState>, token: String, data: CloseRegisterDto) -> Result<CashRegister, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let register = get_open_register_internal(&db)?;

    let expected = register.opening_amount + register.total_cash_sales - register.total_expenses;
    let difference = data.closing_amount - expected;

    db.execute(
        "UPDATE cash_registers SET closing_amount=?1, expected_amount=?2, difference=?3, status='closed', closed_at=datetime('now','localtime') WHERE id=?4",
        params![data.closing_amount, expected, difference, register.id],
    ).map_err(|e| e.to_string())?;

    // Automatic backup on close, when enabled in config.
    let auto_backup: bool = db.query_row(
        "SELECT value FROM system_config WHERE key = 'auto_backup'",
        [],
        |row| row.get::<_, String>(0),
    ).map(|v| v.trim() == "1").unwrap_or(false);
    if auto_backup {
        if let Err(e) = crate::commands::backup::perform_backup(&db) {
            log::warn!("Auto-backup on close failed: {}", e);
        }
    }

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

    let mut stmt = db.prepare(
        "SELECT cr.*, u.full_name as user_name FROM cash_registers cr
         LEFT JOIN users u ON cr.user_id = u.id
         ORDER BY cr.opened_at DESC LIMIT ?1"
    ).map_err(|e| e.to_string())?;

    let registers = stmt
        .query_map(params![limit], |row| {
            Ok(CashRegister {
                id: row.get(0)?,
                user_id: row.get(1)?,
                opening_amount: row.get(2)?,
                closing_amount: row.get(3)?,
                expected_amount: row.get(4)?,
                difference: row.get(5)?,
                total_sales: row.get(6)?,
                total_cash_sales: row.get(7)?,
                total_card_sales: row.get(8)?,
                total_transfer_sales: row.get(9)?,
                total_expenses: row.get(10)?,
                sale_count: row.get(11)?,
                status: row.get(12)?,
                opened_at: row.get(13)?,
                closed_at: row.get(14)?,
                user_name: row.get(15)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(registers)
}

fn get_open_register_internal(db: &rusqlite::Connection) -> Result<CashRegister, String> {
    db.query_row(
        "SELECT cr.*, u.full_name as user_name FROM cash_registers cr
         LEFT JOIN users u ON cr.user_id = u.id
         WHERE cr.status = 'open' LIMIT 1",
        [],
        |row| {
            Ok(CashRegister {
                id: row.get(0)?,
                user_id: row.get(1)?,
                opening_amount: row.get(2)?,
                closing_amount: row.get(3)?,
                expected_amount: row.get(4)?,
                difference: row.get(5)?,
                total_sales: row.get(6)?,
                total_cash_sales: row.get(7)?,
                total_card_sales: row.get(8)?,
                total_transfer_sales: row.get(9)?,
                total_expenses: row.get(10)?,
                sale_count: row.get(11)?,
                status: row.get(12)?,
                opened_at: row.get(13)?,
                closed_at: row.get(14)?,
                user_name: row.get(15)?,
            })
        },
    ).map_err(|_| "No hay caja abierta".to_string())
}

fn get_register_by_id(db: &rusqlite::Connection, id: i64) -> Result<CashRegister, String> {
    db.query_row(
        "SELECT cr.*, u.full_name as user_name FROM cash_registers cr
         LEFT JOIN users u ON cr.user_id = u.id
         WHERE cr.id = ?1",
        params![id],
        |row| {
            Ok(CashRegister {
                id: row.get(0)?,
                user_id: row.get(1)?,
                opening_amount: row.get(2)?,
                closing_amount: row.get(3)?,
                expected_amount: row.get(4)?,
                difference: row.get(5)?,
                total_sales: row.get(6)?,
                total_cash_sales: row.get(7)?,
                total_card_sales: row.get(8)?,
                total_transfer_sales: row.get(9)?,
                total_expenses: row.get(10)?,
                sale_count: row.get(11)?,
                status: row.get(12)?,
                opened_at: row.get(13)?,
                closed_at: row.get(14)?,
                user_name: row.get(15)?,
            })
        },
    ).map_err(|e| e.to_string())
}
