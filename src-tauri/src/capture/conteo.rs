//! Contar la mercancía con el teléfono, aunque la caja esté apagada.
//!
//! El problema no es apuntar cuánto hay: es que la tienda sigue vendiendo
//! mientras se cuenta, y el conteo puede llegar horas después. Entre que alguien
//! anota "hay doce" a las tres de la tarde y que la caja lo recibe a las ocho,
//! pudieron venderse tres.
//!
//! Escribir doce sin más borraría esas ventas del inventario. Un conteo es una
//! foto de un momento, así que se guarda la hora y al aplicarlo se le suman los
//! movimientos posteriores: doce a las tres con tres ventas después son nueve.
//!
//! Es lo que hace que contar sin conexión sea seguro. Sin esto, hacer inventario
//! por la mañana y sincronizar por la tarde desharía la venta del día.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Con esto empieza el motivo de todo ajuste que viene de un conteo. Es lo que
/// permite separarlos de los movimientos de mercancía, también en los conteos
/// que ya estaban en la base antes de este arreglo.
const RAZON_CONTEO: &str = "Conteo desde el celular";
const RAZON_CONTEO_COMO_PATRON: &str = "Conteo desde el celular%";

#[derive(Debug, Deserialize, Clone)]
pub struct ConteoDelCelular {
    /// Lo pone el teléfono al contar, no al mandar: reintentar no cuenta dos veces.
    pub conteo_id: String,
    pub sku: String,
    #[serde(default)]
    pub variant_id: Option<i64>,
    pub contado: i32,
    /// Cuándo se contó, en hora local de la tienda ("YYYY-MM-DD HH:MM:SS").
    pub contado_en: String,
}

#[derive(Debug, Serialize)]
pub struct ResultadoConteo {
    pub sku: String,
    pub etiqueta: String,
    /// Lo que había registrado antes de aplicar el conteo.
    pub antes: i32,
    /// Lo que quedó.
    pub despues: i32,
    /// Cuánto se movió entre el conteo y su llegada. Cero cuando nada pasó.
    pub movido_mientras: i32,
}

/// Desfase de reloj a partir del cual se corrige la hora del conteo.
///
/// Por debajo, la diferencia la explica el viaje: el teléfono manda, el relevo lo
/// guarda y la caja pasa a recogerlo hasta tres minutos después.
const DESFASE_TOLERADO_SEGUNDOS: i64 = 10 * 60;

const FORMATO: &str = "%Y-%m-%d %H:%M:%S";

/// Corrige la hora del conteo cuando el reloj del teléfono está desfasado.
///
/// La caja le suma al conteo lo que se movió después de la hora en que se contó,
/// y esa hora la pone el teléfono. Con el reloj adelantado no veía ningún
/// movimiento posterior y escribía el conteo tal cual, ignorando lo vendido
/// entre el conteo y su llegada; con el reloj atrasado contaba ventas anteriores
/// al conteo. El teléfono manda también su reloj al enviar, así que el desfase se
/// mide y se descuenta.
///
/// Devuelve el desfase corregido en segundos, si hubo que corregir.
pub fn corregir_por_reloj(
    entrada: &mut ConteoDelCelular,
    reloj_del_telefono: Option<&str>,
    ahora: chrono::NaiveDateTime,
) -> Option<i64> {
    let reloj = chrono::NaiveDateTime::parse_from_str(reloj_del_telefono?.trim(), FORMATO).ok()?;
    let contado = chrono::NaiveDateTime::parse_from_str(entrada.contado_en.trim(), FORMATO).ok()?;

    let desfase = (reloj - ahora).num_seconds();
    if desfase.abs() <= DESFASE_TOLERADO_SEGUNDOS {
        return None;
    }
    entrada.contado_en = (contado - chrono::Duration::seconds(desfase)).format(FORMATO).to_string();
    Some(desfase)
}

