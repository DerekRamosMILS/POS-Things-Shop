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

    let (product_id, nombre): (i64, String) = db
        .query_row(
            "SELECT id, name FROM products WHERE sku = ?1",
            params![entrada.sku.trim()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| format!("El producto {} ya no existe", entrada.sku))?;

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
    let movido: i32 = db
        .query_row(
            "SELECT COALESCE(SUM(quantity), 0) FROM inventory_movements
             WHERE product_id = ?1 AND created_at > ?2",
            params![product_id, entrada.contado_en],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let stock_despues = (entrada.contado + movido).max(0);

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

        let razon = if movido == 0 {
            "Conteo desde el celular".to_string()
        } else {
            format!("Conteo desde el celular (se movieron {} mientras tanto)", movido)
        };
        db.execute(
            "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reason, user_id)
             VALUES (?1, 'adjustment', ?2, ?3, ?4, ?5, ?6)",
            params![product_id, stock_despues - stock_antes, stock_antes, stock_despues, razon, user_id],
        ).map_err(|e| e.to_string())?;

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
    db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;

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
}
