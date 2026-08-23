use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::customer::{Customer, CreateCustomerDto, UpdateCustomerDto};
use crate::session::{require_admin, require_auth, SessionState};

const SEL: &str = "SELECT c.id, c.name, c.phone, c.email, c.notes, c.is_active, c.created_at, c.updated_at,
    COALESCE((SELECT SUM(total) FROM sales WHERE customer_id = c.id AND status = 'completed'), 0) AS total_purchases,
    COALESCE((SELECT COUNT(*) FROM sales WHERE customer_id = c.id AND status = 'completed'), 0) AS purchase_count
    FROM customers c";

fn row_to_customer(row: &rusqlite::Row) -> rusqlite::Result<Customer> {
    Ok(Customer {
        id: row.get(0)?,
        name: row.get(1)?,
        phone: row.get(2)?,
        email: row.get(3)?,
        notes: row.get(4)?,
        is_active: row.get::<_, i32>(5)? == 1,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        total_purchases: row.get(8)?,
        purchase_count: row.get(9)?,
    })
}

#[tauri::command]
pub fn get_customers(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<Customer>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.prepare(&format!("{} ORDER BY c.name ASC", SEL)).map_err(|e| e.to_string())?;
    let out = stmt
        .query_map([], row_to_customer)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[tauri::command]
pub fn create_customer(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreateCustomerDto) -> Result<Customer, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    if data.name.trim().is_empty() {
        return Err("El nombre es requerido".to_string());
    }
    db.execute(
        "INSERT INTO customers (name, phone, email, notes) VALUES (?1, ?2, ?3, ?4)",
        params![data.name, data.phone, data.email, data.notes],
    ).map_err(|e| e.to_string())?;
    let id = db.last_insert_rowid();
    db.query_row(&format!("{} WHERE c.id = ?1", SEL), params![id], row_to_customer)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_customer(state: State<DbState>, sessions: State<SessionState>, token: String, data: UpdateCustomerDto) -> Result<(), String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE customers SET name=?1, phone=?2, email=?3, notes=?4, is_active=?5, updated_at=datetime('now','localtime') WHERE id=?6",
        params![data.name, data.phone, data.email, data.notes, data.is_active as i32, data.id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_customer(state: State<DbState>, sessions: State<SessionState>, token: String, id: i64) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE customers SET is_active = 0, updated_at = datetime('now','localtime') WHERE id = ?1",
        params![id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
