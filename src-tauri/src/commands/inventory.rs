use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::inventory::{AdjustStockDto, InventoryMovement, RegisterPurchaseDto};
use crate::session::{require_admin, require_auth, SessionState};

/// Columnas enumeradas a propósito. Con `im.*`, el día que una migración agregue
/// una columna a la tabla se recorren todos los índices y el mapeo empieza a
/// leer un campo por otro sin que nada falle a la vista.
const MOVIMIENTO_COLUMNAS: &str = "im.id, im.product_id, im.movement_type, im.quantity,
    im.previous_stock, im.new_stock, im.reference_id, im.reason, im.user_id, im.created_at,
    p.name as product_name, u.full_name as user_name";

#[tauri::command]
pub fn get_inventory_movements(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    product_id: Option<i64>,
    limit: Option<i32>,
) -> Result<Vec<InventoryMovement>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

    let limit = limit.unwrap_or(200);

    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
        if let Some(pid) = product_id {
            (
                format!(
                    "SELECT {} FROM inventory_movements im
                     LEFT JOIN products p ON im.product_id = p.id
                     LEFT JOIN users u ON im.user_id = u.id
                     WHERE im.product_id = ?1
                     ORDER BY im.created_at DESC LIMIT ?2",
                    MOVIMIENTO_COLUMNAS
                ),
                vec![
                    Box::new(pid) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(limit),
                ],
            )
        } else {
            (
                format!(
                    "SELECT {} FROM inventory_movements im
                     LEFT JOIN products p ON im.product_id = p.id
                     LEFT JOIN users u ON im.user_id = u.id
                     ORDER BY im.created_at DESC LIMIT ?1",
                    MOVIMIENTO_COLUMNAS
                ),
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
    let db = state.conn();
    ajustar_stock(&db, user_id, data)
}

/// Núcleo del ajuste. Mover la existencia y registrar el movimiento van juntos
/// en una transacción: un stock que cambia sin renglón que lo explique deja el
/// inventario sin auditoría.
pub fn ajustar_stock(
    db: &rusqlite::Connection,
    user_id: i64,
    data: AdjustStockDto,
) -> Result<(), String> {
    if data.quantity == 0 {
        return Err("El ajuste debe ser distinto de cero".to_string());
    }
    if data.reason.trim().is_empty() {
        return Err("Indica el motivo del ajuste".to_string());
    }

    let (current_stock, has_variants): (i32, i32) = db
        .query_row(
            "SELECT stock, has_variants FROM products WHERE id = ?1",
            params![data.product_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "El producto no existe".to_string())?;

    if has_variants == 1 {
        return Err("Este producto usa variantes; ajusta el stock por talla/color en Productos".to_string());
    }

    let new_stock = current_stock + data.quantity;
    if new_stock < 0 {
        return Err("El stock no puede ser negativo".to_string());
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {

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
    })();

    match result {
        Ok(()) => { db.execute_batch("COMMIT;").map_err(|e| e.to_string())?; Ok(()) }
        Err(e) => { db.execute_batch("ROLLBACK;").ok(); Err(e) }
    }
}

#[tauri::command]
pub fn register_purchase(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    data: RegisterPurchaseDto,
) -> Result<(), String> {
    let user_id = require_admin(&sessions, &token)?;
    let db = state.conn();
    registrar_compra(&db, user_id, data)
}

/// Núcleo de la entrada por compra.
pub fn registrar_compra(
    db: &rusqlite::Connection,
    user_id: i64,
    data: RegisterPurchaseDto,
) -> Result<(), String> {
    // Sin esto, una cantidad negativa restaría existencia y quedaría anotada
    // como si hubiera entrado mercancía.
    if data.quantity <= 0 {
        return Err("La cantidad recibida debe ser mayor a cero".to_string());
    }
    if data.purchase_price.is_some_and(|p| p < 0.0) {
        return Err("El costo de compra no puede ser negativo".to_string());
    }

    let (current_stock, has_variants): (i32, i32) = db
        .query_row(
            "SELECT stock, has_variants FROM products WHERE id = ?1",
            params![data.product_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| "El producto no existe".to_string())?;

    if has_variants == 1 {
        return Err("Este producto usa variantes; recibe la compra por talla/color en Productos".to_string());
    }

    let new_stock = current_stock + data.quantity;

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {

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
    })();

    match result {
        Ok(()) => { db.execute_batch("COMMIT;").map_err(|e| e.to_string())?; Ok(()) }
        Err(e) => { db.execute_batch("ROLLBACK;").ok(); Err(e) }
    }
}

#[tauri::command]
pub fn get_low_stock_products(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
) -> Result<Vec<crate::models::product::Product>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

    let mut stmt = db.prepare(
        "SELECT p.id, p.sku, p.barcode, p.name, p.description, p.category_id, p.supplier_id,
                p.purchase_price, p.sale_price, p.stock, p.min_stock, p.is_active,
                p.low_stock_ignored, p.created_at, p.updated_at, c.name as category_name, s.name as supplier_name,
                EXISTS(SELECT 1 FROM product_images WHERE product_id = p.id) as has_image, p.has_variants
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::inventory::{AdjustStockDto, RegisterPurchaseDto};

    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')", [],
        ).unwrap();
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (1, 'CAM', 'Camisa', 50.0, 100.0, 10)", [],
        ).unwrap();
        db
    }

    fn stock(db: &rusqlite::Connection) -> i32 {
        db.query_row("SELECT stock FROM products WHERE id = 1", [], |r| r.get(0)).unwrap()
    }

    fn movimientos(db: &rusqlite::Connection) -> i64 {
        db.query_row("SELECT COUNT(*) FROM inventory_movements", [], |r| r.get(0)).unwrap()
    }

    fn ajuste(cantidad: i32, motivo: &str) -> AdjustStockDto {
        AdjustStockDto { product_id: 1, quantity: cantidad, reason: motivo.to_string() }
    }

    fn compra(cantidad: i32, precio: Option<f64>) -> RegisterPurchaseDto {
        RegisterPurchaseDto { product_id: 1, quantity: cantidad, purchase_price: precio }
    }

    #[test]
    fn un_ajuste_mueve_la_existencia_y_deja_constancia() {
        let db = tienda();
        ajustar_stock(&db, 1, ajuste(-3, "merma")).unwrap();

        assert_eq!(stock(&db), 7);
        assert_eq!(movimientos(&db), 1);
    }

    #[test]
    fn un_ajuste_no_puede_dejar_la_existencia_en_negativo() {
        let db = tienda();
        assert!(ajustar_stock(&db, 1, ajuste(-50, "merma")).is_err());

        assert_eq!(stock(&db), 10);
        assert_eq!(movimientos(&db), 0, "no debe quedar un movimiento sin efecto");
    }

    #[test]
    fn un_ajuste_de_cero_se_rechaza() {
        let db = tienda();
        assert!(ajustar_stock(&db, 1, ajuste(0, "nada")).is_err());
    }

    #[test]
    fn un_ajuste_sin_motivo_se_rechaza() {
        // El motivo es lo único que explica una merma en la auditoría.
        let db = tienda();
        assert!(ajustar_stock(&db, 1, ajuste(-1, "   ")).is_err());
        assert_eq!(stock(&db), 10);
    }

    #[test]
    fn no_se_ajusta_a_mano_un_producto_con_variantes() {
        let db = tienda();
        db.execute("UPDATE products SET has_variants = 1 WHERE id = 1", []).unwrap();

        assert!(ajustar_stock(&db, 1, ajuste(5, "recuento")).is_err(),
                "desincronizaría el producto con sus tallas");
    }

    #[test]
    fn una_compra_suma_existencia_y_actualiza_el_costo() {
        let db = tienda();
        registrar_compra(&db, 1, compra(20, Some(60.0))).unwrap();

        assert_eq!(stock(&db), 30);
        let costo: f64 = db.query_row(
            "SELECT purchase_price FROM products WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(costo, 60.0);
    }

    #[test]
    fn una_compra_negativa_se_rechaza() {
        // Antes restaba existencia y la anotaba como si hubiera entrado mercancía.
        let db = tienda();
        assert!(registrar_compra(&db, 1, compra(-5, None)).is_err());

        assert_eq!(stock(&db), 10);
        assert_eq!(movimientos(&db), 0);
    }

    #[test]
    fn una_compra_con_costo_negativo_se_rechaza() {
        let db = tienda();
        assert!(registrar_compra(&db, 1, compra(5, Some(-10.0))).is_err());
        assert_eq!(stock(&db), 10);
    }

    #[test]
    fn una_compra_sin_costo_conserva_el_anterior() {
        let db = tienda();
        registrar_compra(&db, 1, compra(5, None)).unwrap();

        let costo: f64 = db.query_row(
            "SELECT purchase_price FROM products WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(costo, 50.0);
    }

    #[test]
    fn recibir_mercancia_reactiva_el_aviso_de_stock_bajo() {
        let db = tienda();
        db.execute("UPDATE products SET low_stock_ignored = 1 WHERE id = 1", []).unwrap();

        registrar_compra(&db, 1, compra(5, None)).unwrap();

        let ignorado: i32 = db.query_row(
            "SELECT low_stock_ignored FROM products WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(ignorado, 0);
    }

    #[test]
    fn un_producto_inexistente_da_un_mensaje_claro() {
        let db = tienda();
        let err = ajustar_stock(&db, 1, AdjustStockDto {
            product_id: 999, quantity: 1, reason: "x".to_string(),
        }).unwrap_err();
        assert!(err.contains("no existe"), "mensaje poco claro: {}", err);
    }
}
