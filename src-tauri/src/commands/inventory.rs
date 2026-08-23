use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::inventory::{AdjustStockDto, InventoryMovement, RegisterPurchaseDto};
use crate::session::{require_admin, require_auth, SessionState};

#[tauri::command]
pub fn get_inventory_movements(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    product_id: Option<i64>,
    limit: Option<i32>,
) -> Result<Vec<InventoryMovement>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let limit = limit.unwrap_or(200);

    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
        if let Some(pid) = product_id {
            (
                "SELECT im.*, p.name as product_name, u.full_name as user_name
             FROM inventory_movements im
             LEFT JOIN products p ON im.product_id = p.id
             LEFT JOIN users u ON im.user_id = u.id
             WHERE im.product_id = ?1
             ORDER BY im.created_at DESC LIMIT ?2"
                    .to_string(),
                vec![
                    Box::new(pid) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(limit),
                ],
            )
        } else {
            (
                "SELECT im.*, p.name as product_name, u.full_name as user_name
             FROM inventory_movements im
             LEFT JOIN products p ON im.product_id = p.id
             LEFT JOIN users u ON im.user_id = u.id
             ORDER BY im.created_at DESC LIMIT ?1"
                    .to_string(),
                vec![Box::new(limit) as Box<dyn rusqlite::types::ToSql>],
            )
        };

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let movements = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(InventoryMovement {
                id: row.get(0)?,
                product_id: row.get(1)?,
                movement_type: row.get(2)?,
                quantity: row.get(3)?,
                previous_stock: row.get(4)?,
                new_stock: row.get(5)?,
                reference_id: row.get(6)?,
                reason: row.get(7)?,
                user_id: row.get(8)?,
                created_at: row.get(9)?,
                product_name: row.get(10)?,
                user_name: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(movements)
}

#[tauri::command]
pub fn adjust_stock(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    data: AdjustStockDto,
) -> Result<(), String> {
    let user_id = require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let (current_stock, has_variants): (i32, i32) = db
        .query_row(
            "SELECT stock, has_variants FROM products WHERE id = ?1",
            params![data.product_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if has_variants == 1 {
        return Err("Este producto usa variantes; ajusta el stock por talla/color en Productos".to_string());
    }

    let new_stock = current_stock + data.quantity;
    if new_stock < 0 {
        return Err("El stock no puede ser negativo".to_string());
    }

    db.execute(
        "UPDATE products 
         SET stock = ?1, 
             low_stock_ignored = CASE WHEN ?1 > stock THEN 0 ELSE low_stock_ignored END,
             updated_at = datetime('now','localtime') 
         WHERE id = ?2",
        params![new_stock, data.product_id],
    )
    .map_err(|e| e.to_string())?;

    db.execute(
        "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reason, user_id)
         VALUES (?1, 'adjustment', ?2, ?3, ?4, ?5, ?6)",
        params![data.product_id, data.quantity, current_stock, new_stock, data.reason, user_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn register_purchase(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    data: RegisterPurchaseDto,
) -> Result<(), String> {
    let user_id = require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let (current_stock, has_variants): (i32, i32) = db
        .query_row(
            "SELECT stock, has_variants FROM products WHERE id = ?1",
            params![data.product_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    if has_variants == 1 {
        return Err("Este producto usa variantes; recibe la compra por talla/color en Productos".to_string());
    }

    let new_stock = current_stock + data.quantity;

    db.execute(
        "UPDATE products
         SET stock = ?1,
             low_stock_ignored = 0,
             updated_at = datetime('now','localtime')
         WHERE id = ?2",
        params![new_stock, data.product_id],
    )
    .map_err(|e| e.to_string())?;

    // Update purchase price if provided
    if let Some(price) = data.purchase_price {
        db.execute(
            "UPDATE products SET purchase_price = ?1 WHERE id = ?2",
            params![price, data.product_id],
        )
        .map_err(|e| e.to_string())?;
    }

    db.execute(
        "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reason, user_id)
         VALUES (?1, 'purchase', ?2, ?3, ?4, 'Compra/Reabastecimiento', ?5)",
        params![data.product_id, data.quantity, current_stock, new_stock, user_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_low_stock_products(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
) -> Result<Vec<crate::models::product::Product>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db.prepare(
        "SELECT p.id, p.sku, p.barcode, p.name, p.description, p.category_id, p.supplier_id,
                p.purchase_price, p.sale_price, p.stock, p.min_stock, p.is_active,
                p.low_stock_ignored, p.created_at, p.updated_at, c.name as category_name, s.name as supplier_name,
                (p.image_url IS NOT NULL AND p.image_url != '') as has_image, p.has_variants
         FROM products p
         LEFT JOIN categories c ON p.category_id = c.id
         LEFT JOIN suppliers s ON p.supplier_id = s.id
         WHERE p.stock <= p.min_stock AND p.is_active = 1
         ORDER BY p.stock ASC"
    ).map_err(|e| e.to_string())?;

    let products = stmt
        .query_map([], |row| {
            Ok(crate::models::product::Product {
                id: row.get(0)?,
                sku: row.get(1)?,
                barcode: row.get(2)?,
                name: row.get(3)?,
                description: row.get(4)?,
                category_id: row.get(5)?,
                supplier_id: row.get(6)?,
                purchase_price: row.get(7)?,
                sale_price: row.get(8)?,
                stock: row.get(9)?,
                min_stock: row.get(10)?,
                is_active: row.get::<_, i32>(11)? == 1,
                low_stock_ignored: row.get::<_, i32>(12)? == 1,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
                category_name: row.get(15)?,
                supplier_name: row.get(16)?,
                // El aviso de stock bajo no muestra fotos: no vale la pena cargarlas.
                image_url: None,
                has_image: row.get::<_, i32>(17)? == 1,
                has_variants: row.get::<_, i32>(18)? == 1,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(products)
}
