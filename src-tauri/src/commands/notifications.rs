use crate::db::connection::DbState;
use crate::session::{require_auth, SessionState};
use crate::models::notification::{CreateReminderDto, Notification};
use rusqlite::params;
use tauri::State;

/// Vuelve a encender la alerta de lo que ya se resurtió.
///
/// Descartar la alerta la apaga hasta que el producto se resurta. Solo el ajuste
/// manual y la compra la volvían a encender, y ninguno de los dos existe para
/// productos con tallas —casi toda la ropa—: una vez descartada, su alerta no
/// volvía nunca. Aquí se enciende en cuanto la existencia vuelve a estar por
/// encima del mínimo, venga de donde venga.
pub(crate) fn rearmar_alertas(db: &rusqlite::Connection) {
    db.execute(
        "UPDATE products SET low_stock_ignored = 0 WHERE low_stock_ignored = 1 AND stock > min_stock",
        [],
    )
    .ok();
}

#[tauri::command]
pub fn get_notifications(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<Notification>, String> {
    require_auth(&sessions, &token)?;
    let conn = state.conn();
    rearmar_alertas(&conn);

    // Low stock notifications
    // We dynamically insert/report low stock products that aren't ignored
    let mut stmt = conn
        .prepare(
            "SELECT id, sku, name, stock, min_stock 
             FROM products 
             WHERE stock <= min_stock AND is_active = 1 AND low_stock_ignored = 0",
        )
        .map_err(|e: rusqlite::Error| e.to_string())?;

    let low_stock_iter = stmt
        .query_map([], |row: &rusqlite::Row| {
            let id: i64 = row.get(0)?;
            let name: String = row.get(2)?;
            let stock: i32 = row.get(3)?;
            let min_stock: i32 = row.get(4)?;

            Ok(Notification {
                id: -id, // Virtual ID for low stock (negative product ID)
                product_id: Some(id),
                message: format!(
                    "{} está bajo en stock (Actual: {}, Mínimo: {})",
                    name, stock, min_stock
                ),
                target_date: None,
                is_read: false,
                notification_type: "low_stock".to_string(),
                created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            })
        })
        .map_err(|e: rusqlite::Error| e.to_string())?;

    let mut notifications = Vec::new();
    for alert in low_stock_iter {
        notifications.push(alert.map_err(|e: rusqlite::Error| e.to_string())?);
    }

    // Explicit reminders
    let mut stmt2 = conn
        .prepare(
            "SELECT id, product_id, message, target_date, is_read, notification_type, created_at 
             FROM notifications 
             WHERE is_read = 0 
             ORDER BY target_date ASC, created_at DESC",
        )
        .map_err(|e: rusqlite::Error| e.to_string())?;

    let reminder_iter = stmt2
        .query_map([], |row: &rusqlite::Row| {
            Ok(Notification {
                id: row.get(0)?,
                product_id: row.get(1)?,
                message: row.get(2)?,
                target_date: row.get(3)?,
                is_read: row.get(4).unwrap_or(0) > 0,
                notification_type: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e: rusqlite::Error| e.to_string())?;

    for reminder in reminder_iter {
        notifications.push(reminder.map_err(|e: rusqlite::Error| e.to_string())?);
    }

    Ok(notifications)
}

#[tauri::command]
pub fn mark_notification_read(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    id: i64,
    notification_type: String,
) -> Result<(), String> {
    require_auth(&sessions, &token)?;
    let conn = state.conn();

    if notification_type == "low_stock" {
        // ID is passed as -product_id
        let product_id = -id;
        conn.execute(
            "UPDATE products SET low_stock_ignored = 1 WHERE id = ?1",
            params![product_id],
        )
        .map_err(|e: rusqlite::Error| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE notifications SET is_read = 1 WHERE id = ?1",
            params![id],
        )
        .map_err(|e: rusqlite::Error| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn create_reminder(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    data: CreateReminderDto,
) -> Result<Notification, String> {
    require_auth(&sessions, &token)?;
    let conn = state.conn();

    // Get product name
    let product_name: String = conn
        .query_row(
            "SELECT name FROM products WHERE id = ?1",
            params![data.product_id],
            |row: &rusqlite::Row| row.get(0),
        )
        .map_err(|e: rusqlite::Error| format!("Product not found: {}", e))?;

    let msg = format!("Recordatorio para restock: {}", product_name);

    conn.execute(
        "INSERT INTO notifications (product_id, message, target_date, notification_type) 
         VALUES (?1, ?2, ?3, 'restock_reminder')",
        params![data.product_id, msg, data.target_date],
    )
    .map_err(|e: rusqlite::Error| e.to_string())?;

    let id = conn.last_insert_rowid();

    let notification = conn
        .query_row(
            "SELECT id, product_id, message, target_date, is_read, notification_type, created_at 
         FROM notifications WHERE id = ?1",
            params![id],
            |row: &rusqlite::Row| {
                Ok(Notification {
                    id: row.get(0)?,
                    product_id: row.get(1)?,
                    message: row.get(2)?,
                    target_date: row.get(3)?,
                    is_read: row.get(4).unwrap_or(0) > 0,
                    notification_type: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )
        .map_err(|e: rusqlite::Error| e.to_string())?;

    Ok(notification)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::variants::guardar_variantes;
    use crate::models::variant::SaveVariantDto;

    fn ignorada(db: &rusqlite::Connection) -> bool {
        db.query_row("SELECT low_stock_ignored = 1 FROM products WHERE id = 1", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn la_alerta_de_una_prenda_con_tallas_vuelve_tras_resurtirla() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute_batch(
            "INSERT INTO users (id, username, password_hash, full_name, role) VALUES (1, 'u', 'x', 'U', 'admin');
             INSERT INTO products (id, sku, name, purchase_price, sale_price, stock, min_stock, has_variants)
                 VALUES (1, 'V', 'Vestido', 1, 2, 1, 3, 1);
             INSERT INTO product_variants (id, product_id, size, stock) VALUES (1, 1, 'M', 1);
             UPDATE products SET low_stock_ignored = 1 WHERE id = 1;",
        ).unwrap();

        // Se resurte por el único camino que tienen las tallas.
        guardar_variantes(&db, 1, 1, vec![SaveVariantDto {
            id: Some(1), size: Some("M".into()), color: None, sku: None, barcode: None,
            stock: 10, stock_original: Some(1),
        }]).unwrap();
        assert!(ignorada(&db), "sin el rearme se quedaba apagada");

        rearmar_alertas(&db);
        assert!(!ignorada(&db));
    }

    #[test]
    fn lo_que_sigue_bajo_mantiene_su_alerta_descartada() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute_batch(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock, min_stock, low_stock_ignored)
                 VALUES (1, 'V', 'Vestido', 1, 2, 2, 3, 1);",
        ).unwrap();

        rearmar_alertas(&db);

        assert!(ignorada(&db), "quien la descartó no quiere verla otra vez mientras siga igual");
    }
}
