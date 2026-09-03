use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::cash_register::{CashRegister, CloseRegisterDto, OpenRegisterDto};
use crate::money::Cents;
use crate::session::{require_auth, SessionState};

/// Explicit column list: `cr.*` would silently shift every index the moment a
/// migration adds a column to the table.
const REGISTER_COLUMNS: &str = "cr.id, cr.user_id, u.full_name, cr.opening_amount, cr.closing_amount,
     cr.expected_amount, cr.difference, cr.total_sales, cr.total_cash_sales, cr.total_card_sales,
     cr.total_transfer_sales, cr.total_layaway_cash, cr.total_layaway_card, cr.total_layaway_transfer,
     cr.total_refunds_cash, cr.total_expenses, cr.sale_count, cr.status, cr.opened_at, cr.closed_at";

fn map_register(row: &rusqlite::Row) -> rusqlite::Result<CashRegister> {
    Ok(CashRegister {
        id: row.get(0)?,
        user_id: row.get(1)?,
        user_name: row.get(2)?,
        opening_amount: row.get(3)?,
        closing_amount: row.get(4)?,
        expected_amount: row.get(5)?,
        difference: row.get(6)?,
        total_sales: row.get(7)?,
        total_cash_sales: row.get(8)?,
        total_card_sales: row.get(9)?,
        total_transfer_sales: row.get(10)?,
        total_layaway_cash: row.get(11)?,
        total_layaway_card: row.get(12)?,
        total_layaway_transfer: row.get(13)?,
        total_refunds_cash: row.get(14)?,
        total_expenses: row.get(15)?,
        sale_count: row.get(16)?,
        status: row.get(17)?,
        opened_at: row.get(18)?,
        closed_at: row.get(19)?,
    })
}

/// Cash that should physically be in the drawer right now.
///
/// Every movement that adds or removes bills has to appear here, or the close-out
/// reports a phantom surplus/shortfall: sales in cash come in, layaway deposits in
/// cash come in, cash refunds go out, and petty-cash expenses go out.
pub fn expected_cash(r: &CashRegister) -> f64 {
    let p = Cents::from_pesos;
    // En centavos enteros: un turno con cientos de movimientos no acumula el
    // error de redondeo que arrastraría sumar f64 uno tras otro.
    let expected = p(r.opening_amount) + p(r.total_cash_sales) + p(r.total_layaway_cash)
        - p(r.total_refunds_cash)
        - p(r.total_expenses);
    expected.to_pesos()
}

