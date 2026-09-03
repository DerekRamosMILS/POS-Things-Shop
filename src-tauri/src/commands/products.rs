use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::product::{CreateProductDto, Product, ProductFilters, UpdateProductDto};
use crate::session::{require_admin, require_auth, SessionState};

// ─── Shared SELECT fragment ────────────────────────────────────────────────────
//
// Las fotos se guardan como data URL dentro de la propia fila (~54 KB cada una),
// así que devolverlas en los listados haría que abrir el punto de venta con 500
// productos moviera decenas de megabytes por IPC. El listado solo informa si el
// producto *tiene* foto; la imagen se pide aparte para los que están a la vista.
const SEL: &str = "SELECT p.id, p.sku, p.barcode, p.name, p.description, p.category_id, p.supplier_id,
                          p.purchase_price, p.sale_price, p.stock, p.min_stock, p.is_active,
                          p.low_stock_ignored, p.created_at, p.updated_at,
                          c.name as category_name, s.name as supplier_name,
                          (p.image_url IS NOT NULL AND p.image_url != '') as has_image,
                          p.has_variants
                   FROM products p
                   LEFT JOIN categories c ON p.category_id = c.id
                   LEFT JOIN suppliers s ON p.supplier_id = s.id";

fn row_to_product(row: &rusqlite::Row) -> rusqlite::Result<Product> {
    Ok(Product {
        id:              row.get(0)?,
        sku:             row.get(1)?,
        barcode:         row.get(2)?,
        name:            row.get(3)?,
        description:     row.get(4)?,
        category_id:     row.get(5)?,
        supplier_id:     row.get(6)?,
        purchase_price:  row.get(7)?,
        sale_price:      row.get(8)?,
        stock:           row.get(9)?,
        min_stock:       row.get(10)?,
        is_active:       row.get::<_, i32>(11)? == 1,
        low_stock_ignored: row.get::<_, i32>(12)? == 1,
        created_at:      row.get(13)?,
        updated_at:      row.get(14)?,
        category_name:   row.get(15)?,
        supplier_name:   row.get(16)?,
        image_url:       None,
        has_image:       row.get::<_, i32>(17)? == 1,
        has_variants:    row.get::<_, i32>(18)? == 1,
    })
}

#[tauri::command]
pub fn get_products(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    filters: Option<ProductFilters>,
) -> Result<Vec<Product>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let filters = filters.unwrap_or_default();

    let mut sql = format!("{} WHERE 1=1", SEL);
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
        sql.push_str(" AND p.stock <= p.min_stock AND p.low_stock_ignored = 0");
    }
    sql.push_str(" ORDER BY p.name ASC");

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        param_values.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let products = stmt
        .query_map(params_refs.as_slice(), row_to_product)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(products)
}

#[tauri::command]
pub fn get_product_by_barcode(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    barcode: String,
) -> Result<Option<Product>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let sql = format!("{} WHERE p.barcode = ?1 AND p.is_active = 1", SEL);
    let result = db.query_row(&sql, params![barcode], row_to_product);

    match result {
        Ok(product) => Ok(Some(product)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn create_product(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreateProductDto) -> Result<Product, String> {
    require_admin(&sessions, &token)?;
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
            if e.to_string().contains("sku") { "El SKU ya existe".to_string() }
            else { "El código de barras ya existe".to_string() }
        } else { e.to_string() }
    })?;

    let id = db.last_insert_rowid();

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
pub fn update_product(state: State<DbState>, sessions: State<SessionState>, token: String, data: UpdateProductDto) -> Result<Product, String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let old_price: f64 = db
        .query_row("SELECT sale_price FROM products WHERE id = ?1", params![data.id], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    if (old_price - data.sale_price).abs() > 0.001 {
        db.execute(
            "INSERT INTO price_history (product_id, old_price, new_price) VALUES (?1, ?2, ?3)",
            params![data.id, old_price, data.sale_price],
        ).map_err(|e| e.to_string())?;
    }

    db.execute(
        "UPDATE products SET sku=?1, barcode=?2, name=?3, description=?4,
         category_id=?5, supplier_id=?6, purchase_price=?7, sale_price=?8,
         min_stock=?9, is_active=?10, updated_at=datetime('now','localtime')
         WHERE id=?11",
        params![
            data.sku, data.barcode, data.name, data.description,
            data.category_id, data.supplier_id, data.purchase_price,
            data.sale_price, data.min_stock, data.is_active as i32, data.id
        ],
    ).map_err(|e| e.to_string())?;

    get_product_by_id(&db, data.id)
}

#[tauri::command]
pub fn set_product_image(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    product_id: i64,
    image_url: Option<String>,
) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE products SET image_url=?1, updated_at=datetime('now','localtime') WHERE id=?2",
        params![image_url, product_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_product(state: State<DbState>, sessions: State<SessionState>, token: String, id: i64) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE products SET is_active = 0, updated_at = datetime('now','localtime') WHERE id = ?1",
        params![id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

fn get_product_by_id(db: &rusqlite::Connection, id: i64) -> Result<Product, String> {
    let sql = format!("{} WHERE p.id = ?1", SEL);
    db.query_row(&sql, params![id], row_to_product)
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct PriceHistoryEntry {
    pub id: i64,
    pub old_price: f64,
    pub new_price: f64,
    pub user_name: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub fn get_price_history(state: State<DbState>, sessions: State<SessionState>, token: String, product_id: i64) -> Result<Vec<PriceHistoryEntry>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db.prepare(
        "SELECT ph.id, ph.old_price, ph.new_price, u.full_name, ph.created_at
         FROM price_history ph
         LEFT JOIN users u ON ph.changed_by = u.id
         WHERE ph.product_id = ?1
         ORDER BY ph.created_at DESC"
    ).map_err(|e| e.to_string())?;
    let out = stmt
        .query_map(params![product_id], |row| {
            Ok(PriceHistoryEntry {
                id: row.get(0)?,
                old_price: row.get(1)?,
                new_price: row.get(2)?,
                user_name: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(out)
}
