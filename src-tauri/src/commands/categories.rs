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
    let nombre = crate::commands::nombre_requerido(&data.name, "la categoría")?;
    let db = state.conn();

    db.execute(
        "INSERT INTO categories (name, description) VALUES (?1, ?2)",
        params![nombre, data.description],
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
    let nombre = crate::commands::nombre_requerido(&data.name, "la categoría")?;
    let db = state.conn();

    db.execute(
        "UPDATE categories SET name=?1, description=?2, is_active=?3, updated_at=datetime('now','localtime') WHERE id=?4",
        params![nombre, data.description, data.is_active as i32, data.id],
    ).map_err(|e| {
        // El nombre es único. Al crear ya se explicaba; al renombrar salía el
        // error crudo de SQLite, que en el mostrador no dice nada.
        if e.to_string().contains("UNIQUE") {
            "Ya existe una categoría con ese nombre".to_string()
        } else {
            e.to_string()
        }
    })?;

    Ok(())
}

#[tauri::command]
pub fn delete_category(state: State<DbState>, sessions: State<SessionState>, token: String, id: i64) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    quitar_categoria(&db, id)
}

pub(crate) fn quitar_categoria(db: &rusqlite::Connection, id: i64) -> Result<(), String> {
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

    // `promotions.target_id` no tiene llave foránea: sin esto, la promoción
    // quedaba activa apuntando a nada y al cobrar no descontaba. Las vencidas
    // o apagadas no estorban; ya no se aplican.
    let vigente: Option<String> = db
        .query_row(
            "SELECT name FROM promotions
             WHERE applies_to = 'category' AND target_id = ?1 AND is_active = 1
               AND date(end_date) >= date('now','localtime')
             ORDER BY id LIMIT 1",
            params![id],
            |r| r.get(0),
        )
        .ok();
    if let Some(nombre) = vigente {
        return Err(format!(
            "La promoción «{}» aplica a esta categoría. Cámbiala o desactívala antes de quitarla.",
            nombre
        ));
    }

    db.execute("DELETE FROM categories WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute("INSERT INTO categories (id, name) VALUES (50, 'Temporada')", []).unwrap();
        db
    }

    fn promo(db: &rusqlite::Connection, activa: bool, fin: &str) {
        db.execute(
            "INSERT INTO promotions (name, discount_type, discount_value, start_date, end_date, is_active, applies_to, target_id)
             VALUES ('Verano', 'percentage', 20.0, '2020-01-01', ?1, ?2, 'category', 50)",
            params![fin, activa as i32],
        ).unwrap();
    }

    fn existe(db: &rusqlite::Connection) -> bool {
        db.query_row("SELECT COUNT(*) FROM categories WHERE id = 50", [], |r| r.get::<_, i64>(0)).unwrap() == 1
    }

    #[test]
    fn una_categoria_vacia_se_quita() {
        let db = tienda();
        quitar_categoria(&db, 50).unwrap();
        assert!(!existe(&db));
    }

    #[test]
    fn no_se_quita_una_categoria_con_una_promocion_vigente() {
        // `target_id` no tiene llave foránea. Quitar la categoría dejaba la
        // promoción marcada como activa, la pantalla decía sólo "Categoría" y
        // al cobrar no descontaba nada: el cliente pagaba completo creyendo
        // que había promoción.
        let db = tienda();
        promo(&db, true, "2999-12-31");
        let e = quitar_categoria(&db, 50).unwrap_err();
        assert!(e.contains("Verano"), "el mensaje debe decir cuál promoción: {}", e);
        assert!(existe(&db));
    }

    #[test]
    fn una_promocion_vencida_o_apagada_no_estorba() {
        let db = tienda();
        promo(&db, true, "2000-01-01");
        promo(&db, false, "2999-12-31");
        quitar_categoria(&db, 50).unwrap();
        assert!(!existe(&db));
    }
}
