use std::collections::HashMap;

use rusqlite::params;
use serde::Deserialize;
use tauri::State;

use crate::commands::cash_register::open_register_id;
use crate::db::connection::DbState;
use crate::money::Cents;
use crate::session::{require_admin, SessionState};

#[derive(Debug, Deserialize)]
pub struct ReturnItemDto {
    pub sale_item_id: i64,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateReturnDto {
    pub sale_id: i64,
    pub reason: Option<String>,
    /// Cómo se le regresó el dinero al cliente: "cash", "card" o "transfer".
    /// Solo el efectivo sale del cajón.
    #[serde(default = "default_refund_method")]
    pub refund_method: String,
    pub items: Vec<ReturnItemDto>,
}

fn default_refund_method() -> String {
    "cash".to_string()
}

/// Register a (partial or full) return: restocks inventory, records the movement,
/// tracks returned_quantity per line, and marks the sale 'returned' when fully returned.
///
/// A cash refund physically empties the drawer, so it is subtracted from the open
/// shift; card and transfer refunds are recorded but leave the drawer untouched.
#[tauri::command]
pub fn create_return(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreateReturnDto) -> Result<f64, String> {
    let user_id = require_admin(&sessions, &token)?;
    let db = state.conn();
    registrar_devolucion(&db, user_id, data)
}

/// Núcleo de la devolución, con la conexión explícita para poder probarlo.
pub fn registrar_devolucion(
    db: &rusqlite::Connection,
    user_id: i64,
    data: CreateReturnDto,
) -> Result<f64, String> {
    if data.items.is_empty() {
        return Err("Selecciona artículos a devolver".to_string());
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<f64, String> {
        let sale_status: String = db.query_row(
            "SELECT status FROM sales WHERE id = ?1",
            params![data.sale_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        if sale_status == "cancelled" {
            return Err("La venta está cancelada".to_string());
        }

        // Lo que se devuelve es lo que el cliente pagó por esas piezas, no lo
        // que dice la lista de precios: una venta puede llevar una promoción a
        // nivel de ticket y un impuesto encima, y ninguno de los dos aparece en
        // el renglón. Se reparte el total cobrado en proporción al peso de cada
        // partida.
        let (venta_total, suma_netos): (f64, f64) = db.query_row(
            "SELECT s.total,
                    COALESCE((SELECT SUM(subtotal) FROM sale_items WHERE sale_id = s.id), 0)
             FROM sales s WHERE s.id = ?1",
            params![data.sale_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).map_err(|e| e.to_string())?;

        let venta_total = Cents::from_pesos(venta_total);
        let suma_netos = Cents::from_pesos(suma_netos);

        struct Ri { sale_item_id: i64, product_id: i64, variant_id: Option<i64>, quantity: i32, refund: Cents }
        let mut ris: Vec<Ri> = Vec::new();
        let mut total_refund = Cents::ZERO;
        // Lo que las partidas anteriores de esta misma devolución ya tomaron.
        // El renglón se marca como devuelto hasta el segundo recorrido, así que
        // sin esto la misma partida repetida se mide dos veces contra lo mismo
        // y se devuelven más piezas —y más dinero— de las que se vendieron.
        let mut ya_tomado: HashMap<i64, i32> = HashMap::new();

        for it in &data.items {
            if it.quantity <= 0 {
                continue;
            }
            let (product_id, sold_qty, returned_qty, line_net, variant_id): (i64, i32, i32, f64, Option<i64>) =
                db.query_row(
                    "SELECT product_id, quantity, returned_quantity, subtotal, variant_id FROM sale_items WHERE id = ?1 AND sale_id = ?2",
                    params![it.sale_item_id, data.sale_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                ).map_err(|_| "Esa partida no pertenece a la venta".to_string())?;

            let available =
                sold_qty - returned_qty - ya_tomado.get(&it.sale_item_id).copied().unwrap_or(0);
            if it.quantity > available {
                return Err(format!("Solo puedes devolver hasta {} unidad(es) de esa línea", available.max(0)));
            }
            if sold_qty <= 0 {
                continue;
            }
            *ya_tomado.entry(it.sale_item_id).or_insert(0) += it.quantity;

            // Parte del renglón que corresponde a las piezas devueltas...
            let neto_devuelto = Cents::from_pesos(line_net)
                .prorate(Cents(it.quantity as i64), Cents(sold_qty as i64));
            // ...llevada a lo que realmente se cobró por el ticket.
            let refund = venta_total.prorate(neto_devuelto, suma_netos);

            total_refund = total_refund + refund;
            ris.push(Ri { sale_item_id: it.sale_item_id, product_id, variant_id, quantity: it.quantity, refund });
        }

        if ris.is_empty() {
            return Err("Nada que devolver".to_string());
        }

        let register_id = open_register_id(db);

        db.execute(
            "INSERT INTO returns (sale_id, user_id, total_refund, reason, refund_method, cash_register_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![data.sale_id, user_id, total_refund.to_pesos(), data.reason, data.refund_method, register_id],
        ).map_err(|e| e.to_string())?;
        let return_id = db.last_insert_rowid();

        if data.refund_method == "cash" && total_refund.is_positive() {
            match register_id {
                Some(cr_id) => {
                    db.execute(
                        "UPDATE cash_registers SET total_refunds_cash = total_refunds_cash + ?1 WHERE id = ?2",
                        params![total_refund.to_pesos(), cr_id],
                    ).map_err(|e| e.to_string())?;
                }
                // Sin turno abierto no hay de dónde sacar el efectivo sin
                // descuadrar el siguiente corte.
                None => return Err(
                    "Abre la caja antes de devolver en efectivo.".to_string()
                ),
            }
        }

        for ri in &ris {
            db.execute(
                "INSERT INTO return_items (return_id, sale_item_id, product_id, quantity, refund_amount) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![return_id, ri.sale_item_id, ri.product_id, ri.quantity, ri.refund.to_pesos()],
            ).map_err(|e| e.to_string())?;

            db.execute(
                "UPDATE sale_items SET returned_quantity = returned_quantity + ?1 WHERE id = ?2",
                params![ri.quantity, ri.sale_item_id],
            ).map_err(|e| e.to_string())?;

            let current_stock: i32 = db.query_row(
                "SELECT stock FROM products WHERE id = ?1",
                params![ri.product_id],
                |row| row.get(0),
            ).map_err(|e| e.to_string())?;
            let new_stock = current_stock + ri.quantity;

            db.execute(
                "UPDATE products SET stock = ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                params![new_stock, ri.product_id],
            ).map_err(|e| e.to_string())?;

            if let Some(vid) = ri.variant_id {
                db.execute(
                    "UPDATE product_variants SET stock = stock + ?1, updated_at = datetime('now','localtime') WHERE id = ?2",
                    params![ri.quantity, vid],
                ).map_err(|e| e.to_string())?;
            }

            db.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reference_id, reason, user_id)
                 VALUES (?1, 'return', ?2, ?3, ?4, ?5, 'Devolución de venta', ?6)",
                params![ri.product_id, ri.quantity, current_stock, new_stock, data.sale_id, user_id],
            ).map_err(|e| e.to_string())?;
        }

        // Mark the sale as fully returned when nothing remains.
        let remaining: i32 = db.query_row(
            "SELECT COALESCE(SUM(quantity - returned_quantity), 0) FROM sale_items WHERE sale_id = ?1",
            params![data.sale_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        if remaining <= 0 {
            db.execute(
                "UPDATE sales SET status = 'returned' WHERE id = ?1",
                params![data.sale_id],
            ).map_err(|e| e.to_string())?;
        }

        db.execute(
            "INSERT INTO app_logs (level, module, message, user_id) VALUES ('warn', 'returns', ?1, ?2)",
            params![format!("Devolución de venta {} por {}", data.sale_id, total_refund), user_id],
        ).ok();

        Ok(total_refund.to_pesos())
    })();

    match result {
        Ok(total) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            Ok(total)
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::cash_register::expected_cash;
    use crate::commands::sales::registrar_venta;
    use crate::models::sale::{CreateSaleDto, CreateSaleItemDto, PaymentSplitDto};

    struct Tienda {
        db: rusqlite::Connection,
    }

    impl Tienda {
        fn nueva() -> Tienda {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
            crate::db::migrations::run_migrations(&db).unwrap();
            db.execute(
                "INSERT INTO users (id, username, password_hash, full_name, role)
                 VALUES (1, 'u', 'x', 'U', 'admin')",
                [],
            ).unwrap();
            Tienda { db }
        }

        fn con_caja(self) -> Tienda {
            self.db.execute(
                "INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, 1000.0)", [],
            ).unwrap();
            self
        }

        fn producto(&self, sku: &str, precio: f64, stock: i32) -> i64 {
            self.db.execute(
                "INSERT INTO products (sku, name, purchase_price, sale_price, stock)
                 VALUES (?1, ?1, 10.0, ?2, ?3)",
                params![sku, precio, stock],
            ).unwrap();
            self.db.last_insert_rowid()
        }

        fn vender(&self, items: Vec<(i64, i32, f64)>, promocion: Option<i64>) -> i64 {
            registrar_venta(&self.db, 1, None, CreateSaleDto {
                items: items.into_iter().map(|(product_id, quantity, discount)| CreateSaleItemDto {
                    product_id, quantity, unit_price: 0.0, discount, variant_id: None,
                }).collect(),
                payment_method: "cash".to_string(),
                amount_paid: 100_000.0,
                payments: vec![],
                discount_total: 0.0,
                promotion_id: promocion,
                requiere_factura: false,
                notes: None,
                customer_id: None,
                client_request_id: None,
            }).unwrap().id
        }

        fn partidas(&self, sale_id: i64) -> Vec<i64> {
            self.db.prepare("SELECT id FROM sale_items WHERE sale_id = ?1 ORDER BY id").unwrap()
                .query_map(params![sale_id], |r| r.get(0)).unwrap()
                .collect::<Result<Vec<_>, _>>().unwrap()
        }

        fn devolver(&self, sale_id: i64, items: Vec<(i64, i32)>, metodo: &str) -> Result<f64, String> {
            registrar_devolucion(&self.db, 1, CreateReturnDto {
                sale_id,
                reason: None,
                refund_method: metodo.to_string(),
                items: items.into_iter().map(|(sale_item_id, quantity)| ReturnItemDto {
                    sale_item_id, quantity,
                }).collect(),
            })
        }

        fn total_venta(&self, sale_id: i64) -> f64 {
            self.db.query_row("SELECT total FROM sales WHERE id = ?1", params![sale_id], |r| r.get(0)).unwrap()
        }

        fn stock(&self, id: i64) -> i32 {
            self.db.query_row("SELECT stock FROM products WHERE id = ?1", params![id], |r| r.get(0)).unwrap()
        }

        fn caja(&self) -> crate::models::cash_register::CashRegister {
            self.db.query_row(
                "SELECT cr.id, cr.user_id, u.full_name, cr.opening_amount, cr.closing_amount,
                        cr.expected_amount, cr.difference, cr.total_sales, cr.total_cash_sales,
                        cr.total_card_sales, cr.total_transfer_sales, cr.total_layaway_cash,
                        cr.total_layaway_card, cr.total_layaway_transfer, cr.total_refunds_cash,
                        cr.total_expenses, cr.sale_count, cr.status, cr.opened_at, cr.closed_at
                 FROM cash_registers cr LEFT JOIN users u ON cr.user_id = u.id LIMIT 1",
                [],
                |row| Ok(crate::models::cash_register::CashRegister {
                    id: row.get(0)?, user_id: row.get(1)?, user_name: row.get(2)?,
                    opening_amount: row.get(3)?, closing_amount: row.get(4)?,
                    expected_amount: row.get(5)?, difference: row.get(6)?,
                    total_sales: row.get(7)?, total_cash_sales: row.get(8)?,
                    total_card_sales: row.get(9)?, total_transfer_sales: row.get(10)?,
                    total_layaway_cash: row.get(11)?, total_layaway_card: row.get(12)?,
                    total_layaway_transfer: row.get(13)?, total_refunds_cash: row.get(14)?,
                    total_expenses: row.get(15)?, sale_count: row.get(16)?,
                    status: row.get(17)?, opened_at: row.get(18)?, closed_at: row.get(19)?,
                }),
            ).unwrap()
        }
    }

    #[test]
    fn la_misma_partida_repetida_no_devuelve_de_mas() {
        // El renglón se marca como devuelto hasta el segundo recorrido, así que
        // la misma partida dos veces se medía dos veces contra lo mismo: salían
        // del cajón seis piezas de una venta de cinco.
        let t = Tienda::nueva().con_caja();
        let blusa = t.producto("BLU", 100.0, 10);
        let venta = t.vender(vec![(blusa, 5, 0.0)], None);
        let partida = t.partidas(venta)[0];

        let error = t.devolver(venta, vec![(partida, 3), (partida, 3)], "cash");

        assert!(error.is_err(), "seis de cinco no deberían devolverse");
        assert_eq!(t.stock(blusa), 5, "una devolución rechazada no toca el inventario");
        assert_eq!(t.caja().total_refunds_cash, 0.0, "ni el cajón");
    }

    #[test]
    fn dos_partidas_de_la_misma_venta_se_devuelven_juntas() {
        let t = Tienda::nueva().con_caja();
        let blusa = t.producto("BLU", 100.0, 10);
        let venta = t.vender(vec![(blusa, 5, 0.0)], None);
        let partida = t.partidas(venta)[0];

        let devuelto = t.devolver(venta, vec![(partida, 2), (partida, 3)], "cash").unwrap();

        assert_eq!(devuelto, t.total_venta(venta), "cinco de cinco es la venta entera");
        assert_eq!(t.stock(blusa), 10);
    }

    #[test]
    fn devolver_todo_regresa_exactamente_lo_cobrado() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 249.0, 10);
        let venta = t.vender(vec![(p, 2, 0.0)], None);
        let partida = t.partidas(venta)[0];

        let reembolso = t.devolver(venta, vec![(partida, 2)], "cash").unwrap();

        assert_eq!(reembolso, t.total_venta(venta), "se devuelve lo que se cobró");
        assert_eq!(t.stock(p), 10, "el stock vuelve a su lugar");
    }

    #[test]
    fn devolver_una_de_dos_unidades_regresa_la_mitad() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 249.0, 10);
        let venta = t.vender(vec![(p, 2, 0.0)], None);
        let partida = t.partidas(venta)[0];

        assert_eq!(t.devolver(venta, vec![(partida, 1)], "cash").unwrap(), 249.0);
        assert_eq!(t.stock(p), 9);
    }

    #[test]
    fn el_descuento_de_linea_se_reparte_entre_las_unidades() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 100.0, 10);
        // 2 piezas de 100 con 40 de descuento => el cliente pagó 80 por pieza.
        let venta = t.vender(vec![(p, 2, 40.0)], None);
        let partida = t.partidas(venta)[0];

        assert_eq!(t.devolver(venta, vec![(partida, 1)], "cash").unwrap(), 80.0);
    }

