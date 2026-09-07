use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::category::{Category, CreateCategoryDto, UpdateCategoryDto};
use crate::session::{require_admin, require_auth, SessionState};

/// Columnas enumeradas a propósito: con `c.*`, agregar una columna a la tabla
/// recorrería los índices y `product_count` pasaría a leer otra cosa.
const COLUMNAS: &str = "c.id, c.name, c.description, c.is_active, c.created_at, c.updated_at";

#[tauri::command]
pub fn get_categories(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<Category>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

    let mut stmt = db.prepare(
        &format!(
            "SELECT {}, (SELECT COUNT(*) FROM products WHERE category_id = c.id) as product_count
             FROM categories c ORDER BY c.name ASC",
            COLUMNAS
        )
    ).map_err(|e| e.to_string())?;

    let categories = stmt
        .query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                is_active: row.get::<_, i32>(3)? == 1,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                product_count: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(categories)
}

#[tauri::command]
pub fn create_category(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreateCategoryDto) -> Result<Category, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    db.execute(
        "INSERT INTO categories (name, description) VALUES (?1, ?2)",
        params![data.name, data.description],
    ).map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "Ya existe una categoría con ese nombre".to_string()
        } else {
            e.to_string()
        }
    })?;

    let id = db.last_insert_rowid();
    db.query_row(
        &format!("SELECT {}, 0 as product_count FROM categories c WHERE c.id = ?1", COLUMNAS),
        params![id],
        |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                is_active: row.get::<_, i32>(3)? == 1,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                product_count: row.get(6)?,
            })
        },
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_category(state: State<DbState>, sessions: State<SessionState>, token: String, data: UpdateCategoryDto) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    db.execute(
        "UPDATE categories SET name=?1, description=?2, is_active=?3, updated_at=datetime('now','localtime') WHERE id=?4",
        params![data.name, data.description, data.is_active as i32, data.id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn delete_category(state: State<DbState>, sessions: State<SessionState>, token: String, id: i64) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    // Cuentan también los descontinuados: la llave foránea no distingue, y
    // filtrar por activos dejaba pasar el borrado para que SQLite lo rechazara
    // después con un error que nadie entiende.
    let product_count: i64 = db.query_row(
        "SELECT COUNT(*) FROM products WHERE category_id = ?1",
        params![id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    if product_count > 0 {
        return Err(format!(
            "Todavía hay {} producto(s) en esta categoría. Cámbialos de categoría antes de quitarla.",
            product_count
        ));
    }

    db.execute("DELETE FROM categories WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    Ok(())
}