/// Aplica un conteo, respetando lo que se vendió mientras tanto.
pub fn aplicar_conteo(
    db: &Connection,
    user_id: Option<i64>,
    entrada: &ConteoDelCelular,
) -> Result<ResultadoConteo, String> {
    if entrada.contado < 0 {
        return Err("Lo contado no puede ser negativo".to_string());
    }
    if entrada.conteo_id.trim().is_empty() {
        return Err("El conteo llegó sin identificador".to_string());
    }

    // Ya llegó antes: se devuelve lo que se resolvió entonces, sin volver a
    // tocar el inventario.
    if let Ok((sku, etiqueta, antes, despues, movido)) = db.query_row(
        "SELECT p.sku, COALESCE(v.size || ' ' || COALESCE(v.color, ''), 'Único'),
                c.stock_antes, c.stock_despues, c.ajuste_por_movimientos
         FROM conteos c
         JOIN products p ON c.product_id = p.id
         LEFT JOIN product_variants v ON c.variant_id = v.id
         WHERE c.conteo_id = ?1",
        params![entrada.conteo_id.trim()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    ) {
        return Ok(ResultadoConteo { sku, etiqueta, antes, despues, movido_mientras: movido });
    }

    let (product_id, nombre, activo, con_tallas): (i64, String, bool, bool) = db
        .query_row(
            "SELECT id, name, is_active, has_variants = 1 FROM products WHERE sku = ?1",
            params![entrada.sku.trim()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|_| format!("El producto {} ya no existe", entrada.sku))?;
    // El catálogo del teléfono puede tener días: la prenda pudo darse de baja
    // mientras tanto, y contarla le movía la existencia a algo que ya no se vende.
    if !activo {
        return Err(format!("'{}' está dado de baja; su conteo no se aplicó", nombre));
    }
    // El total de un producto con tallas es la suma de sus tallas: escribirlo
    // directo rompe esa cuenta y nada lo delata hasta el siguiente conteo por
    // talla. Pasa cuando el catálogo del celular es más viejo que las tallas.
    if con_tallas && entrada.variant_id.is_none() {
        return Err(format!(
            "'{}' ahora se cuenta por tallas; vuelve a escanear el código de la computadora para traer el catálogo al día",
            nombre
        ));
    }

    let (etiqueta, stock_antes) = match entrada.variant_id {
        Some(vid) => {
            let (size, color, stock): (Option<String>, Option<String>, i32) = db
                .query_row(
                    "SELECT size, color, stock FROM product_variants
                     WHERE id = ?1 AND product_id = ?2 AND is_active = 1",
                    params![vid, product_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .map_err(|_| "Esa talla ya no existe".to_string())?;
            let etiqueta = [size, color]
                .into_iter()
                .flatten()
                .filter(|v| !v.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" / ");
            (if etiqueta.is_empty() { nombre.clone() } else { format!("{} ({})", nombre, etiqueta) }, stock)
        }
        None => {
            let stock: i32 = db
                .query_row("SELECT stock FROM products WHERE id = ?1", params![product_id], |r| r.get(0))
                .map_err(|e| e.to_string())?;
            (nombre.clone(), stock)
        }
    };

    // Lo que se movió desde que se contó. Las ventas son negativas, las
    // devoluciones positivas: sumarlas al conteo lo trae al presente.
    //
    // Cuando se contó una talla, solo cuentan los movimientos de esa talla. Sin
    // esa distinción, anotar "talla M: 5" y vender después tres piezas de la
    // talla G dejaba la M en 2: el conteo absorbía ventas de mercancía que nadie
    // había contado.
    //
    // Los ajustes de otros conteos no son movimientos de mercancía: se anotan al
    // llegar, no al contar, así que un recuento que llega junto con el primer
    // conteo los veía "después" y se los restaba. Recontar dejaba la talla en
    // cero.
    let movido: i32 = match entrada.variant_id {
        Some(vid) => db
            .query_row(
                "SELECT COALESCE(SUM(quantity), 0) FROM inventory_movements
                 WHERE product_id = ?1 AND variant_id = ?2 AND created_at > ?3
                   AND NOT (movement_type = 'adjustment' AND reason LIKE ?4)",
                params![product_id, vid, entrada.contado_en, RAZON_CONTEO_COMO_PATRON],
                |r| r.get(0),
            )
            .unwrap_or(0),
        // Sin talla se contó la prenda entera, así que todo su movimiento cuenta.
        None => db
            .query_row(
                "SELECT COALESCE(SUM(quantity), 0) FROM inventory_movements
                 WHERE product_id = ?1 AND created_at > ?2
                   AND NOT (movement_type = 'adjustment' AND reason LIKE ?3)",
                params![product_id, entrada.contado_en, RAZON_CONTEO_COMO_PATRON],
                |r| r.get(0),
            )
            .unwrap_or(0),
    };

    // El relevo no entrega en el orden en que se contó. Si esta misma prenda ya
    // tiene un conteo hecho más tarde, ese es la verdad más reciente: este llega
    // viejo y se anota sin tocar el inventario.
    let hay_uno_mas_nuevo: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM conteos
                           WHERE product_id = ?1 AND variant_id IS ?2 AND contado_en > ?3)",
            params![product_id, entrada.variant_id, entrada.contado_en],
            |r| r.get(0),
        )
        .unwrap_or(false);

    let (stock_despues, movido) = if hay_uno_mas_nuevo {
        (stock_antes, 0)
    } else {
        ((entrada.contado + movido).max(0), movido)
    };

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let resultado = (|| -> Result<(), String> {
        match entrada.variant_id {
            Some(vid) => {
                db.execute(
                    "UPDATE product_variants SET stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                    params![stock_despues, vid],
                ).map_err(|e| e.to_string())?;
                // El total del producto es la suma de sus tallas.
                db.execute(
                    "UPDATE products SET stock = (
                         SELECT COALESCE(SUM(stock), 0) FROM product_variants
                         WHERE product_id = ?1 AND is_active = 1
                     ), updated_at = datetime('now','localtime') WHERE id = ?1",
                    params![product_id],
                ).map_err(|e| e.to_string())?;
            }
            None => {
                db.execute(
                    "UPDATE products SET stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                    params![stock_despues, product_id],
                ).map_err(|e| e.to_string())?;
            }
        }

        if !hay_uno_mas_nuevo {
            let razon = if movido == 0 {
                RAZON_CONTEO.to_string()
            } else {
                format!("{} (se movieron {} mientras tanto)", RAZON_CONTEO, movido)
            };
            db.execute(
                "INSERT INTO inventory_movements (product_id, variant_id, movement_type, quantity, previous_stock, new_stock, reason, user_id)
                 VALUES (?1, ?2, 'adjustment', ?3, ?4, ?5, ?6, ?7)",
                params![product_id, entrada.variant_id, stock_despues - stock_antes, stock_antes, stock_despues, razon, user_id],
            ).map_err(|e| e.to_string())?;
        }

        db.execute(
            "INSERT INTO conteos (conteo_id, product_id, variant_id, contado, contado_en,
                                  stock_antes, stock_despues, ajuste_por_movimientos, user_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entrada.conteo_id.trim(), product_id, entrada.variant_id, entrada.contado,
                entrada.contado_en, stock_antes, stock_despues, movido, user_id
            ],
        ).map_err(|e| e.to_string())?;
        Ok(())
    })();

    if let Err(e) = resultado {
        db.execute_batch("ROLLBACK;").ok();
        return Err(e);
    }
    crate::db::connection::confirmar(db)?;

    Ok(ResultadoConteo {
        sku: entrada.sku.trim().to_string(),
        etiqueta,
        antes: stock_antes,
        despues: stock_despues,
        movido_mientras: movido,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tienda() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')",
            [],
        ).unwrap();
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (1, 'TS-000001', 'Vestido', 200.0, 499.0, 20)",
            [],
        ).unwrap();
        db
    }

    fn conteo(contado: i32, cuando: &str) -> ConteoDelCelular {
        ConteoDelCelular {
            conteo_id: uuid::Uuid::new_v4().to_string(),
            sku: "TS-000001".into(),
            variant_id: None,
            contado,
            contado_en: cuando.into(),
        }
    }

    fn stock(db: &Connection) -> i32 {
        db.query_row("SELECT stock FROM products WHERE id = 1", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn sin_nada_de_por_medio_el_conteo_manda() {
        let db = tienda();
        let r = aplicar_conteo(&db, Some(1), &conteo(12, "2026-01-01 15:00:00")).unwrap();
        assert_eq!(stock(&db), 12);
        assert_eq!(r.antes, 20);
        assert_eq!(r.movido_mientras, 0);
    }

    #[test]
    fn lo_vendido_despues_del_conteo_no_se_borra() {
        // El caso que hace peligroso contar sin conexión: se cuenta a las tres,
        // se vende, y el conteo llega a las ocho. Escribir doce a secas
        // desharía la venta.
        let db = tienda();
        db.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, created_at)
             VALUES (1, 'sale', -3, 20, 17, '2026-01-01 16:30:00')", [],
        ).unwrap();

        let r = aplicar_conteo(&db, Some(1), &conteo(12, "2026-01-01 15:00:00")).unwrap();

        assert_eq!(r.movido_mientras, -3);
        assert_eq!(stock(&db), 9, "doce contados menos tres vendidos después");
    }

    #[test]
    fn lo_devuelto_despues_del_conteo_tambien_cuenta() {
        let db = tienda();
        db.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, created_at)
             VALUES (1, 'return', 2, 20, 22, '2026-01-01 17:00:00')", [],
        ).unwrap();

        assert_eq!(aplicar_conteo(&db, Some(1), &conteo(12, "2026-01-01 15:00:00")).unwrap().despues, 14);
    }

    #[test]
    fn lo_que_paso_antes_del_conteo_ya_estaba_contado() {
        // Quien contó vio la mercancía con esas ventas ya hechas.
        let db = tienda();
        db.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, created_at)
             VALUES (1, 'sale', -5, 25, 20, '2026-01-01 10:00:00')", [],
        ).unwrap();

        assert_eq!(aplicar_conteo(&db, Some(1), &conteo(12, "2026-01-01 15:00:00")).unwrap().despues, 12);
    }

    #[test]
    fn mandar_el_mismo_conteo_dos_veces_no_lo_aplica_dos_veces() {
        let db = tienda();
        let c = conteo(12, "2026-01-01 15:00:00");

        let primero = aplicar_conteo(&db, Some(1), &c).unwrap();
        let segundo = aplicar_conteo(&db, Some(1), &c).unwrap();

        assert_eq!(primero.despues, segundo.despues);
        assert_eq!(stock(&db), 12);
        let cuantos: i64 = db.query_row("SELECT COUNT(*) FROM conteos", [], |r| r.get(0)).unwrap();
        assert_eq!(cuantos, 1);
    }

    #[test]
    fn el_inventario_nunca_queda_negativo() {
        // Se contaron dos y después se vendieron cinco: la cuenta da -3, que no
        // existe. Queda en cero y el movimiento lo deja anotado para revisarlo.
        let db = tienda();
        db.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, created_at)
             VALUES (1, 'sale', -5, 20, 15, '2026-01-01 16:00:00')", [],
        ).unwrap();

        assert_eq!(aplicar_conteo(&db, Some(1), &conteo(2, "2026-01-01 15:00:00")).unwrap().despues, 0);
    }

    #[test]
    fn contar_una_talla_actualiza_el_total_del_producto() {
        let db = tienda();
        db.execute(
            "INSERT INTO product_variants (id, product_id, size, stock) VALUES (1, 1, 'M', 8), (2, 1, 'G', 12)",
            [],
        ).unwrap();
        db.execute("UPDATE products SET has_variants = 1, stock = 20 WHERE id = 1", []).unwrap();

        let mut c = conteo(5, "2026-01-01 15:00:00");
        c.variant_id = Some(1);
        aplicar_conteo(&db, Some(1), &c).unwrap();

        let m: i32 = db.query_row("SELECT stock FROM product_variants WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(m, 5);
        assert_eq!(stock(&db), 17, "el total es la suma de las tallas");
    }

    #[test]
    fn un_conteo_sin_talla_no_pisa_el_total_de_un_producto_con_tallas() {
        // El catálogo del celular puede tener semanas: una prenda que ganó tallas
        // después sigue apareciendo sin ellas. El conteo caía sobre el total del
        // producto, que es la suma de sus tallas, y esa suma dejaba de cuadrar sin
        // que nada lo dijera. `ajustar_stock` sí se niega; esto no lo hacía.
        let db = tienda();
        con_tallas(&db);

        let e = aplicar_conteo(&db, Some(1), &conteo(5, "2026-01-01 15:00:00")).unwrap_err();

        assert!(e.contains("tallas"), "mensaje inesperado: {}", e);
        assert_eq!(stock(&db), 20, "el total no se toca");
        assert_eq!(stock_de(&db, 1), 8);
        assert_eq!(stock_de(&db, 2), 12);
        let conteos: i64 = db.query_row("SELECT COUNT(*) FROM conteos", [], |r| r.get(0)).unwrap();
        assert_eq!(conteos, 0, "no queda anotado como aplicado");
    }

    /// Deja el producto con dos tallas y devuelve sus ids.
    fn con_tallas(db: &Connection) -> (i64, i64) {
        db.execute(
            "INSERT INTO product_variants (id, product_id, size, stock) VALUES (1, 1, 'M', 8), (2, 1, 'G', 12)",
            [],
        ).unwrap();
        db.execute("UPDATE products SET has_variants = 1, stock = 20 WHERE id = 1", []).unwrap();
        (1, 2)
    }

    fn movimiento(db: &Connection, variant_id: Option<i64>, cantidad: i32, cuando: &str) {
        db.execute(
            "INSERT INTO inventory_movements (product_id, variant_id, movement_type, quantity, previous_stock, new_stock, created_at)
             VALUES (1, ?1, 'sale', ?2, 20, 20, ?3)",
            params![variant_id, cantidad, cuando],
        ).unwrap();
    }

    fn stock_de(db: &Connection, variant_id: i64) -> i32 {
        db.query_row("SELECT stock FROM product_variants WHERE id = ?1", params![variant_id], |r| r.get(0)).unwrap()
    }

    #[test]
    fn contar_una_talla_no_absorbe_lo_vendido_de_otra() {
        // El caso que corrompía el inventario: se anotaba "talla M: 5", se
        // vendían tres piezas de la talla G, y al llegar el conteo la M quedaba
        // en 2. El conteo se comía ventas de mercancía que nadie había contado.
        let db = tienda();
        let (m, _g) = con_tallas(&db);
        movimiento(&db, Some(2), -3, "2026-01-01 16:30:00");

        let mut c = conteo(5, "2026-01-01 15:00:00");
        c.variant_id = Some(m);
        let r = aplicar_conteo(&db, Some(1), &c).unwrap();

        assert_eq!(r.movido_mientras, 0, "lo de la talla G no es asunto de la M");
        assert_eq!(stock_de(&db, m), 5, "la talla M queda en lo que se contó");
        assert_eq!(stock_de(&db, 2), 12, "la talla G no se toca");
    }

    #[test]
    fn contar_una_talla_si_respeta_lo_vendido_de_ella_misma() {
        // La otra mitad: lo que sí se vendió de esa talla tiene que descontarse,
        // o el conteo desharía la venta.
        let db = tienda();
        let (m, _) = con_tallas(&db);
        movimiento(&db, Some(m), -3, "2026-01-01 16:30:00");

        let mut c = conteo(5, "2026-01-01 15:00:00");
        c.variant_id = Some(m);
        let r = aplicar_conteo(&db, Some(1), &c).unwrap();

        assert_eq!(r.movido_mientras, -3);
        assert_eq!(stock_de(&db, m), 2, "cinco contados menos tres vendidos después");
    }

    #[test]
    fn una_venta_de_verdad_queda_atada_a_su_talla() {
        // Sin pasar por SQL a mano: se vende la talla G por el camino normal y se
        // comprueba que el conteo de la M no se enteró.
        let db = tienda();
        let (m, g) = con_tallas(&db);
        db.execute("INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, 0.0)", []).unwrap();
        crate::commands::sales::registrar_venta(
            &db,
            1,
            None,
            crate::models::sale::CreateSaleDto {
                items: vec![crate::models::sale::CreateSaleItemDto {
                    product_id: 1, quantity: 3, unit_price: 499.0, discount: 0.0, variant_id: Some(g),
                }],
                payment_method: "cash".into(),
                amount_paid: 2000.0,
                payments: vec![],
                discount_total: 0.0,
                promotion_id: None,
                notes: None,
                customer_id: None,
                client_request_id: None,
                requiere_factura: false,
            },
        ).unwrap();

        // Se contó la M *antes* de esa venta.
        let mut c = conteo(5, "2000-01-01 00:00:00");
        c.variant_id = Some(m);
        let r = aplicar_conteo(&db, Some(1), &c).unwrap();

        assert_eq!(r.movido_mientras, 0);
        assert_eq!(stock_de(&db, m), 5);
        assert_eq!(stock_de(&db, g), 9, "la venta de la G sigue descontada");
    }

    #[test]
    fn el_conteo_deja_anotada_la_talla_que_se_contó() {
        // Si el propio movimiento del conteo no llevara la talla, el siguiente
        // conteo de esa talla no vería este ajuste.
        let db = tienda();
        let (m, _) = con_tallas(&db);
        let mut c = conteo(5, "2026-01-01 15:00:00");
        c.variant_id = Some(m);
        aplicar_conteo(&db, Some(1), &c).unwrap();

        let anotada: Option<i64> = db.query_row(
            "SELECT variant_id FROM inventory_movements WHERE reason LIKE 'Conteo%'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(anotada, Some(m));
    }

    #[test]
    fn se_rechaza_lo_que_no_tiene_sentido() {
        let db = tienda();
        assert!(aplicar_conteo(&db, Some(1), &conteo(-4, "2026-01-01 15:00:00")).is_err());

        let mut sin_id = conteo(4, "2026-01-01 15:00:00");
        sin_id.conteo_id = "  ".into();
        assert!(aplicar_conteo(&db, Some(1), &sin_id).is_err());

        let mut fantasma = conteo(4, "2026-01-01 15:00:00");
        fantasma.sku = "NO-EXISTE".into();
        assert!(aplicar_conteo(&db, Some(1), &fantasma).is_err());
    }

    #[test]
    fn recontar_no_le_resta_al_segundo_conteo_el_ajuste_del_primero() {
        // Se cuenta 5, se nota el error y se recuenta 6. Los dos llegan juntos:
        // el ajuste del primero se anota al llegar, "después" del segundo, y
        // sumarlo como si fuera una venta dejaba 3.
        let db = tienda();
        aplicar_conteo(&db, Some(1), &conteo(5, "2026-01-01 15:00:00")).unwrap();
        aplicar_conteo(&db, Some(1), &conteo(6, "2026-01-01 15:01:00")).unwrap();
        assert_eq!(stock(&db), 6);
    }

    #[test]
    fn un_conteo_viejo_que_llega_tarde_no_pisa_a_uno_mas_nuevo() {
        // El relevo no entrega en orden: el recuento puede aplicarse primero.
        let db = tienda();
        aplicar_conteo(&db, Some(1), &conteo(6, "2026-01-01 15:01:00")).unwrap();
        let r = aplicar_conteo(&db, Some(1), &conteo(5, "2026-01-01 15:00:00")).unwrap();
        assert_eq!(stock(&db), 6);
        assert_eq!(r.antes, r.despues);
    }

    #[test]
    fn un_conteo_viejo_no_pisa_lo_vendido_despues_del_nuevo() {
        let db = tienda();
        aplicar_conteo(&db, Some(1), &conteo(6, "2026-01-01 15:01:00")).unwrap();
        db.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, created_at)
             VALUES (1, 'sale', -1, 6, 5, '2099-01-01 10:00:00')", [],
        ).unwrap();
        db.execute("UPDATE products SET stock = 5 WHERE id = 1", []).unwrap();
        aplicar_conteo(&db, Some(1), &conteo(9, "2026-01-01 15:00:00")).unwrap();
        assert_eq!(stock(&db), 5);
    }
    fn en(texto: &str) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str(texto, FORMATO).unwrap()
    }

    #[test]
    fn un_reloj_adelantado_corre_la_hora_del_conteo_hacia_atras() {
        // El teléfono cree que es un día después. Sin corregir, la caja no veía
        // ninguna venta "posterior" al conteo y lo escribía tal cual.
        let mut c = conteo(12, "2026-01-02 11:00:00");
        let desfase = corregir_por_reloj(&mut c, Some("2026-01-02 12:00:00"), en("2026-01-01 12:00:00"));

        assert_eq!(desfase, Some(86_400));
        assert_eq!(c.contado_en, "2026-01-01 11:00:00");
    }

    #[test]
    fn un_reloj_atrasado_la_corre_hacia_adelante() {
        let mut c = conteo(12, "2026-01-01 08:00:00");
        let desfase = corregir_por_reloj(&mut c, Some("2026-01-01 09:00:00"), en("2026-01-01 12:00:00"));

        assert_eq!(desfase, Some(-10_800));
        assert_eq!(c.contado_en, "2026-01-01 11:00:00");
    }

    #[test]
    fn una_diferencia_de_minutos_es_el_viaje_y_no_se_toca() {
        // El teléfono manda, el relevo guarda y la caja recoge hasta tres minutos
        // después: esa diferencia es normal.
        let mut c = conteo(12, "2026-01-01 11:00:00");
        assert_eq!(corregir_por_reloj(&mut c, Some("2026-01-01 11:57:00"), en("2026-01-01 12:00:00")), None);
        assert_eq!(c.contado_en, "2026-01-01 11:00:00");
    }

    #[test]
    fn sin_reloj_o_con_uno_ilegible_se_queda_como_vino() {
        let mut c = conteo(12, "2026-01-01 11:00:00");
        assert_eq!(corregir_por_reloj(&mut c, None, en("2026-01-01 12:00:00")), None);
        assert_eq!(corregir_por_reloj(&mut c, Some("ayer por la tarde"), en("2026-01-01 12:00:00")), None);
        assert_eq!(c.contado_en, "2026-01-01 11:00:00");
    }

    #[test]
    fn con_el_reloj_adelantado_lo_vendido_despues_del_conteo_si_se_resta() {
        // La prueba que importa: el conteo se corrige y entonces la venta que
        // ocurrió entre contar y llegar vuelve a quedar "después".
        let db = tienda();
        let ahora = chrono::Local::now().naive_local();
        let hace_media_hora = (ahora - chrono::Duration::minutes(30)).format(FORMATO).to_string();
        db.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, created_at)
             VALUES (1, 'sale', -3, 20, 17, ?1)",
            params![hace_media_hora],
        ).unwrap();

        // Contó hace una hora, pero su reloj va un día adelantado.
        let reloj = (ahora + chrono::Duration::days(1)).format(FORMATO).to_string();
        let contado_en = (ahora + chrono::Duration::days(1) - chrono::Duration::hours(1)).format(FORMATO).to_string();
        let mut c = conteo(12, &contado_en);
        corregir_por_reloj(&mut c, Some(&reloj), ahora);

        let r = aplicar_conteo(&db, Some(1), &c).unwrap();

        assert_eq!(r.movido_mientras, -3, "la venta posterior al conteo cuenta");
        assert_eq!(stock(&db), 9);
    }

    #[test]
    fn no_se_cuenta_un_producto_dado_de_baja() {
        let db = tienda();
        db.execute("UPDATE products SET is_active = 0 WHERE id = 1", []).unwrap();

        let err = aplicar_conteo(&db, Some(1), &conteo(5, "2026-01-01 15:00:00")).unwrap_err();

        assert!(err.contains("dado de baja"), "{}", err);
        assert_eq!(stock(&db), 20, "la existencia no se movió");
        let conteos: i64 = db.query_row("SELECT COUNT(*) FROM conteos", [], |r| r.get(0)).unwrap();
        assert_eq!(conteos, 0);
    }

}
