use crate::db::connection::DbState;
use crate::models::notification::{CreateReminderDto, Notification};
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub fn get_notifications(state: State<DbState>) -> Result<Vec<Notification>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

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
    id: i64,
    notification_type: String,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

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
    data: CreateReminderDto,
) -> Result<Notification, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;

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
