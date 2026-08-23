use rusqlite::params;
use serde::Deserialize;
use tauri::State;

use crate::db::connection::DbState;
use crate::session::{require_admin, SessionState};

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[derive(Debug, Deserialize)]
pub struct ReturnItemDto {
    pub sale_item_id: i64,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateReturnDto {
    pub sale_id: i64,
    pub reason: Option<String>,
    pub items: Vec<ReturnItemDto>,
}

/// Register a (partial or full) return: restocks inventory, records the movement,
/// tracks returned_quantity per line, and marks the sale 'returned' when fully returned.
/// Note: cash drawer / revenue are intentionally NOT auto-adjusted here.
#[tauri::command]
pub fn create_return(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreateReturnDto) -> Result<f64, String> {
    let user_id = require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    if data.items.is_empty() {
        return Err("Selecciona artículos a devolver".to_string());
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<f64, String> {
        let sale_status: String = db.query_row(
            "SELECT status FROM sales WHERE id = ?1",
            params![data.sale_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        if sale_status == "cancelled" {
            return Err("La venta está cancelada".to_string());
        }

        struct Ri { sale_item_id: i64, product_id: i64, variant_id: Option<i64>, quantity: i32, refund: f64 }
        let mut ris: Vec<Ri> = Vec::new();
        let mut total_refund = 0.0;

        for it in &data.items {
            if it.quantity <= 0 {
                continue;
            }
            let (product_id, sold_qty, returned_qty, unit_price, discount, variant_id): (i64, i32, i32, f64, f64, Option<i64>) =
                db.query_row(
                    "SELECT product_id, quantity, returned_quantity, unit_price, discount, variant_id FROM sale_items WHERE id = ?1 AND sale_id = ?2",
                    params![it.sale_item_id, data.sale_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                ).map_err(|e| e.to_string())?;

            let available = sold_qty - returned_qty;
            if it.quantity > available {
                return Err(format!("Solo puedes devolver hasta {} unidad(es) de esa línea", available));
            }

            let per_unit_net = if sold_qty > 0 {
                (unit_price * sold_qty as f64 - discount) / sold_qty as f64
            } else {
                0.0
            };
            let refund = round2(per_unit_net * it.quantity as f64);
            total_refund += refund;

            ris.push(Ri { sale_item_id: it.sale_item_id, product_id, variant_id, quantity: it.quantity, refund });
        }

        if ris.is_empty() {
            return Err("Nada que devolver".to_string());
        }
        total_refund = round2(total_refund);

        db.execute(
            "INSERT INTO returns (sale_id, user_id, total_refund, reason) VALUES (?1, ?2, ?3, ?4)",
            params![data.sale_id, user_id, total_refund, data.reason],
        ).map_err(|e| e.to_string())?;
        let return_id = db.last_insert_rowid();

        for ri in &ris {
            db.execute(
                "INSERT INTO return_items (return_id, sale_item_id, product_id, quantity, refund_amount) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![return_id, ri.sale_item_id, ri.product_id, ri.quantity, ri.refund],
            ).map_err(|e| e.to_string())?;

            db.execute(
                "UPDATE sale_items SET returned_quantity = returned_quantity + ?1 WHERE id = ?2",
                params![ri.quantity, ri.sale_item_id],
            ).map_err(|e| e.to_string())?;

            let current_stock: i32 = db.query_row(
                "SELECT stock FROM products WHERE id = ?1",
                params![ri.product_id],
                |row| row.get(0),
            ).map_err(|e| e.to_string())?;
            let new_stock = current_stock + ri.quantity;

            db.execute(
                "UPDATE products SET stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![new_stock, ri.product_id],
            ).map_err(|e| e.to_string())?;

            if let Some(vid) = ri.variant_id {
                db.execute(
                    "UPDATE product_variants SET stock = stock + ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                    params![ri.quantity, vid],
                ).map_err(|e| e.to_string())?;
            }

            db.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reference_id, reason, user_id)
                 VALUES (?1, 'return', ?2, ?3, ?4, ?5, 'Devolución de venta', ?6)",
                params![ri.product_id, ri.quantity, current_stock, new_stock, data.sale_id, user_id],
            ).map_err(|e| e.to_string())?;
        }

        // Mark the sale as fully returned when nothing remains.
        let remaining: i32 = db.query_row(
            "SELECT COALESCE(SUM(quantity - returned_quantity), 0) FROM sale_items WHERE sale_id = ?1",
            params![data.sale_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if remaining <= 0 {
            db.execute(
                "UPDATE sales SET status = 'returned' WHERE id = ?1",
                params![data.sale_id],
            ).map_err(|e| e.to_string())?;
        }

        db.execute(
            "INSERT INTO app_logs (level, module, message, user_id) VALUES ('warn', 'returns', ?1, ?2)",
            params![format!("Devolución de venta {} por {}", data.sale_id, total_refund), user_id],
        ).ok();

        Ok(total_refund)
    })();

    match result {
        Ok(total) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            Ok(total)
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}
