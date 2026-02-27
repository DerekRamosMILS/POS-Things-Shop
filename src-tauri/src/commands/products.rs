use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::product::{CreateProductDto, Product, ProductFilters, UpdateProductDto};

#[tauri::command]
pub fn get_products(
    state: State<DbState>,
    filters: Option<ProductFilters>,
) -> Result<Vec<Product>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let filters = filters.unwrap_or_default();

    let mut sql = String::from(
        "SELECT p.id, p.sku, p.barcode, p.name, p.description, p.category_id, p.supplier_id, 
                p.purchase_price, p.sale_price, p.stock, p.min_stock, p.is_active, 
                p.low_stock_ignored, p.created_at, p.updated_at, c.name as category_name, s.name as supplier_name
         FROM products p
         LEFT JOIN categories c ON p.category_id = c.id
         LEFT JOIN suppliers s ON p.supplier_id = s.id
         WHERE 1=1"
    );
    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(ref search) = filters.search {
        sql.push_str(" AND (p.name LIKE ?1 OR p.sku LIKE ?1 OR p.barcode LIKE ?1)");
        param_values.push(Box::new(format!("%{}%", search)));
    }
    if let Some(cat_id) = filters.category_id {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND p.category_id = ?{}", idx));
        param_values.push(Box::new(cat_id));
    }
    if let Some(active) = filters.is_active {
        let idx = param_values.len() + 1;
        sql.push_str(&format!(" AND p.is_active = ?{}", idx));
        param_values.push(Box::new(active as i32));
    }
    if filters.low_stock.unwrap_or(false) {
        sql.push_str(" AND p.stock <= p.min_stock");
    }

    sql.push_str(" ORDER BY p.name ASC");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let products = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(Product {
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
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(products)
}

#[tauri::command]
pub fn get_product_by_barcode(
    state: State<DbState>,
    barcode: String,
) -> Result<Option<Product>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let result = db.query_row(
        "SELECT p.id, p.sku, p.barcode, p.name, p.description, p.category_id, p.supplier_id, 
                p.purchase_price, p.sale_price, p.stock, p.min_stock, p.is_active, 
                p.low_stock_ignored, p.created_at, p.updated_at, c.name as category_name, s.name as supplier_name
         FROM products p
         LEFT JOIN categories c ON p.category_id = c.id
         LEFT JOIN suppliers s ON p.supplier_id = s.id
         WHERE p.barcode = ?1 AND p.is_active = 1",
        params![barcode],
        |row| {
            Ok(Product {
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
            })
        },
    );

    match result {
        Ok(product) => Ok(Some(product)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn create_product(state: State<DbState>, data: CreateProductDto) -> Result<Product, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute(
        "INSERT INTO products (sku, barcode, name, description, category_id, supplier_id, purchase_price, sale_price, stock, min_stock)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            data.sku, data.barcode, data.name, data.description,
            data.category_id, data.supplier_id, data.purchase_price,
            data.sale_price, data.stock, data.min_stock
        ],
    ).map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            if e.to_string().contains("sku") {
                "El SKU ya existe".to_string()
            } else {
                "El código de barras ya existe".to_string()
            }
        } else {
            e.to_string()
        }
    })?;

    let id = db.last_insert_rowid();

    // Record initial stock movement if stock > 0
    if data.stock > 0 {
        db.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reason)
             VALUES (?1, 'adjustment', ?2, 0, ?2, 'Stock inicial')",
            params![id, data.stock],
        ).map_err(|e| e.to_string())?;
    }

    get_product_by_id(&db, id)
}

#[tauri::command]
pub fn update_product(state: State<DbState>, data: UpdateProductDto) -> Result<Product, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Check for price change and record history
    let old_price: f64 = db
        .query_row(
            "SELECT sale_price FROM products WHERE id = ?1",
            params![data.id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if (old_price - data.sale_price).abs() > 0.001 {
        db.execute(
            "INSERT INTO price_history (product_id, old_price, new_price)
             VALUES (?1, ?2, ?3)",
            params![data.id, old_price, data.sale_price],
        )
        .map_err(|e| e.to_string())?;
    }

    db.execute(
        "UPDATE products SET sku=?1, barcode=?2, name=?3, description=?4,
         category_id=?5, supplier_id=?6, purchase_price=?7, sale_price=?8,
         min_stock=?9, is_active=?10, updated_at=datetime('now','localtime')
         WHERE id=?11",
        params![
            data.sku,
            data.barcode,
            data.name,
            data.description,
            data.category_id,
            data.supplier_id,
            data.purchase_price,
            data.sale_price,
            data.min_stock,
            data.is_active as i32,
            data.id
        ],
    )
    .map_err(|e| e.to_string())?;

    get_product_by_id(&db, data.id)
}

#[tauri::command]
pub fn delete_product(state: State<DbState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Soft delete - just deactivate
    db.execute(
        "UPDATE products SET is_active = 0, updated_at = datetime('now','localtime') WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn get_product_by_id(db: &rusqlite::Connection, id: i64) -> Result<Product, String> {
    db.query_row(
        "SELECT p.id, p.sku, p.barcode, p.name, p.description, p.category_id, p.supplier_id, 
                p.purchase_price, p.sale_price, p.stock, p.min_stock, p.is_active, 
                p.low_stock_ignored, p.created_at, p.updated_at, c.name as category_name, s.name as supplier_name
         FROM products p
         LEFT JOIN categories c ON p.category_id = c.id
         LEFT JOIN suppliers s ON p.supplier_id = s.id
         WHERE p.id = ?1",
        params![id],
        |row| {
            Ok(Product {
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
            })
        },
    ).map_err(|e| e.to_string())
}
