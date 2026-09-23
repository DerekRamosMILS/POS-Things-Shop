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
        Ok(()) => { crate::db::connection::confirmar(db)?; Ok(()) }
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

    let (current_stock, has_variants, costo_antes, nombre, activo): (i32, i32, f64, String, bool) = db
        .query_row(
            "SELECT stock, has_variants, purchase_price, name, is_active FROM products WHERE id = ?1",
            params![data.product_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .map_err(|_| "El producto no existe".to_string())?;

    if has_variants == 1 {
        return Err("Este producto usa variantes; recibe la compra por talla/color en Productos".to_string());
    }
    // Meterle mercancía a una prenda dada de baja la deja con existencia que no
    // se puede vender ni aparece en ninguna lista: se pierde de vista.
    if !activo {
        return Err(format!("'{}' está dado de baja: reactívalo antes de recibirle mercancía.", nombre));
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
        crate::commands::products::anotar_precio(db, data.product_id, "costo", costo_antes, price, user_id)?;
    }

    db.execute(
        "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reason, user_id)
         VALUES (?1, 'purchase', ?2, ?3, ?4, 'Compra/Reabastecimiento', ?5)",
        params![data.product_id, data.quantity, current_stock, new_stock, user_id],
    ).map_err(|e| e.to_string())?;

        Ok(())
    })();

    match result {
        Ok(()) => { crate::db::connection::confirmar(db)?; Ok(()) }
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
    poco_stock(&db)
}

