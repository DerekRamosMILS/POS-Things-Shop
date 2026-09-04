use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::supplier::{CreateSupplierDto, Supplier, UpdateSupplierDto};
use crate::session::{require_admin, require_auth, SessionState};

#[tauri::command]
pub fn get_suppliers(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<Supplier>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

    let mut stmt = db.prepare(
        "SELECT * FROM suppliers ORDER BY name ASC"
    ).map_err(|e| e.to_string())?;

    let suppliers = stmt
        .query_map([], |row| {
            Ok(Supplier {
                id: row.get(0)?,
                name: row.get(1)?,
                contact_name: row.get(2)?,
                phone: row.get(3)?,
                email: row.get(4)?,
                address: row.get(5)?,
                notes: row.get(6)?,
                is_active: row.get::<_, i32>(7)? == 1,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(suppliers)
}

#[tauri::command]
pub fn create_supplier(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreateSupplierDto) -> Result<Supplier, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    db.execute(
        "INSERT INTO suppliers (name, contact_name, phone, email, address, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![data.name, data.contact_name, data.phone, data.email, data.address, data.notes],
    ).map_err(|e| e.to_string())?;

    let id = db.last_insert_rowid();
    db.query_row(
        "SELECT * FROM suppliers WHERE id = ?1",
        params![id],
        |row| {
            Ok(Supplier {
                id: row.get(0)?,
                name: row.get(1)?,
                contact_name: row.get(2)?,
                phone: row.get(3)?,
                email: row.get(4)?,
                address: row.get(5)?,
                notes: row.get(6)?,
                is_active: row.get::<_, i32>(7)? == 1,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_supplier(state: State<DbState>, sessions: State<SessionState>, token: String, data: UpdateSupplierDto) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    db.execute(
        "UPDATE suppliers SET name=?1, contact_name=?2, phone=?3, email=?4, address=?5, notes=?6, is_active=?7, updated_at=datetime('now','localtime') WHERE id=?8",
        params![data.name, data.contact_name, data.phone, data.email, data.address, data.notes, data.is_active as i32, data.id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn delete_supplier(state: State<DbState>, sessions: State<SessionState>, token: String, id: i64) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    db.execute(
        "UPDATE suppliers SET is_active = 0, updated_at = datetime('now','localtime') WHERE id = ?1",
        params![id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