    #[test]
    fn una_venta_con_promocion_no_devuelve_de_mas() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 200.0, 10);
        t.db.execute(
            "INSERT INTO promotions (name, discount_type, discount_value, start_date, end_date, applies_to)
             VALUES ('mitad', 'percentage', 50, date('now','localtime','-1 day'), date('now','localtime','+1 day'), 'all')",
            [],
        ).unwrap();
        let promo = t.db.last_insert_rowid();

        let venta = t.vender(vec![(p, 1, 0.0)], Some(promo));
        assert_eq!(t.total_venta(venta), 100.0, "el cliente pagó la mitad");

        let partida = t.partidas(venta)[0];
        let reembolso = t.devolver(venta, vec![(partida, 1)], "cash").unwrap();

        assert_eq!(reembolso, 100.0, "no puede devolverse más de lo que se cobró");
    }

    #[test]
    fn el_impuesto_cobrado_tambien_se_devuelve() {
        let t = Tienda::nueva().con_caja();
        t.db.execute("UPDATE system_config SET value = '16' WHERE key = 'tax_rate'", []).unwrap();
        let p = t.producto("CAM", 100.0, 10);

        let venta = t.vender(vec![(p, 1, 0.0)], None);
        assert_eq!(t.total_venta(venta), 116.0);

        let partida = t.partidas(venta)[0];
        assert_eq!(
            t.devolver(venta, vec![(partida, 1)], "cash").unwrap(),
            116.0,
            "el cliente pagó el impuesto y hay que regresárselo",
        );
    }

    #[test]
    fn no_se_puede_devolver_mas_de_lo_vendido() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 249.0, 10);
        let venta = t.vender(vec![(p, 2, 0.0)], None);
        let partida = t.partidas(venta)[0];

        assert!(t.devolver(venta, vec![(partida, 3)], "cash").is_err());
    }

    #[test]
    fn dos_devoluciones_parciales_no_superan_lo_vendido() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 100.0, 10);
        let venta = t.vender(vec![(p, 2, 0.0)], None);
        let partida = t.partidas(venta)[0];

        t.devolver(venta, vec![(partida, 1)], "cash").unwrap();
        t.devolver(venta, vec![(partida, 1)], "cash").unwrap();

        assert!(t.devolver(venta, vec![(partida, 1)], "cash").is_err(), "ya no queda nada por devolver");
    }

    #[test]
    fn devolver_todo_marca_la_venta_como_devuelta() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 100.0, 10);
        let venta = t.vender(vec![(p, 1, 0.0)], None);
        let partida = t.partidas(venta)[0];

        t.devolver(venta, vec![(partida, 1)], "cash").unwrap();

        let estado: String = t.db.query_row(
            "SELECT status FROM sales WHERE id = ?1", params![venta], |r| r.get(0)).unwrap();
        assert_eq!(estado, "returned");
    }

    #[test]
    fn el_efectivo_devuelto_sale_del_cajon() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 100.0, 10);
        let venta = t.vender(vec![(p, 1, 0.0)], None);
        let esperado_antes = expected_cash(&t.caja());
        let partida = t.partidas(venta)[0];

        let reembolso = t.devolver(venta, vec![(partida, 1)], "cash").unwrap();

        assert_eq!(t.caja().total_refunds_cash, reembolso);
        assert_eq!(expected_cash(&t.caja()), esperado_antes - reembolso);
    }

    #[test]
    fn una_devolucion_con_tarjeta_no_toca_el_cajon() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 100.0, 10);
        let venta = t.vender(vec![(p, 1, 0.0)], None);
        let esperado_antes = expected_cash(&t.caja());
        let partida = t.partidas(venta)[0];

        t.devolver(venta, vec![(partida, 1)], "card").unwrap();

        assert_eq!(t.caja().total_refunds_cash, 0.0);
        assert_eq!(expected_cash(&t.caja()), esperado_antes);
    }

    #[test]
    fn sin_caja_abierta_no_se_devuelve_efectivo_ni_queda_rastro() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 100.0, 10);
        let venta = t.vender(vec![(p, 1, 0.0)], None);
        t.db.execute("UPDATE cash_registers SET status = 'closed'", []).unwrap();
        let partida = t.partidas(venta)[0];

        assert!(t.devolver(venta, vec![(partida, 1)], "cash").is_err());

        let devoluciones: i64 = t.db.query_row("SELECT COUNT(*) FROM returns", [], |r| r.get(0)).unwrap();
        assert_eq!(devoluciones, 0, "la transacción debe revertirse completa");
        assert_eq!(t.stock(p), 9, "el stock no debe restaurarse si la devolución falló");
    }

    #[test]
    fn no_se_devuelve_sobre_una_venta_cancelada() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 100.0, 10);
        let venta = t.vender(vec![(p, 1, 0.0)], None);
        let partida = t.partidas(venta)[0];
        t.db.execute("UPDATE sales SET status = 'cancelled' WHERE id = ?1", params![venta]).unwrap();

        assert!(t.devolver(venta, vec![(partida, 1)], "cash").is_err());
    }

    #[test]
    fn se_registra_el_movimiento_de_inventario_de_la_devolucion() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 100.0, 10);
        let venta = t.vender(vec![(p, 2, 0.0)], None);
        let partida = t.partidas(venta)[0];

        t.devolver(venta, vec![(partida, 2)], "cash").unwrap();

        let (tipo, cantidad): (String, i32) = t.db.query_row(
            "SELECT movement_type, quantity FROM inventory_movements
             WHERE movement_type = 'return' AND reference_id = ?1",
            params![venta], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(tipo, "return");
        assert_eq!(cantidad, 2);
    }

    #[test]
    fn devolver_de_una_venta_mixta_regresa_lo_cobrado() {
        let t = Tienda::nueva().con_caja();
        let p = t.producto("CAM", 100.0, 10);
        let venta_id = registrar_venta(&t.db, 1, None, CreateSaleDto {
            items: vec![CreateSaleItemDto { product_id: p, quantity: 2, unit_price: 0.0, discount: 0.0, variant_id: None }],
            payment_method: "cash".to_string(),
            amount_paid: 0.0,
            payments: vec![
                PaymentSplitDto { method: "card".into(), amount: 150.0 },
                PaymentSplitDto { method: "cash".into(), amount: 50.0 },
            ],
            discount_total: 0.0, promotion_id: None, requiere_factura: false,
            notes: None, customer_id: None, client_request_id: None,
        }).unwrap().id;

        let partida = t.partidas(venta_id)[0];
        assert_eq!(t.devolver(venta_id, vec![(partida, 2)], "cash").unwrap(), 200.0);
    }
}