/// Id of the shift currently open, if any. Shared by sales, layaways and returns
/// so every money movement lands on the right cut.
pub fn open_register_id(db: &rusqlite::Connection) -> Option<i64> {
    db.query_row(
        "SELECT id FROM cash_registers WHERE status = 'open' ORDER BY opened_at DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .ok()
}

#[tauri::command]
pub fn open_register(state: State<DbState>, sessions: State<SessionState>, token: String, data: OpenRegisterDto) -> Result<CashRegister, String> {
    // The cashier on record is the authenticated user, never a client-sent id.
    let user_id = require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    if data.opening_amount < 0.0 {
        return Err("El fondo de apertura no puede ser negativo".to_string());
    }

    // Check if there's already an open register
    let open_count: i64 = db.query_row(
        "SELECT COUNT(*) FROM cash_registers WHERE status = 'open'",
        [],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    if open_count > 0 {
        return Err("Ya hay una caja abierta. Ciérrela primero.".to_string());
    }

    db.execute(
        "INSERT INTO cash_registers (user_id, opening_amount) VALUES (?1, ?2)",
        params![user_id, data.opening_amount],
    ).map_err(|e| e.to_string())?;

    let id = db.last_insert_rowid();
    db.execute(
        "INSERT INTO app_logs (level, module, message, user_id) VALUES ('info', 'caja', ?1, ?2)",
        params![format!("Caja abierta con fondo de {}", data.opening_amount), user_id],
    ).ok();

    get_register_by_id(&db, id)
}

#[tauri::command]
pub fn close_register(state: State<DbState>, sessions: State<SessionState>, token: String, data: CloseRegisterDto) -> Result<CashRegister, String> {
    let user_id = require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    cerrar_caja(&db, user_id, data)
}

/// Núcleo del corte, con la conexión explícita.
pub fn cerrar_caja(
    db: &rusqlite::Connection,
    user_id: i64,
    data: CloseRegisterDto,
) -> Result<CashRegister, String> {
    let register = get_open_register_internal(db)?;

    let expected = expected_cash(&register);
    let difference =
        (Cents::from_pesos(data.closing_amount) - Cents::from_pesos(expected)).to_pesos();

    db.execute(
        "UPDATE cash_registers SET closing_amount=?1, expected_amount=?2, difference=?3, status='closed', closed_at=datetime('now','localtime') WHERE id=?4",
        params![data.closing_amount, expected, difference, register.id],
    ).map_err(|e| e.to_string())?;

    db.execute(
        "INSERT INTO app_logs (level, module, message, user_id) VALUES (?1, 'caja', ?2, ?3)",
        params![
            if Cents::from_pesos(difference).abs().is_positive() { "warn" } else { "info" },
            format!(
                "Caja cerrada: esperado {}, contado {}, diferencia {}",
                expected, data.closing_amount, difference
            ),
            user_id
        ],
    ).ok();

    get_register_by_id(db, register.id)
}

#[tauri::command]
pub fn get_open_register(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Option<CashRegister>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    match get_open_register_internal(&db) {
        Ok(reg) => Ok(Some(reg)),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub fn get_register_history(state: State<DbState>, sessions: State<SessionState>, token: String, limit: Option<i32>) -> Result<Vec<CashRegister>, String> {
    require_auth(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let limit = limit.unwrap_or(30);

    let mut stmt = db.prepare(&format!(
        "SELECT {} FROM cash_registers cr
         LEFT JOIN users u ON cr.user_id = u.id
         ORDER BY cr.opened_at DESC LIMIT ?1",
        REGISTER_COLUMNS
    )).map_err(|e| e.to_string())?;

    let registers = stmt
        .query_map(params![limit], map_register)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(registers)
}

fn get_open_register_internal(db: &rusqlite::Connection) -> Result<CashRegister, String> {
    db.query_row(
        &format!(
            "SELECT {} FROM cash_registers cr
             LEFT JOIN users u ON cr.user_id = u.id
             WHERE cr.status = 'open' LIMIT 1",
            REGISTER_COLUMNS
        ),
        [],
        map_register,
    ).map_err(|_| "No hay caja abierta".to_string())
}

fn get_register_by_id(db: &rusqlite::Connection, id: i64) -> Result<CashRegister, String> {
    db.query_row(
        &format!(
            "SELECT {} FROM cash_registers cr
             LEFT JOIN users u ON cr.user_id = u.id
             WHERE cr.id = ?1",
            REGISTER_COLUMNS
        ),
        params![id],
        map_register,
    ).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register() -> CashRegister {
        CashRegister {
            id: 1, user_id: 1, user_name: None,
            opening_amount: 1000.0, closing_amount: None, expected_amount: None, difference: None,
            total_sales: 0.0, total_cash_sales: 0.0, total_card_sales: 0.0, total_transfer_sales: 0.0,
            total_layaway_cash: 0.0, total_layaway_card: 0.0, total_layaway_transfer: 0.0,
            total_refunds_cash: 0.0, total_expenses: 0.0, sale_count: 0,
            status: "open".to_string(), opened_at: String::new(), closed_at: None,
        }
    }

    #[test]
    fn an_untouched_shift_expects_its_opening_float() {
        assert_eq!(expected_cash(&register()), 1000.0);
    }

    #[test]
    fn card_and_transfer_sales_never_reach_the_drawer() {
        let r = CashRegister { total_card_sales: 500.0, total_transfer_sales: 300.0, ..register() };
        assert_eq!(expected_cash(&r), 1000.0);
    }

    #[test]
    fn layaway_deposits_in_cash_are_counted() {
        // El caso que antes aparecía como sobrante en el corte.
        let r = CashRegister { total_layaway_cash: 500.0, ..register() };
        assert_eq!(expected_cash(&r), 1500.0);
    }

    #[test]
    fn layaway_deposits_by_card_are_not() {
        let r = CashRegister { total_layaway_card: 500.0, total_layaway_transfer: 200.0, ..register() };
        assert_eq!(expected_cash(&r), 1000.0);
    }

    #[test]
    fn cash_refunds_leave_the_drawer() {
        // El caso que antes aparecía como faltante en el corte.
        let r = CashRegister { total_cash_sales: 800.0, total_refunds_cash: 300.0, ..register() };
        assert_eq!(expected_cash(&r), 1500.0);
    }

    #[test]
    fn every_movement_adds_up_together() {
        let r = CashRegister {
            total_cash_sales: 2500.0,
            total_card_sales: 900.0,
            total_layaway_cash: 400.0,
            total_layaway_card: 150.0,
            total_refunds_cash: 250.0,
            total_expenses: 180.0,
            ..register()
        };
        // 1000 + 2500 + 400 - 250 - 180
        assert_eq!(expected_cash(&r), 3470.0);
    }

    #[test]
    fn the_result_is_rounded_to_cents() {
        let r = CashRegister { total_cash_sales: 0.1, total_layaway_cash: 0.2, ..register() };
        assert_eq!(expected_cash(&r), 1000.30);
    }

    #[test]
    fn many_small_movements_do_not_drift_a_single_cent() {
        // Sumar 0.1 + 0.2 en f64 da 0.30000000000000004; en centavos, 0.30.
        let r = CashRegister {
            opening_amount: 0.0,
            total_cash_sales: 0.1,
            total_layaway_cash: 0.2,
            total_expenses: 0.3,
            ..register()
        };
        assert_eq!(expected_cash(&r), 0.0);
    }
}

/// Un día completo de mostrador, comprobando que el corte cuadre contra el
/// dinero que realmente quedaría en el cajón.
///
/// Cada tipo de movimiento se probó por separado; esto verifica que juntos, en
/// el orden en que ocurren en una tienda, siguen dando el mismo número.
#[cfg(test)]
mod dia_completo {
    use super::*;
    use crate::commands::layaways::{abonar_apartado, registrar_apartado};
    use crate::commands::returns::{registrar_devolucion, CreateReturnDto, ReturnItemDto};
    use crate::commands::sales::registrar_venta;
    use crate::models::layaway::{CreateLayawayDto, CreateLayawayItemDto};
    use crate::models::sale::{CreateSaleDto, CreateSaleItemDto, PaymentSplitDto};

    struct Mostrador {
        db: rusqlite::Connection,
        /// Lo que un humano contaría en el cajón siguiendo cada movimiento.
        efectivo_real: f64,
    }

    impl Mostrador {
        fn abre_con(fondo: f64) -> Mostrador {
            let db = rusqlite::Connection::open_in_memory().unwrap();
            db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
            crate::db::migrations::run_migrations(&db).unwrap();
            db.execute(
                "INSERT INTO users (id, username, password_hash, full_name, role)
                 VALUES (1, 'u', 'x', 'U', 'admin')", [],
            ).unwrap();
            db.execute(
                "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
                 VALUES (1, 'CAM', 'Camisa', 50.0, 100.0, 500)", [],
            ).unwrap();
            db.execute(
                "INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, ?1)",
                params![fondo],
            ).unwrap();
            Mostrador { db, efectivo_real: fondo }
        }

        fn venta_efectivo(&mut self, piezas: i32, recibe: f64) -> i64 {
            let sale = registrar_venta(&self.db, 1, None, CreateSaleDto {
                items: vec![CreateSaleItemDto {
                    product_id: 1, quantity: piezas, unit_price: 0.0, discount: 0.0, variant_id: None,
                }],
                payment_method: "cash".to_string(), amount_paid: recibe,
                payments: vec![], discount_total: 0.0, promotion_id: None,
                requiere_factura: false, notes: None, customer_id: None, client_request_id: None,
            }).unwrap();
            // Entra lo que dio el cliente y sale el cambio.
            self.efectivo_real += recibe - sale.change_amount;
            sale.id
        }

        fn venta_tarjeta(&mut self, piezas: i32) {
            registrar_venta(&self.db, 1, None, CreateSaleDto {
                items: vec![CreateSaleItemDto {
                    product_id: 1, quantity: piezas, unit_price: 0.0, discount: 0.0, variant_id: None,
                }],
                payment_method: "card".to_string(), amount_paid: 0.0,
                payments: vec![], discount_total: 0.0, promotion_id: None,
                requiere_factura: false, notes: None, customer_id: None, client_request_id: None,
            }).unwrap();
            // No toca el cajón.
        }

        fn venta_mixta(&mut self, piezas: i32, tarjeta: f64, efectivo: f64) {
            let sale = registrar_venta(&self.db, 1, None, CreateSaleDto {
                items: vec![CreateSaleItemDto {
                    product_id: 1, quantity: piezas, unit_price: 0.0, discount: 0.0, variant_id: None,
                }],
                payment_method: "cash".to_string(), amount_paid: 0.0,
                payments: vec![
                    PaymentSplitDto { method: "card".into(), amount: tarjeta },
                    PaymentSplitDto { method: "cash".into(), amount: efectivo },
                ],
                discount_total: 0.0, promotion_id: None,
                requiere_factura: false, notes: None, customer_id: None, client_request_id: None,
            }).unwrap();
            self.efectivo_real += efectivo - sale.change_amount;
        }

        fn gasto(&mut self, monto: f64) {
            let cr = open_register_id(&self.db).unwrap();
            self.db.execute(
                "INSERT INTO expenses (cash_register_id, category, description, amount, user_id)
                 VALUES (?1, 'varios', 'gasto', ?2, 1)",
                params![cr, monto],
            ).unwrap();
            self.db.execute(
                "UPDATE cash_registers SET total_expenses = total_expenses + ?1 WHERE id = ?2",
                params![monto, cr],
            ).unwrap();
            self.efectivo_real -= monto;
        }

        fn apartado_con_anticipo(&mut self, piezas: i32, anticipo: f64) -> i64 {
            let l = registrar_apartado(&self.db, 1, CreateLayawayDto {
                customer_id: None, notes: None, due_date: None,
                initial_payment: anticipo, payment_method: "cash".to_string(),
                items: vec![CreateLayawayItemDto {
                    product_id: 1, quantity: piezas, unit_price: 0.0, variant_id: None,
                }],
            }).unwrap();
            self.efectivo_real += anticipo;
            l.id
        }

        fn abono(&mut self, layaway_id: i64, monto: f64) {
            abonar_apartado(&self.db, 1, layaway_id, monto, "cash").unwrap();
            self.efectivo_real += monto;
        }

        fn devolucion_efectivo(&mut self, sale_id: i64, piezas: i32) {
            let partida: i64 = self.db.query_row(
                "SELECT id FROM sale_items WHERE sale_id = ?1", params![sale_id], |r| r.get(0)).unwrap();
            let reembolso = registrar_devolucion(&self.db, 1, CreateReturnDto {
                sale_id, reason: None, refund_method: "cash".to_string(),
                items: vec![ReturnItemDto { sale_item_id: partida, quantity: piezas }],
            }).unwrap();
            self.efectivo_real -= reembolso;
        }

        fn abierta(&self) -> CashRegister {
            get_open_register_internal(&self.db).unwrap()
        }
    }

    #[test]
    fn el_corte_cuadra_tras_un_dia_con_movimientos_de_todo_tipo() {
        let mut m = Mostrador::abre_con(1500.0);

        let v1 = m.venta_efectivo(2, 500.0);   // 200, cambio 300
        m.venta_tarjeta(3);                     // 300 por tarjeta
        m.venta_mixta(4, 250.0, 200.0);         // 400: 250 tarjeta + 150 efectivo, cambio 50
        let apartado = m.apartado_con_anticipo(5, 200.0);
        m.abono(apartado, 150.0);
        m.gasto(180.0);
        m.devolucion_efectivo(v1, 1);           // devuelve 100

        let caja = m.abierta();
        assert_eq!(
            expected_cash(&caja), m.efectivo_real,
            "lo que el sistema espera debe ser lo que hay en el cajón",
        );

        // Y al cerrar contando exactamente eso, no debe haber diferencia.
        let cerrada = cerrar_caja(&m.db, 1, CloseRegisterDto { closing_amount: m.efectivo_real }).unwrap();
        assert_eq!(cerrada.difference, Some(0.0));
        assert_eq!(cerrada.status, "closed");
    }

    #[test]
    fn un_faltante_se_reporta_con_su_signo() {
        let mut m = Mostrador::abre_con(1000.0);
        m.venta_efectivo(1, 100.0);

        // Faltan 50 pesos en el cajón.
        let cerrada = cerrar_caja(&m.db, 1, CloseRegisterDto {
            closing_amount: m.efectivo_real - 50.0,
        }).unwrap();

        assert_eq!(cerrada.difference, Some(-50.0));
    }

    #[test]
    fn un_sobrante_se_reporta_con_su_signo() {
        let mut m = Mostrador::abre_con(1000.0);
        m.venta_efectivo(1, 100.0);

        let cerrada = cerrar_caja(&m.db, 1, CloseRegisterDto {
            closing_amount: m.efectivo_real + 25.0,
        }).unwrap();

        assert_eq!(cerrada.difference, Some(25.0));
    }

    #[test]
    fn no_se_cierra_una_caja_que_no_esta_abierta() {
        let m = Mostrador::abre_con(0.0);
        cerrar_caja(&m.db, 1, CloseRegisterDto { closing_amount: 0.0 }).unwrap();

        assert!(cerrar_caja(&m.db, 1, CloseRegisterDto { closing_amount: 0.0 }).is_err());
    }

    #[test]
    fn el_corte_guarda_lo_esperado_para_poder_auditarlo_despues() {
        let mut m = Mostrador::abre_con(500.0);
        m.venta_efectivo(3, 300.0);

        let cerrada = cerrar_caja(&m.db, 1, CloseRegisterDto { closing_amount: m.efectivo_real }).unwrap();

        assert_eq!(cerrada.expected_amount, Some(m.efectivo_real));
        assert_eq!(cerrada.closing_amount, Some(m.efectivo_real));
        assert!(cerrada.closed_at.is_some());
    }

    #[test]
    fn tras_cerrar_no_se_puede_seguir_cobrando() {
        let mut m = Mostrador::abre_con(0.0);
        cerrar_caja(&m.db, 1, CloseRegisterDto { closing_amount: 0.0 }).unwrap();

        let r = registrar_venta(&m.db, 1, None, CreateSaleDto {
            items: vec![CreateSaleItemDto {
                product_id: 1, quantity: 1, unit_price: 0.0, discount: 0.0, variant_id: None,
            }],
            payment_method: "cash".to_string(), amount_paid: 1000.0,
            payments: vec![], discount_total: 0.0, promotion_id: None,
            requiere_factura: false, notes: None, customer_id: None, client_request_id: None,
        });
        assert!(r.is_err(), "sin turno abierto no hay dónde registrar el dinero");
        let _ = &mut m;
    }
}
