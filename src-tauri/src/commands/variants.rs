use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::variant::{ProductVariant, SaveVariantDto};
use crate::session::{require_admin, require_auth, SessionState};

fn row_to_variant(row: &rusqlite::Row) -> rusqlite::Result<ProductVariant> {
    Ok(ProductVariant {
        id: row.get(0)?,
        product_id: row.get(1)?,
        size: row.get(2)?,
        color: row.get(3)?,
        sku: row.get(4)?,
        barcode: row.get(5)?,
        stock: row.get(6)?,
        is_active: row.get::<_, i32>(7)? == 1,
    })
}

const SEL: &str = "SELECT id, product_id, size, color, sku, barcode, stock, is_active FROM product_variants";

#[tauri::command]
pub fn get_variants(state: State<DbState>, sessions: State<SessionState>, token: String, product_id: i64) -> Result<Vec<ProductVariant>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare(&format!("{} WHERE product_id = ?1 AND is_active = 1 ORDER BY id ASC", SEL))
        .map_err(|e| e.to_string())?;
    let out = stmt
        .query_map(params![product_id], row_to_variant)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[tauri::command]
pub fn get_variant_by_barcode(state: State<DbState>, sessions: State<SessionState>, token: String, barcode: String) -> Result<Option<ProductVariant>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let result = db.query_row(
        &format!("{} WHERE barcode = ?1 AND is_active = 1", SEL),
        params![barcode],
        row_to_variant,
    );
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Replace the variant set for a product. Existing variants missing from the
/// incoming list are soft-deleted (kept for historical sales). Afterwards the
/// product's aggregate stock and has_variants flag are recomputed.
#[tauri::command]
pub fn save_variants(state: State<DbState>, sessions: State<SessionState>, token: String, product_id: i64, variants: Vec<SaveVariantDto>) -> Result<Vec<ProductVariant>, String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let norm = |s: &Option<String>| -> Option<String> {
        s.as_ref().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
    };

    let result = (|| -> Result<(), String> {
        // Soft-delete variants that are no longer present.
        let incoming_ids: Vec<i64> = variants.iter().filter_map(|v| v.id).collect();
        let mut stmt = db.prepare("SELECT id FROM product_variants WHERE product_id = ?1 AND is_active = 1")
            .map_err(|e| e.to_string())?;
        let existing: Vec<i64> = stmt
            .query_map(params![product_id], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for id in existing {
            if !incoming_ids.contains(&id) {
                db.execute("UPDATE product_variants SET is_active = 0, updated_at = datetime('now','localtime') WHERE id = ?1", params![id])
                    .map_err(|e| e.to_string())?;
            }
        }

        let dup_err = |e: rusqlite::Error| {
            let s = e.to_string();
            if s.contains("UNIQUE") { "El SKU o código de barras de una variante ya existe".to_string() } else { s }
        };

        for v in &variants {
            let size = norm(&v.size);
            let color = norm(&v.color);
            let sku = norm(&v.sku);
            let barcode = norm(&v.barcode);
            let stock = v.stock.max(0);
            match v.id {
                Some(id) => {
                    db.execute(
                        "UPDATE product_variants SET size=?1, color=?2, sku=?3, barcode=?4, stock=?5, is_active=1, updated_at=datetime('now','localtime') WHERE id=?6 AND product_id=?7",
                        params![size, color, sku, barcode, stock, id, product_id],
                    ).map_err(dup_err)?;
                }
                None => {
                    db.execute(
                        "INSERT INTO product_variants (product_id, size, color, sku, barcode, stock) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![product_id, size, color, sku, barcode, stock],
                    ).map_err(dup_err)?;
                }
            }
        }

        // Recompute aggregate stock + flag.
        let (count, sum): (i64, i32) = db.query_row(
            "SELECT COUNT(*), COALESCE(SUM(stock), 0) FROM product_variants WHERE product_id = ?1 AND is_active = 1",
            params![product_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(|e| e.to_string())?;

        if count > 0 {
            db.execute(
                "UPDATE products SET has_variants = 1, stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![sum, product_id],
            ).map_err(|e| e.to_string())?;
        } else {
            db.execute(
                "UPDATE products SET has_variants = 0, updated_at = datetime('now','localtime') WHERE id = ?1",
                params![product_id],
            ).map_err(|e| e.to_string())?;
        }

        Ok(())
    })();

    match result {
        Ok(()) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            let mut stmt = db
                .prepare(&format!("{} WHERE product_id = ?1 AND is_active = 1 ORDER BY id ASC", SEL))
                .map_err(|e| e.to_string())?;
            let out = stmt
                .query_map(params![product_id], row_to_variant)
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            Ok(out)
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

/// El stock de un producto con variantes debe ser siempre la suma de las suyas.
///
/// Cada operación que mueve inventario toca las dos tablas por separado, así que
/// basta con que una se olvide para que el catálogo empiece a mentir. Estas
/// pruebas recorren el ciclo completo comprobando el invariante en cada paso.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::layaways::{cancelar_apartado, registrar_apartado};
    use crate::commands::returns::registrar_devolucion;
    use crate::commands::sales::{cancelar_venta, registrar_venta};
    use crate::models::layaway::{CreateLayawayDto, CreateLayawayItemDto};
    use crate::models::sale::{CreateSaleDto, CreateSaleItemDto};

    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')", [],
        ).unwrap();
        db.execute("INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, 0)", []).unwrap();
        db
    }

    /// Producto con dos variantes de 5 piezas cada una.
    fn con_variantes(db: &rusqlite::Connection) -> (i64, i64, i64) {
        db.execute(
            "INSERT INTO products (sku, name, purchase_price, sale_price, stock, has_variants)
             VALUES ('CAM', 'Camisa', 50.0, 100.0, 0, 1)", [],
        ).unwrap();
        let p = db.last_insert_rowid();
        db.execute(
            "INSERT INTO product_variants (product_id, size, stock) VALUES (?1, 'M', 5)",
            params![p],
        ).unwrap();
        let m = db.last_insert_rowid();
        db.execute(
            "INSERT INTO product_variants (product_id, size, stock) VALUES (?1, 'L', 5)",
            params![p],
        ).unwrap();
        let l = db.last_insert_rowid();
        db.execute("UPDATE products SET stock = 10 WHERE id = ?1", params![p]).unwrap();
        (p, m, l)
    }

    fn invariante(db: &rusqlite::Connection, product_id: i64, paso: &str) {
        let (total, suma): (i32, i32) = db.query_row(
            "SELECT p.stock, COALESCE((SELECT SUM(stock) FROM product_variants
                                       WHERE product_id = p.id AND is_active = 1), 0)
             FROM products p WHERE p.id = ?1",
            params![product_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(total, suma, "el stock dejó de cuadrar con sus variantes tras {}", paso);
    }

    fn vender(db: &rusqlite::Connection, product_id: i64, variant_id: i64, cantidad: i32) -> i64 {
        registrar_venta(db, 1, None, CreateSaleDto {
            items: vec![CreateSaleItemDto {
                product_id, quantity: cantidad, unit_price: 0.0, discount: 0.0,
                variant_id: Some(variant_id),
            }],
            payment_method: "cash".to_string(),
            amount_paid: 100_000.0,
            payments: vec![], discount_total: 0.0, promotion_id: None,
            requiere_factura: false, notes: None, customer_id: None, client_request_id: None,
        }).unwrap().id
    }

    #[test]
    fn vender_una_variante_mantiene_el_invariante() {
        let db = tienda();
        let (p, m, _) = con_variantes(&db);

        vender(&db, p, m, 2);

        invariante(&db, p, "una venta");
        let stock_m: i32 = db.query_row(
            "SELECT stock FROM product_variants WHERE id = ?1", params![m], |r| r.get(0)).unwrap();
        assert_eq!(stock_m, 3, "solo baja la talla vendida");
    }

    #[test]
    fn cancelar_la_venta_mantiene_el_invariante() {
        let db = tienda();
        let (p, m, _) = con_variantes(&db);
        let venta = vender(&db, p, m, 3);

        cancelar_venta(&db, 1, venta).unwrap();

        invariante(&db, p, "cancelar la venta");
        let stock_m: i32 = db.query_row(
            "SELECT stock FROM product_variants WHERE id = ?1", params![m], |r| r.get(0)).unwrap();
        assert_eq!(stock_m, 5, "la talla vuelve a su existencia original");
    }

    #[test]
    fn devolver_una_variante_mantiene_el_invariante() {
        let db = tienda();
        let (p, m, _) = con_variantes(&db);
        let venta = vender(&db, p, m, 2);
        let partida: i64 = db.query_row(
            "SELECT id FROM sale_items WHERE sale_id = ?1", params![venta], |r| r.get(0)).unwrap();

        registrar_devolucion(&db, 1, crate::commands::returns::CreateReturnDto {
            sale_id: venta, reason: None, refund_method: "cash".to_string(),
            items: vec![crate::commands::returns::ReturnItemDto { sale_item_id: partida, quantity: 1 }],
        }).unwrap();

        invariante(&db, p, "una devolución");
    }

    #[test]
    fn apartar_y_cancelar_una_variante_mantiene_el_invariante() {
        let db = tienda();
        let (p, m, _) = con_variantes(&db);

        let l = registrar_apartado(&db, 1, CreateLayawayDto {
            customer_id: None, notes: None, due_date: None,
            initial_payment: 0.0, payment_method: "cash".to_string(),
            items: vec![CreateLayawayItemDto {
                product_id: p, quantity: 2, unit_price: 0.0, variant_id: Some(m),
            }],
        }).unwrap();
        invariante(&db, p, "crear el apartado");

        cancelar_apartado(&db, 1, l.id).unwrap();
        invariante(&db, p, "cancelar el apartado");

        let stock_m: i32 = db.query_row(
            "SELECT stock FROM product_variants WHERE id = ?1", params![m], |r| r.get(0)).unwrap();
        assert_eq!(stock_m, 5);
    }

    #[test]
    fn no_se_vende_una_variante_sin_existencia_aunque_el_producto_tenga() {
        let db = tienda();
        let (p, m, _) = con_variantes(&db);
        db.execute("UPDATE product_variants SET stock = 0 WHERE id = ?1", params![m]).unwrap();
        db.execute("UPDATE products SET stock = 5 WHERE id = ?1", params![p]).unwrap();

        let r = registrar_venta(&db, 1, None, CreateSaleDto {
            items: vec![CreateSaleItemDto {
                product_id: p, quantity: 1, unit_price: 0.0, discount: 0.0, variant_id: Some(m),
            }],
            payment_method: "cash".to_string(), amount_paid: 1000.0,
            payments: vec![], discount_total: 0.0, promotion_id: None,
            requiere_factura: false, notes: None, customer_id: None, client_request_id: None,
        });
        assert!(r.is_err(), "la talla agotada no debe poder venderse");
    }

    #[test]
    fn una_variante_de_otro_producto_no_se_puede_vender() {
        let db = tienda();
        let (p, _, _) = con_variantes(&db);
        db.execute(
            "INSERT INTO products (sku, name, purchase_price, sale_price, stock)
             VALUES ('OTRO', 'Otro', 1.0, 2.0, 10)", [],
        ).unwrap();
        let otro = db.last_insert_rowid();
        db.execute(
            "INSERT INTO product_variants (product_id, size, stock) VALUES (?1, 'XL', 5)",
            params![otro],
        ).unwrap();
        let ajena = db.last_insert_rowid();

        let r = registrar_venta(&db, 1, None, CreateSaleDto {
            items: vec![CreateSaleItemDto {
                product_id: p, quantity: 1, unit_price: 0.0, discount: 0.0, variant_id: Some(ajena),
            }],
            payment_method: "cash".to_string(), amount_paid: 1000.0,
            payments: vec![], discount_total: 0.0, promotion_id: None,
            requiere_factura: false, notes: None, customer_id: None, client_request_id: None,
        });
        assert!(r.is_err(), "una variante debe pertenecer al producto que se vende");
    }
}