/// Núcleo del aviso de stock bajo, con la conexión explícita.
///
/// Descarta lo que ya se descartó (`low_stock_ignored`). El panel no lo miraba y
/// la lista de productos y las notificaciones sí: apagar un aviso lo callaba en
/// dos lugares de tres, y volvía a aparecer donde más se ve.
pub(crate) fn poco_stock(
    db: &rusqlite::Connection,
) -> Result<Vec<crate::models::product::Product>, String> {
    let mut stmt = db.prepare(
        "SELECT p.id, p.sku, p.barcode, p.name, p.description, p.category_id, p.supplier_id,
                p.purchase_price, p.sale_price, p.stock, p.min_stock, p.is_active,
                p.low_stock_ignored, p.created_at, p.updated_at, c.name as category_name, s.name as supplier_name,
                EXISTS(SELECT 1 FROM product_images WHERE product_id = p.id) as has_image, p.has_variants
         FROM products p
         LEFT JOIN categories c ON p.category_id = c.id
         LEFT JOIN suppliers s ON p.supplier_id = s.id
         WHERE p.stock <= p.min_stock AND p.is_active = 1 AND p.low_stock_ignored = 0
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
    fn historial_de_costos(db: &rusqlite::Connection) -> Vec<(f64, f64)> {
        db.prepare("SELECT old_price, new_price FROM price_history WHERE product_id = 1 AND tipo = 'costo' ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn una_compra_a_otro_costo_deja_el_cambio_en_el_historial() {
        // El costo se reescribía sin rastro, y con él la utilidad de las ventas
        // viejas que no guardaron el suyo.
        let db = tienda();
        registrar_compra(&db, 1, compra(5, Some(62.5))).unwrap();

        assert_eq!(historial_de_costos(&db), vec![(50.0, 62.5)]);
    }

    #[test]
    fn una_compra_al_mismo_costo_o_sin_costo_no_ensucia_el_historial() {
        let db = tienda();
        registrar_compra(&db, 1, compra(5, Some(50.0))).unwrap();
        registrar_compra(&db, 1, compra(5, None)).unwrap();

        assert!(historial_de_costos(&db).is_empty());
        let de_venta: i64 = db.query_row(
            "SELECT COUNT(*) FROM price_history WHERE tipo = 'venta'", [], |r| r.get(0)).unwrap();
        assert_eq!(de_venta, 0);
    }

    #[test]
    fn el_aviso_de_stock_bajo_respeta_lo_que_ya_se_descarto() {
        // Apagar el aviso lo callaba en la lista de productos y en las
        // notificaciones, pero el panel lo seguía enseñando.
        let db = tienda();
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock, min_stock)
             VALUES (90, 'BAJO', 'Se acaba', 1, 2, 1, 5), (91, 'MUDO', 'Ya lo sé', 1, 2, 1, 5)",
            [],
        ).unwrap();
        db.execute("UPDATE products SET low_stock_ignored = 1 WHERE id = 91", []).unwrap();

        let avisados: Vec<String> = poco_stock(&db).unwrap().into_iter().map(|p| p.sku).collect();

        assert!(avisados.contains(&"BAJO".to_string()));
        assert!(!avisados.contains(&"MUDO".to_string()), "lo descartado no vuelve: {:?}", avisados);
    }

    #[test]
    fn el_aviso_de_stock_bajo_deja_fuera_lo_dado_de_baja() {
        let db = tienda();
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock, min_stock, is_active)
             VALUES (92, 'RETIRADO', 'Ya no', 1, 2, 0, 5, 0)",
            [],
        ).unwrap();

        let avisados: Vec<String> = poco_stock(&db).unwrap().into_iter().map(|p| p.sku).collect();
        assert!(!avisados.contains(&"RETIRADO".to_string()));
    }

    #[test]
    fn no_se_le_recibe_mercancia_a_una_prenda_dada_de_baja() {
        // Quedaba con existencia que no se puede vender ni sale en ninguna lista.
        let db = tienda();
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock, is_active)
             VALUES (93, 'FUERA', 'Ya no se vende', 10, 20, 0, 0)",
            [],
        ).unwrap();

        let e = registrar_compra(&db, 1, RegisterPurchaseDto {
            product_id: 93, quantity: 5, purchase_price: None,
        }).unwrap_err();

        assert!(e.contains("dado de baja"), "mensaje inesperado: {}", e);
        let stock: i32 = db.query_row("SELECT stock FROM products WHERE id = 93", [], |r| r.get(0)).unwrap();
        assert_eq!(stock, 0);
    }

    // ─── El invariante del inventario ───────────────────────────────────────
    //
    // La existencia de una prenda tiene que ser siempre la suma de sus
    // movimientos. Es lo que hace que el historial sirva para algo: si no cuadra,
    // el inventario dice un número y su explicación dice otro, y no hay forma de
    // saber cuál está mal. Lo tocan siete caminos —venta, devolución, cancelación
    // de venta, reserva y cancelación de apartado, compra, ajuste y conteo del
    // celular— y ninguna prueba lo miraba entero.

    /// La existencia guardada de un producto y la que dicen sus movimientos.
    fn existencia_y_movimientos(db: &rusqlite::Connection, product_id: i64) -> (i32, i32) {
        let stock: i32 = db
            .query_row("SELECT stock FROM products WHERE id = ?1", params![product_id], |r| r.get(0))
            .unwrap();
        let suma: i32 = db
            .query_row(
                "SELECT COALESCE(SUM(quantity), 0) FROM inventory_movements WHERE product_id = ?1",
                params![product_id],
                |r| r.get(0),
            )
            .unwrap();
        (stock, suma)
    }

    /// Una tienda con caja abierta y una prenda que empieza con su movimiento.
    fn tienda_con_historial() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')", [],
        ).unwrap();
        db.execute("INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, 1000.0)", []).unwrap();
        // Por el camino de verdad, que deja el movimiento del stock inicial.
        crate::commands::products::crear_producto(&db, 1, crate::models::product::CreateProductDto {
            sku: "CAM".into(), barcode: None, name: "Camisa".into(), description: None,
            category_id: None, supplier_id: None,
            purchase_price: 50.0, sale_price: 100.0, stock: 100, min_stock: 2,
        }).unwrap();
        db
    }

    fn vender(db: &rusqlite::Connection, piezas: i32) -> i64 {
        use crate::models::sale::{CreateSaleDto, CreateSaleItemDto};
        crate::commands::sales::registrar_venta(db, 1, None, CreateSaleDto {
            items: vec![CreateSaleItemDto { product_id: 1, quantity: piezas, unit_price: 0.0, discount: 0.0, variant_id: None }],
            payment_method: "cash".to_string(), amount_paid: 1_000_000.0, payments: vec![],
            discount_total: 0.0, promotion_id: None, requiere_factura: false,
            notes: None, customer_id: None, client_request_id: None,
        }).unwrap().id
    }

    #[test]
    fn la_existencia_siempre_es_la_suma_de_sus_movimientos() {
        use crate::commands::layaways::{cancelar_apartado, registrar_apartado};
        use crate::commands::returns::{registrar_devolucion, CreateReturnDto, ReturnItemDto};
        use crate::models::layaway::{CreateLayawayDto, CreateLayawayItemDto};

        let db = tienda_con_historial();
        let revisar = |paso: &str| {
            let (stock, movimientos) = existencia_y_movimientos(&db, 1);
            assert_eq!(stock, movimientos, "tras {}: existencia {} vs movimientos {}", paso, stock, movimientos);
        };
        revisar("el alta");

        // Venta.
        let venta = vender(&db, 7);
        revisar("una venta");

        // Devolución parcial de esa venta.
        let partida: i64 = db
            .query_row("SELECT id FROM sale_items WHERE sale_id = ?1", params![venta], |r| r.get(0))
            .unwrap();
        registrar_devolucion(&db, 1, CreateReturnDto {
            sale_id: venta, reason: None, refund_method: "cash".to_string(),
            items: vec![ReturnItemDto { sale_item_id: partida, quantity: 3 }],
        }).unwrap();
        revisar("una devolución parcial");

        // Otra venta, cancelada entera.
        let cancelable = vender(&db, 5);
        revisar("otra venta");
        crate::commands::sales::cancelar_venta(&db, 1, cancelable).unwrap();
        revisar("cancelar esa venta");

        // Compra.
        registrar_compra(&db, 1, RegisterPurchaseDto { product_id: 1, quantity: 20, purchase_price: Some(55.0) }).unwrap();
        revisar("una compra");

        // Ajuste a mano, en los dos sentidos.
        ajustar_stock(&db, 1, AdjustStockDto { product_id: 1, quantity: -4, reason: "Merma".into() }).unwrap();
        revisar("un ajuste negativo");
        ajustar_stock(&db, 1, AdjustStockDto { product_id: 1, quantity: 9, reason: "Apareció".into() }).unwrap();
        revisar("un ajuste positivo");

        // Apartado: reserva y cancelación.
        let apartado = registrar_apartado(&db, 1, CreateLayawayDto {
            customer_id: None,
            items: vec![CreateLayawayItemDto { product_id: 1, quantity: 6, unit_price: 0.0, variant_id: None }],
            initial_payment: 0.0, payment_method: "cash".to_string(), notes: None, due_date: None,
        }).unwrap();
        revisar("reservar un apartado");
        cancelar_apartado(&db, 1, apartado.id).unwrap();
        revisar("cancelar el apartado");

        // Conteo desde el celular, hacia arriba y hacia abajo.
        let (antes, _) = existencia_y_movimientos(&db, 1);
        for contado in [antes + 11, antes - 5] {
            let id = format!("c-{}", contado);
            crate::capture::conteo::aplicar_conteo(&db, Some(1), &crate::capture::conteo::ConteoDelCelular {
                conteo_id: id,
                sku: "CAM".to_string(),
                variant_id: None,
                contado,
                contado_en: "2099-01-01 10:00:00".to_string(),
            }).unwrap();
            revisar("un conteo del celular");
        }

        // Y que de verdad hubo movimiento: si todo diera cero, el invariante se
        // cumpliría trivialmente y esta prueba no valdría nada.
        let cuantos: i64 = db
            .query_row("SELECT COUNT(*) FROM inventory_movements WHERE product_id = 1", [], |r| r.get(0))
            .unwrap();
        assert!(cuantos >= 9, "se esperaban movimientos de todos los caminos, hubo {}", cuantos);
        let (stock, _) = existencia_y_movimientos(&db, 1);
        assert!(stock > 0, "la prenda tiene que quedar con existencia");
    }

    /// La existencia de una talla y la que dicen sus movimientos.
    fn talla_y_movimientos(db: &rusqlite::Connection, variant_id: i64) -> (i32, i32) {
        let stock: i32 = db
            .query_row("SELECT stock FROM product_variants WHERE id = ?1", params![variant_id], |r| r.get(0))
            .unwrap();
        let suma: i32 = db
            .query_row(
                "SELECT COALESCE(SUM(quantity), 0) FROM inventory_movements WHERE variant_id = ?1",
                params![variant_id],
                |r| r.get(0),
            )
            .unwrap();
        (stock, suma)
    }

    /// El total del producto y la suma de sus tallas activas.
    fn total_y_suma_de_tallas(db: &rusqlite::Connection, product_id: i64) -> (i32, i32) {
        let stock: i32 = db
            .query_row("SELECT stock FROM products WHERE id = ?1", params![product_id], |r| r.get(0))
            .unwrap();
        let suma: i32 = db
            .query_row(
                "SELECT COALESCE(SUM(stock), 0) FROM product_variants WHERE product_id = ?1 AND is_active = 1",
                params![product_id],
                |r| r.get(0),
            )
            .unwrap();
        (stock, suma)
    }

    #[test]
    fn cada_talla_es_la_suma_de_sus_movimientos_y_el_total_la_suma_de_las_tallas() {
        // Dos invariantes encadenados. El de las tallas es el que más caminos tiene
        // y el que ya se rompió una vez: hasta la migración 024, cambiar la
        // existencia de una talla desde Productos no dejaba ningún movimiento.
        use crate::commands::returns::{registrar_devolucion, CreateReturnDto, ReturnItemDto};
        use crate::commands::variants::guardar_variantes;
        use crate::models::sale::{CreateSaleDto, CreateSaleItemDto};
        use crate::models::variant::SaveVariantDto;

        let db = tienda_con_historial();

        // `stock_original: None` es "vengo a poner esta existencia", que es lo que
        // hace el formulario cuando alguien la escribe a mano.
        let talla = |id: Option<i64>, size: &str, stock: i32| SaveVariantDto {
            id, size: Some(size.to_string()), color: None, sku: None, barcode: None, stock,
            stock_original: None,
        };

        let puestas = guardar_variantes(&db, 1, 1, vec![
            talla(None, "M", 8), talla(None, "G", 12), talla(None, "XG", 5),
        ]).unwrap();
        let ids: Vec<i64> = puestas.iter().map(|v| v.id).collect();

        let revisar = |paso: &str| {
            for id in &ids {
                let existe: i64 = db
                    .query_row("SELECT COUNT(*) FROM product_variants WHERE id = ?1 AND is_active = 1", params![id], |r| r.get(0))
                    .unwrap();
                if existe == 0 { continue; }
                let (stock, mov) = talla_y_movimientos(&db, *id);
                assert_eq!(stock, mov, "tras {}: la talla {} tiene {} y sus movimientos dicen {}", paso, id, stock, mov);
            }
            let (total, suma) = total_y_suma_de_tallas(&db, 1);
            assert_eq!(total, suma, "tras {}: el total dice {} y las tallas suman {}", paso, total, suma);
            // Y el invariante de arriba, que también tiene que valer con tallas: el
            // total del producto es la suma de **todos** sus movimientos, los de
            // talla y los que no la llevan. Es lo que hace que una talla quitada no
            // pueda llevarse su mercancía sin dejar rastro.
            let (stock, movimientos) = existencia_y_movimientos(&db, 1);
            assert_eq!(
                stock, movimientos,
                "tras {}: el producto tiene {} y sus movimientos dicen {}", paso, stock, movimientos
            );
        };
        revisar("poner las tallas");

        // Vender una talla.
        let venta = crate::commands::sales::registrar_venta(&db, 1, None, CreateSaleDto {
            items: vec![CreateSaleItemDto { product_id: 1, quantity: 3, unit_price: 0.0, discount: 0.0, variant_id: Some(ids[0]) }],
            payment_method: "cash".to_string(), amount_paid: 100_000.0, payments: vec![],
            discount_total: 0.0, promotion_id: None, requiere_factura: false,
            notes: None, customer_id: None, client_request_id: None,
        }).unwrap().id;
        revisar("vender una talla");

        // Devolver parte.
        let partida: i64 = db
            .query_row("SELECT id FROM sale_items WHERE sale_id = ?1", params![venta], |r| r.get(0))
            .unwrap();
        registrar_devolucion(&db, 1, CreateReturnDto {
            sale_id: venta, reason: None, refund_method: "cash".to_string(),
            items: vec![ReturnItemDto { sale_item_id: partida, quantity: 1 }],
        }).unwrap();
        revisar("devolver una pieza de esa talla");

        // Contar una talla desde el celular.
        crate::capture::conteo::aplicar_conteo(&db, Some(1), &crate::capture::conteo::ConteoDelCelular {
            conteo_id: "ct-1".to_string(), sku: "CAM".to_string(),
            variant_id: Some(ids[1]), contado: 20,
            contado_en: "2099-01-01 10:00:00".to_string(),
        }).unwrap();
        revisar("contar una talla");

        // Cambiar existencias desde Productos: el caso de la migración 024.
        guardar_variantes(&db, 1, 1, vec![
            talla(Some(ids[0]), "M", 30), talla(Some(ids[1]), "G", 1), talla(Some(ids[2]), "XG", 5),
        ]).unwrap();
        revisar("cambiar existencias desde Productos");

        // Y quitar una talla, que se lleva su mercancía del total.
        guardar_variantes(&db, 1, 1, vec![
            talla(Some(ids[0]), "M", 30), talla(Some(ids[1]), "G", 1),
        ]).unwrap();
        revisar("quitar una talla");

        let cuantos: i64 = db
            .query_row("SELECT COUNT(*) FROM inventory_movements WHERE variant_id IS NOT NULL", [], |r| r.get(0))
            .unwrap();
        assert!(cuantos >= 7, "se esperaban movimientos por talla de todos los caminos, hubo {}", cuantos);
    }
}
