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
    let db = state.conn();
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
    let db = state.conn();
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
    let user_id = require_admin(&sessions, &token)?;
    let db = state.conn();
    guardar_variantes(&db, user_id, product_id, variants)
}

/// Cambio de existencia de una talla que hay que dejar anotado.
struct Movida {
    variant_id: i64,
    etiqueta: String,
    delta: i32,
}

fn etiqueta_de(size: &Option<String>, color: &Option<String>) -> String {
    let partes: Vec<&str> = [size.as_deref(), color.as_deref()]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect();
    if partes.is_empty() { "Único".to_string() } else { partes.join(" / ") }
}

/// Núcleo del guardado, con la conexión explícita.
///
/// Ajustar la existencia de una talla se hace desde aquí —`adjust_stock` la
/// rechaza a propósito y manda a esta pantalla—, y hasta ahora no dejaba ningún
/// movimiento de inventario. El único lugar al que el propio sistema mandaba a
/// corregir existencias era justo el que no quedaba auditado: una talla podía
/// pasar de ocho a cero, o desaparecer con su mercancía dentro, sin un renglón
/// que dijera quién y cuándo.
pub fn guardar_variantes(
    db: &rusqlite::Connection,
    user_id: i64,
    product_id: i64,
    variants: Vec<SaveVariantDto>,
) -> Result<Vec<ProductVariant>, String> {
    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let norm = |s: &Option<String>| -> Option<String> {
        s.as_ref().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
    };

    let result = (|| -> Result<(), String> {
        // Existencia del producto antes de tocar nada: los movimientos que se
        // anoten abajo encadenan desde aquí.
        let stock_antes: i32 = db
            .query_row("SELECT stock FROM products WHERE id = ?1", params![product_id], |r| r.get(0))
            .map_err(|_| "El producto no existe".to_string())?;
        let mut movidas: Vec<Movida> = Vec::new();

        // Soft-delete variants that are no longer present.
        let incoming_ids: Vec<i64> = variants.iter().filter_map(|v| v.id).collect();
        let mut stmt = db.prepare(
            "SELECT id, size, color, stock FROM product_variants WHERE product_id = ?1 AND is_active = 1"
        ).map_err(|e| e.to_string())?;
        let existing: Vec<(i64, Option<String>, Option<String>, i32)> = stmt
            .query_map(params![product_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        let previo: std::collections::HashMap<i64, i32> =
            existing.iter().map(|(id, _, _, stock)| (*id, *stock)).collect();

        for (id, size, color, stock) in &existing {
            if !incoming_ids.contains(id) {
                db.execute("UPDATE product_variants SET is_active = 0, updated_at = datetime('now','localtime') WHERE id = ?1", params![id])
                    .map_err(|e| e.to_string())?;
                // Quitar la talla se lleva su mercancía del total del producto.
                // Sin este renglón, esas piezas se esfumaban sin explicación.
                if *stock != 0 {
                    movidas.push(Movida {
                        variant_id: *id,
                        etiqueta: format!("Talla quitada ({})", etiqueta_de(size, color)),
                        delta: -*stock,
                    });
                }
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
                    let antes = previo.get(&id).copied().unwrap_or(0);
                    if stock != antes {
                        movidas.push(Movida {
                            variant_id: id,
                            etiqueta: format!("Ajuste de talla ({})", etiqueta_de(&size, &color)),
                            delta: stock - antes,
                        });
                    }
                }
                None => {
                    db.execute(
                        "INSERT INTO product_variants (product_id, size, color, sku, barcode, stock) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![product_id, size, color, sku, barcode, stock],
                    ).map_err(dup_err)?;
                    if stock != 0 {
                        movidas.push(Movida {
                            variant_id: db.last_insert_rowid(),
                            etiqueta: format!("Talla nueva ({})", etiqueta_de(&size, &color)),
                            delta: stock,
                        });
                    }
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

        // Los renglones van al final, encadenados: cada uno dice de qué talla es,
        // cuánto se movió y en qué dejó el total del producto.
        let mut corriendo = stock_antes;
        for m in &movidas {
            let nuevo = corriendo + m.delta;
            db.execute(
                "INSERT INTO inventory_movements (product_id, variant_id, movement_type, quantity, previous_stock, new_stock, reason, user_id)
                 VALUES (?1, ?2, 'adjustment', ?3, ?4, ?5, ?6, ?7)",
                params![product_id, m.variant_id, m.delta, corriendo, nuevo, m.etiqueta, user_id],
            ).map_err(|e| e.to_string())?;
            corriendo = nuevo;
        }

        // Una prenda que hasta ahora no tenía tallas trae existencia propia que
        // no pertenece a ninguna. Al pasar a tallas, esa diferencia también tiene
        // que quedar dicha en vez de aparecer como un descuadre.
        if count > 0 && corriendo != sum {
            db.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reason, user_id)
                 VALUES (?1, 'adjustment', ?2, ?3, ?4, 'Existencia repartida en tallas', ?5)",
                params![product_id, sum - corriendo, corriendo, sum, user_id],
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

    // ── Rastro de los cambios de existencia por talla ────────────────────────
    //
    // `adjust_stock` rechaza los productos con tallas y manda aquí. Si aquí no se
    // anota nada, el único lugar al que el sistema manda a corregir existencias
    // es el único sin auditoría.

    fn variante(id: Option<i64>, size: &str, stock: i32) -> SaveVariantDto {
        SaveVariantDto {
            id,
            size: Some(size.to_string()),
            color: None,
            sku: None,
            barcode: None,
            stock,
        }
    }

    /// Movimientos anotados para un producto: (talla, cantidad, motivo).
    fn movimientos(db: &rusqlite::Connection, product_id: i64) -> Vec<(Option<i64>, i32, String)> {
        let mut stmt = db.prepare(
            "SELECT variant_id, quantity, COALESCE(reason, '') FROM inventory_movements
             WHERE product_id = ?1 ORDER BY id",
        ).unwrap();
        stmt.query_map(params![product_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn ajustar_la_existencia_de_una_talla_deja_movimiento() {
        let db = tienda();
        let (p, m, l) = con_variantes(&db);

        guardar_variantes(&db, 1, p, vec![variante(Some(m), "M", 2), variante(Some(l), "L", 5)]).unwrap();

        let movs = movimientos(&db, p);
        assert_eq!(movs.len(), 1, "se esperaba un solo renglón: {:?}", movs);
        assert_eq!(movs[0].0, Some(m), "el renglón tiene que decir de qué talla es");
        assert_eq!(movs[0].1, -3, "cinco piezas pasaron a dos");
        assert!(movs[0].2.contains('M'), "el motivo nombra la talla: {}", movs[0].2);
        invariante(&db, p, "tras ajustar una talla");
    }

    #[test]
    fn quitar_una_talla_no_desaparece_su_mercancia_en_silencio() {
        // Al desactivar la talla, su existencia salía del total del producto sin
        // un renglón que lo explicara: diez piezas se volvían cinco y nadie sabía
        // por qué.
        let db = tienda();
        let (p, m, l) = con_variantes(&db);

        guardar_variantes(&db, 1, p, vec![variante(Some(m), "M", 5)]).unwrap();

        let movs = movimientos(&db, p);
        assert_eq!(movs.len(), 1, "{:?}", movs);
        assert_eq!(movs[0].0, Some(l));
        assert_eq!(movs[0].1, -5);
        assert!(movs[0].2.contains("quitada"), "{}", movs[0].2);
        invariante(&db, p, "tras quitar una talla");
    }

    #[test]
    fn una_talla_nueva_entra_como_movimiento() {
        let db = tienda();
        let (p, m, l) = con_variantes(&db);

        guardar_variantes(
            &db, 1, p,
            vec![variante(Some(m), "M", 5), variante(Some(l), "L", 5), variante(None, "XL", 4)],
        ).unwrap();

        let movs = movimientos(&db, p);
        assert_eq!(movs.len(), 1, "{:?}", movs);
        assert_eq!(movs[0].1, 4);
        assert!(movs[0].2.contains("nueva"), "{}", movs[0].2);
        assert!(movs[0].0.is_some(), "la talla nueva ya tiene id y debe quedar anotada");
        invariante(&db, p, "tras agregar una talla");
    }

    #[test]
    fn guardar_sin_cambiar_nada_no_ensucia_el_historial() {
        let db = tienda();
        let (p, m, l) = con_variantes(&db);

        guardar_variantes(&db, 1, p, vec![variante(Some(m), "M", 5), variante(Some(l), "L", 5)]).unwrap();

        assert!(movimientos(&db, p).is_empty(), "solo se anota lo que se movió");
    }

    #[test]
    fn los_renglones_encadenan_hasta_el_total_del_producto() {
        // Cada renglón dice en qué dejó el total; el último tiene que coincidir
        // con lo que quedó en el catálogo, o el historial no se puede leer.
        let db = tienda();
        let (p, m, l) = con_variantes(&db);

        guardar_variantes(&db, 1, p, vec![variante(Some(m), "M", 1), variante(Some(l), "L", 9)]).unwrap();

        let ultimo: (i32, i32) = db.query_row(
            "SELECT previous_stock, new_stock FROM inventory_movements
             WHERE product_id = ?1 ORDER BY id DESC LIMIT 1",
            params![p], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        let total: i32 = db.query_row("SELECT stock FROM products WHERE id = ?1", params![p], |r| r.get(0)).unwrap();
        assert_eq!(ultimo.1, total, "el último renglón debe cerrar en el total real");
        assert_eq!(total, 10);
    }

    #[test]
    fn quien_hizo_el_ajuste_queda_anotado() {
        let db = tienda();
        let (p, m, l) = con_variantes(&db);

        guardar_variantes(&db, 1, p, vec![variante(Some(m), "M", 2), variante(Some(l), "L", 5)]).unwrap();

        let quien: Option<i64> = db.query_row(
            "SELECT user_id FROM inventory_movements WHERE product_id = ?1", params![p], |r| r.get(0),
        ).unwrap();
        assert_eq!(quien, Some(1));
    }

    #[test]
    fn pasar_una_prenda_sin_tallas_a_tallas_explica_la_diferencia() {
        // La prenda tenía siete piezas propias y ahora se reparten en dos tallas
        // de dos. Esa diferencia tiene que quedar dicha en vez de verse como un
        // descuadre del catálogo.
        let db = tienda();
        db.execute(
            "INSERT INTO products (sku, name, purchase_price, sale_price, stock) VALUES ('SIN', 'Blusa', 10.0, 20.0, 7)",
            [],
        ).unwrap();
        let p = db.last_insert_rowid();

        guardar_variantes(&db, 1, p, vec![variante(None, "M", 2), variante(None, "L", 2)]).unwrap();

        let movs = movimientos(&db, p);
        assert!(
            movs.iter().any(|(_, _, motivo)| motivo.contains("repartida")),
            "falta el renglón que explica la diferencia: {:?}", movs
        );
        invariante(&db, p, "tras pasar a tallas");
    }
}
