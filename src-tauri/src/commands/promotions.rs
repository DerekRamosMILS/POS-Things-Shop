use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::promotion::{CreatePromotionDto, Promotion};
use crate::session::{require_admin, require_auth, SessionState};

#[tauri::command]
pub fn get_promotions(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<Promotion>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db.prepare(
        "SELECT id, name, description, discount_type, discount_value, start_date, end_date, is_active, applies_to, target_id, created_at
         FROM promotions ORDER BY created_at DESC"
    ).map_err(|e| e.to_string())?;

    let promotions = stmt.query_map([], |row| {
        Ok(Promotion {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            discount_type: row.get(3)?,
            discount_value: row.get(4)?,
            start_date: row.get(5)?,
            end_date: row.get(6)?,
            is_active: row.get(7)?,
            applies_to: row.get(8)?,
            target_id: row.get(9)?,
            created_at: row.get(10)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok(promotions)
}

#[tauri::command]
pub fn create_promotion(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreatePromotionDto) -> Result<Promotion, String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute(
        "INSERT INTO promotions (name, description, discount_type, discount_value, start_date, end_date, is_active, applies_to, target_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)",
        params![data.name, data.description, data.discount_type, data.discount_value, data.start_date, data.end_date, data.applies_to, data.target_id],
    ).map_err(|e| e.to_string())?;

    let id = db.last_insert_rowid();
    db.query_row(
        "SELECT id, name, description, discount_type, discount_value, start_date, end_date, is_active, applies_to, target_id, created_at FROM promotions WHERE id = ?1",
        params![id],
        |row| Ok(Promotion {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            discount_type: row.get(3)?,
            discount_value: row.get(4)?,
            start_date: row.get(5)?,
            end_date: row.get(6)?,
            is_active: row.get(7)?,
            applies_to: row.get(8)?,
            target_id: row.get(9)?,
            created_at: row.get(10)?,
        })
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_promotion(state: State<DbState>, sessions: State<SessionState>, token: String, data: Promotion) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute(
        "UPDATE promotions SET name = ?1, description = ?2, discount_type = ?3, discount_value = ?4,
         start_date = ?5, end_date = ?6, is_active = ?7, applies_to = ?8, target_id = ?9
         WHERE id = ?10",
        params![data.name, data.description, data.discount_type, data.discount_value,
                data.start_date, data.end_date, data.is_active, data.applies_to, data.target_id, data.id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn delete_promotion(state: State<DbState>, sessions: State<SessionState>, token: String, id: i64) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM promotions WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
