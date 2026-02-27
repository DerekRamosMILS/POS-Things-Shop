use rusqlite::params;
use tauri::State;
use chrono::Local;

use crate::db::connection::DbState;
use crate::models::sale::{CreateSaleDto, Sale, SaleFilters, SaleItem};

#[tauri::command]
pub fn create_sale(
    state: State<DbState>,
    user_id: i64,
    cash_register_id: Option<i64>,
    data: CreateSaleDto,
) -> Result<Sale, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Begin atomic transaction
    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<Sale, String> {
        // Generate folio
        let today = Local::now().format("%Y%m%d").to_string();
        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM sales WHERE folio LIKE ?1",
            params![format!("V-{}-%%", today)],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        let folio = format!("V-{}-{:03}", today, count + 1);

        // Calculate totals
        let mut subtotal = 0.0;
        for item in &data.items {
            let item_subtotal = (item.unit_price * item.quantity as f64) - item.discount;
            subtotal += item_subtotal;
        }
        let total = subtotal - data.discount_total;

        let change_amount = if data.payment_method == "cash" {
            data.amount_paid - total
        } else {
            0.0
        };

        // Insert sale
        db.execute(
            "INSERT INTO sales (folio, user_id, cash_register_id, subtotal, discount_total, tax, total, payment_method, amount_paid, change_amount, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8, ?9, ?10)",
            params![
                folio, user_id, cash_register_id, subtotal,
                data.discount_total, total, data.payment_method,
                data.amount_paid, change_amount, data.notes
            ],
        ).map_err(|e| e.to_string())?;

        let sale_id = db.last_insert_rowid();

        // Insert sale items and decrease stock
        for item in &data.items {
            let item_subtotal = (item.unit_price * item.quantity as f64) - item.discount;

            // Get product info for snapshot
            let (product_name, product_sku, current_stock): (String, String, i32) = db.query_row(
                "SELECT name, sku, stock FROM products WHERE id = ?1",
                params![item.product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).map_err(|e| e.to_string())?;

            // Check stock
            if current_stock < item.quantity {
                return Err(format!(
                    "Stock insuficiente para '{}'. Disponible: {}, Solicitado: {}",
                    product_name, current_stock, item.quantity
                ));
            }

            // Insert sale item
            db.execute(
                "INSERT INTO sale_items (sale_id, product_id, product_name, product_sku, quantity, unit_price, discount, subtotal)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    sale_id, item.product_id, product_name, product_sku,
                    item.quantity, item.unit_price, item.discount, item_subtotal
                ],
            ).map_err(|e| e.to_string())?;

            let new_stock = current_stock - item.quantity;

            // Decrease stock
            db.execute(
                "UPDATE products SET stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![new_stock, item.product_id],
            ).map_err(|e| e.to_string())?;

            // Record inventory movement
            db.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reference_id, user_id)
                 VALUES (?1, 'sale', ?2, ?3, ?4, ?5, ?6)",
                params![
                    item.product_id, -item.quantity, current_stock,
                    new_stock, sale_id, user_id
                ],
            ).map_err(|e| e.to_string())?;
        }

        // Update cash register totals
        if let Some(cr_id) = cash_register_id {
            let payment_field = match data.payment_method.as_str() {
                "cash" => "total_cash_sales",
                "card" => "total_card_sales",
                "transfer" => "total_transfer_sales",
                _ => "total_cash_sales",
            };

            db.execute(
                &format!(
                    "UPDATE cash_registers SET total_sales = total_sales + ?1, {} = {} + ?1, sale_count = sale_count + 1 WHERE id = ?2",
                    payment_field, payment_field
                ),
                params![total, cr_id],
            ).map_err(|e| e.to_string())?;
        }

        // Return the created sale
        get_sale_by_id(&db, sale_id)
    })();

    match result {
        Ok(sale) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            Ok(sale)
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

#[tauri::command]
pub fn cancel_sale(state: State<DbState>, sale_id: i64, user_id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<(), String> {
        // Get sale status
        let status: String = db.query_row(
            "SELECT status FROM sales WHERE id = ?1",
            params![sale_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        if status != "completed" {
            return Err("Solo se pueden cancelar ventas completadas".to_string());
        }

        // Get sale items to restore stock
        let mut stmt = db.prepare(
            "SELECT product_id, quantity FROM sale_items WHERE sale_id = ?1"
        ).map_err(|e| e.to_string())?;

        let items: Vec<(i64, i32)> = stmt.query_map(params![sale_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

        // Restore stock for each item
        for (product_id, quantity) in items {
            let current_stock: i32 = db.query_row(
                "SELECT stock FROM products WHERE id = ?1",
                params![product_id],
                |row| row.get(0),
            ).map_err(|e| e.to_string())?;

            let new_stock = current_stock + quantity;

            db.execute(
                "UPDATE products SET stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![new_stock, product_id],
            ).map_err(|e| e.to_string())?;

            db.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reference_id, reason, user_id)
                 VALUES (?1, 'cancellation', ?2, ?3, ?4, ?5, 'Cancelación de venta', ?6)",
                params![product_id, quantity, current_stock, new_stock, sale_id, user_id],
            ).map_err(|e| e.to_string())?;
        }

        // Update sale status
        db.execute(
            "UPDATE sales SET status = 'cancelled' WHERE id = ?1",
            params![sale_id],
        ).map_err(|e| e.to_string())?;

        // Update cash register
        let (total, payment_method, cr_id): (f64, String, Option<i64>) = db.query_row(
            "SELECT total, payment_method, cash_register_id FROM sales WHERE id = ?1",
            params![sale_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).map_err(|e| e.to_string())?;

        if let Some(cr_id) = cr_id {
            let payment_field = match payment_method.as_str() {
                "cash" => "total_cash_sales",
                "card" => "total_card_sales",
                "transfer" => "total_transfer_sales",
                _ => "total_cash_sales",
            };

            db.execute(
                &format!(
                    "UPDATE cash_registers SET total_sales = total_sales - ?1, {} = {} - ?1, sale_count = sale_count - 1 WHERE id = ?2",
                    payment_field, payment_field
                ),
                params![total, cr_id],
            ).map_err(|e| e.to_string())?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

#[tauri::command]
pub fn get_sales(state: State<DbState>, filters: Option<SaleFilters>) -> Result<Vec<Sale>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let filters = filters.unwrap_or_default();

    let mut sql = String::from(
        "SELECT s.*, u.full_name as user_name FROM sales s
         LEFT JOIN users u ON s.user_id = u.id WHERE 1=1"
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref from) = filters.date_from {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND s.created_at >= ?{}", idx));
        param_values.push(Box::new(from.clone()));
    }
    if let Some(ref to) = filters.date_to {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND s.created_at <= ?{}", idx));
        param_values.push(Box::new(to.clone()));
    }
    if let Some(ref status) = filters.status {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND s.status = ?{}", idx));
        param_values.push(Box::new(status.clone()));
    }
    if let Some(ref pm) = filters.payment_method {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND s.payment_method = ?{}", idx));
        param_values.push(Box::new(pm.clone()));
    }

    sql.push_str(" ORDER BY s.created_at DESC LIMIT 500");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let sales = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(Sale {
                id: row.get(0)?,
                folio: row.get(1)?,
                user_id: row.get(2)?,
                cash_register_id: row.get(3)?,
                subtotal: row.get(4)?,
                discount_total: row.get(5)?,
                tax: row.get(6)?,
                total: row.get(7)?,
                payment_method: row.get(8)?,
                amount_paid: row.get(9)?,
                change_amount: row.get(10)?,
                status: row.get(11)?,
                notes: row.get(12)?,
                created_at: row.get(13)?,
                user_name: row.get(14)?,
                items: None,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(sales)
}

#[tauri::command]
pub fn get_sale_detail(state: State<DbState>, sale_id: i64) -> Result<Sale, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    get_sale_by_id(&db, sale_id)
}

fn get_sale_by_id(db: &rusqlite::Connection, sale_id: i64) -> Result<Sale, String> {
    let mut sale = db.query_row(
        "SELECT s.*, u.full_name as user_name FROM sales s
         LEFT JOIN users u ON s.user_id = u.id
         WHERE s.id = ?1",
        params![sale_id],
        |row| {
            Ok(Sale {
                id: row.get(0)?,
                folio: row.get(1)?,
                user_id: row.get(2)?,
                cash_register_id: row.get(3)?,
                subtotal: row.get(4)?,
                discount_total: row.get(5)?,
                tax: row.get(6)?,
                total: row.get(7)?,
                payment_method: row.get(8)?,
                amount_paid: row.get(9)?,
                change_amount: row.get(10)?,
                status: row.get(11)?,
                notes: row.get(12)?,
                created_at: row.get(13)?,
                user_name: row.get(14)?,
                items: None,
            })
        },
    ).map_err(|e| e.to_string())?;

    // Get items
    let mut stmt = db.prepare(
        "SELECT * FROM sale_items WHERE sale_id = ?1"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(params![sale_id], |row| {
        Ok(SaleItem {
            id: row.get(0)?,
            sale_id: row.get(1)?,
            product_id: row.get(2)?,
            product_name: row.get(3)?,
            product_sku: row.get(4)?,
            quantity: row.get(5)?,
            unit_price: row.get(6)?,
            discount: row.get(7)?,
            subtotal: row.get(8)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    sale.items = Some(items);
    Ok(sale)
}
