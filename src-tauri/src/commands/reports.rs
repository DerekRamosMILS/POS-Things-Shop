use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use tauri::State;

use crate::db::connection::DbState;
use crate::money::Cents;
use crate::session::{require_admin, SessionState};

#[derive(Debug, Serialize)]
pub struct DailySalesReport {
    pub date: String,
    pub total_sales: f64,
    pub sale_count: i64,
    pub total_cash: f64,
    pub total_card: f64,
    pub total_transfer: f64,
    pub total_expenses: f64,
    pub gross_profit: f64,
}

#[derive(Debug, Serialize)]
pub struct TopProduct {
    pub product_id: i64,
    pub product_name: String,
    pub total_quantity: i64,
    pub total_revenue: f64,
}

#[derive(Debug, Serialize)]
pub struct CashierReport {
    pub user_id: i64,
    pub user_name: String,
    pub sale_count: i64,
    pub total_sales: f64,
    pub gross_profit: f64,
}

#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub today_sales: f64,
    pub today_count: i64,
    pub month_sales: f64,
    pub month_count: i64,
    pub total_products: i64,
    pub low_stock_count: i64,
    pub today_profit: f64,
}

/// Lo que de verdad quedó de una venta: su total menos lo que se devolvió.
///
/// Una devolución parcial deja la venta como `completed` con su total original,
/// así que sumar `s.total` a secas contaba como ingreso dinero que ya se le
/// había regresado al cliente.
const NETO_VENTA: &str =
    "(s.total - COALESCE((SELECT SUM(r.total_refund) FROM returns r WHERE r.sale_id = s.id), 0))";

/// Costo unitario con el que se calcula la utilidad de una partida.
///
/// El bueno es el que se guardó al vender (`unit_cost > 0`). Los renglones de
/// antes de que existiera esa columna no lo tienen, y caían al costo *actual* del
/// producto: subirle el costo a una prenda rehacía hacia atrás la utilidad de
/// todos los meses en que se vendía más barata, sin que nada lo dijera.
///
/// Para esos renglones se busca en el historial de costos el primer cambio
/// posterior a la venta: su `old_price` es lo que costaba ese día. Si no hay
/// ningún cambio posterior, el costo de hoy sigue siendo el de entonces.
///
/// Depende de que la consulta tenga `sales s` y `products p` a la vista.
const COSTO_AL_VENDER: &str = "COALESCE(
        NULLIF(si.unit_cost, 0),
        (SELECT ph.old_price FROM price_history ph
          WHERE ph.product_id = si.product_id AND ph.tipo = 'costo'
            AND ph.created_at > s.created_at
          ORDER BY ph.created_at ASC, ph.id ASC LIMIT 1),
        p.purchase_price)";

/// Utilidad de una partida, solo por las piezas que no se devolvieron.
fn utilidad_partida() -> String {
    format!(
        "(si.subtotal * (si.quantity - si.returned_quantity) / si.quantity
          - {} * (si.quantity - si.returned_quantity))",
        COSTO_AL_VENDER
    )
}

/// Filtro de antigüedad, como parámetro `?1` con la forma `-30`.
const DESDE: &str = "datetime('now', ?1 || ' days', 'localtime')";

fn filas<T>(
    db: &rusqlite::Connection,
    sql: &str,
    params: impl rusqlite::Params,
    f: impl FnMut(&rusqlite::Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>, String> {
    let mut stmt = db.prepare(sql).map_err(|e| e.to_string())?;
    let out = stmt
        .query_map(params, f)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string());
    out
}

pub(crate) fn panel(db: &rusqlite::Connection) -> Result<DashboardStats, String> {
    crate::commands::notifications::rearmar_alertas(db);
    let uno = |sql: &str| -> Result<f64, String> {
        db.query_row(sql, [], |row| row.get(0)).map_err(|e| e.to_string())
    };
    let cuenta = |sql: &str| -> Result<i64, String> {
        db.query_row(sql, [], |row| row.get(0)).map_err(|e| e.to_string())
    };

    Ok(DashboardStats {
        today_sales: uno(&format!(
            "SELECT COALESCE(SUM({}), 0) FROM sales s
             WHERE date(s.created_at) = date('now','localtime') AND s.status = 'completed'",
            NETO_VENTA
        ))?,
        today_count: cuenta(
            "SELECT COUNT(*) FROM sales WHERE date(created_at) = date('now','localtime') AND status = 'completed'",
        )?,
        month_sales: uno(&format!(
            "SELECT COALESCE(SUM({}), 0) FROM sales s
             WHERE strftime('%Y-%m', s.created_at) = strftime('%Y-%m', 'now','localtime') AND s.status = 'completed'",
            NETO_VENTA
        ))?,
        month_count: cuenta(
            "SELECT COUNT(*) FROM sales WHERE strftime('%Y-%m', created_at) = strftime('%Y-%m', 'now','localtime') AND status = 'completed'",
        )?,
        total_products: cuenta("SELECT COUNT(*) FROM products WHERE is_active = 1")?,
        low_stock_count: cuenta(
            "SELECT COUNT(*) FROM products WHERE stock <= min_stock AND is_active = 1 AND low_stock_ignored = 0",
        )?,
        today_profit: uno(&format!(
            "SELECT COALESCE(SUM({}), 0)
             FROM sale_items si
             JOIN sales s ON si.sale_id = s.id
             JOIN products p ON si.product_id = p.id
             WHERE date(s.created_at) = date('now','localtime') AND s.status = 'completed'",
            utilidad_partida()
        ))?,
    })
}

#[tauri::command]
pub fn get_dashboard_stats(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<DashboardStats, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    panel(&db)
}

/// El renglón de un día, creándolo en ceros la primera vez.
fn dia<'a>(dias: &'a mut BTreeMap<String, DailySalesReport>, d: &str) -> &'a mut DailySalesReport {
    dias.entry(d.to_string()).or_insert_with(|| DailySalesReport {
        date: d.to_string(),
        total_sales: 0.0,
        sale_count: 0,
        total_cash: 0.0,
        total_card: 0.0,
        total_transfer: 0.0,
        total_expenses: 0.0,
        gross_profit: 0.0,
    })
}

/// Reporte por día.
///
/// Dos cosas distintas en el mismo renglón, cada una con su propia fecha:
///
/// - **Ventas y utilidad** se reconocen el día de la venta, netas de lo que se
///   devolvió de ella. Un apartado cuenta el día que se entrega.
/// - **Efectivo, tarjeta y transferencia** son el dinero que se movió ese día:
///   lo cobrado en ventas directas, los abonos de apartados el día que entraron
///   y los reembolsos el día que salieron. Antes los abonos no aparecían hasta la
///   entrega, y ese día aparecía de golpe el efectivo de semanas: ningún día del
///   reporte cuadraba con su corte.
pub(crate) fn reporte_diario(db: &rusqlite::Connection, days: i32) -> Result<Vec<DailySalesReport>, String> {
    let since = format!("-{}", days);
    let mut dias: BTreeMap<String, DailySalesReport> = BTreeMap::new();
    // Suma en centavos por día y método, para no arrastrar error de redondeo.
    let mut dinero: HashMap<(String, String), Cents> = HashMap::new();

    for (d, total, cuantas) in filas(
        db,
        &format!(
            "SELECT date(s.created_at), COALESCE(SUM({}), 0), COUNT(*)
             FROM sales s
             WHERE s.status = 'completed' AND s.created_at >= {}
             GROUP BY date(s.created_at)",
            NETO_VENTA, DESDE
        ),
        params![since],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, i64>(2)?)),
    )? {
        let r = dia(&mut dias, &d);
        r.total_sales = Cents::from_pesos(total).to_pesos();
        r.sale_count = cuantas;
    }

    // Lo cobrado en ventas directas. Las devueltas también: ese dinero sí
    // entró, y su reembolso sale abajo el día en que salió. Las entregas de
    // apartado no: su dinero ya se contó abono por abono.
    for (d, metodo, monto) in filas(
        db,
        &format!(
            "SELECT date(s.created_at), sp.method, SUM(sp.amount)
             FROM sale_payments sp JOIN sales s ON s.id = sp.sale_id
             WHERE s.status IN ('completed', 'returned') AND s.layaway_id IS NULL
               AND s.created_at >= {}
             GROUP BY date(s.created_at), sp.method",
            DESDE
        ),
        params![since],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?)),
    )? {
        let c = dinero.entry((d, metodo)).or_default();
        *c = *c + Cents::from_pesos(monto);
    }

    for (d, metodo, monto) in filas(
        db,
        &format!(
            "SELECT date(created_at), payment_method, SUM(amount) FROM layaway_payments
             WHERE created_at >= {} GROUP BY date(created_at), payment_method",
            DESDE
        ),
        params![since],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?)),
    )? {
        let c = dinero.entry((d, metodo)).or_default();
        *c = *c + Cents::from_pesos(monto);
    }

    for (d, metodo, monto) in filas(
        db,
        &format!(
            "SELECT date(created_at), refund_method, SUM(total_refund) FROM returns
             WHERE created_at >= {} GROUP BY date(created_at), refund_method",
            DESDE
        ),
        params![since],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, f64>(2)?)),
    )? {
        let c = dinero.entry((d, metodo)).or_default();
        *c = *c - Cents::from_pesos(monto);
    }

    for ((d, metodo), monto) in dinero {
        let r = dia(&mut dias, &d);
        match metodo.as_str() {
            "cash" => r.total_cash = monto.to_pesos(),
            "card" => r.total_card = monto.to_pesos(),
            "transfer" => r.total_transfer = monto.to_pesos(),
            _ => {}
        }
    }

    for (d, utilidad) in filas(
        db,
        &format!(
            "SELECT date(s.created_at), COALESCE(SUM({}), 0)
             FROM sale_items si
             JOIN sales s ON si.sale_id = s.id
             JOIN products p ON si.product_id = p.id
             WHERE s.status = 'completed' AND s.created_at >= {}
             GROUP BY date(s.created_at)",
            utilidad_partida(), DESDE
        ),
        params![since],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)),
    )? {
        dia(&mut dias, &d).gross_profit = Cents::from_pesos(utilidad).to_pesos();
    }

    for (d, gastos) in filas(
        db,
        &format!(
            "SELECT date(created_at), COALESCE(SUM(amount), 0) FROM expenses
             WHERE created_at >= {} GROUP BY date(created_at)",
            DESDE
        ),
        params![since],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)),
    )? {
        dia(&mut dias, &d).total_expenses = Cents::from_pesos(gastos).to_pesos();
    }

    Ok(dias.into_values().rev().collect())
}

#[tauri::command]
pub fn get_daily_sales_report(state: State<DbState>, sessions: State<SessionState>, token: String, days: Option<i32>) -> Result<Vec<DailySalesReport>, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    reporte_diario(&db, days.unwrap_or(30))
}

pub(crate) fn mas_vendidos(db: &rusqlite::Connection, days: i32, limit: i32) -> Result<Vec<TopProduct>, String> {
    // Solo las piezas que se quedaron vendidas.
    filas(
        db,
        &format!(
            "SELECT si.product_id, si.product_name,
                    SUM(si.quantity - si.returned_quantity) AS total_qty,
                    SUM(si.subtotal * (si.quantity - si.returned_quantity) / si.quantity) AS total_rev
             FROM sale_items si
             JOIN sales s ON si.sale_id = s.id
             WHERE s.status = 'completed' AND s.created_at >= {}
             GROUP BY si.product_id
             HAVING total_qty > 0
             ORDER BY total_qty DESC
             LIMIT ?2",
            DESDE
        ),
        params![format!("-{}", days), limit],
        |row| {
            Ok(TopProduct {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                total_quantity: row.get(2)?,
                total_revenue: Cents::from_pesos(row.get(3)?).to_pesos(),
            })
        },
    )
}

#[tauri::command]
pub fn get_top_products(state: State<DbState>, sessions: State<SessionState>, token: String, days: Option<i32>, limit: Option<i32>) -> Result<Vec<TopProduct>, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    mas_vendidos(&db, days.unwrap_or(30), limit.unwrap_or(10))
}

pub(crate) fn por_cajero(db: &rusqlite::Connection, days: i32) -> Result<Vec<CashierReport>, String> {
    let since = format!("-{}", days);
    let mut reports = filas(
        db,
        &format!(
            "SELECT s.user_id, u.full_name, COUNT(*) AS cnt, COALESCE(SUM({}), 0) AS total
             FROM sales s
             JOIN users u ON s.user_id = u.id
             WHERE s.status = 'completed' AND s.created_at >= {}
             GROUP BY s.user_id
             ORDER BY total DESC",
            NETO_VENTA, DESDE
        ),
        params![since],
        |row| {
            Ok(CashierReport {
                user_id: row.get(0)?,
                user_name: row.get(1)?,
                sale_count: row.get(2)?,
                total_sales: Cents::from_pesos(row.get(3)?).to_pesos(),
                gross_profit: 0.0,
            })
        },
    )?;

    let idx: HashMap<i64, usize> = reports.iter().enumerate().map(|(i, r)| (r.user_id, i)).collect();
    for (uid, p) in filas(
        db,
        &format!(
            "SELECT s.user_id, COALESCE(SUM({}), 0)
             FROM sale_items si
             JOIN sales s ON si.sale_id = s.id
             JOIN products p ON si.product_id = p.id
             WHERE s.status = 'completed' AND s.created_at >= {}
             GROUP BY s.user_id",
            utilidad_partida(), DESDE
        ),
        params![since],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?)),
    )? {
        if let Some(&i) = idx.get(&uid) {
            reports[i].gross_profit = Cents::from_pesos(p).to_pesos();
        }
    }
    Ok(reports)
}

#[tauri::command]
pub fn get_cashier_report(state: State<DbState>, sessions: State<SessionState>, token: String, days: Option<i32>) -> Result<Vec<CashierReport>, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    por_cajero(&db, days.unwrap_or(30))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::layaways::{entregar_apartado, registrar_apartado, abonar_apartado};
    use crate::commands::returns::{registrar_devolucion, CreateReturnDto, ReturnItemDto};
    use crate::commands::sales::{cancelar_venta, registrar_venta};
    use crate::models::layaway::{CreateLayawayDto, CreateLayawayItemDto};
    use crate::models::sale::{CreateSaleDto, CreateSaleItemDto, PaymentSplitDto};

    /// Una tienda con caja abierta y un producto de $100 que costó $50.
    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')",
            [],
        ).unwrap();
        db.execute("INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, 0)", []).unwrap();
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (1, 'P', 'P', 50.0, 100.0, 1000)",
            [],
        ).unwrap();
        db
    }

    fn cobrar(db: &rusqlite::Connection, cantidad: i32, pagos: Vec<(&str, f64)>) -> i64 {
        registrar_venta(db, 1, None, CreateSaleDto {
            items: vec![CreateSaleItemDto {
                product_id: 1, quantity: cantidad, unit_price: 0.0, discount: 0.0, variant_id: None,
            }],
            payment_method: "cash".to_string(),
            amount_paid: 100_000.0,
            payments: pagos.into_iter()
                .map(|(m, a)| PaymentSplitDto { method: m.to_string(), amount: a })
                .collect(),
            discount_total: 0.0,
            promotion_id: None,
            requiere_factura: false,
            notes: None,
            customer_id: None,
            client_request_id: None,
        }).unwrap().id
    }

    fn devolver(db: &rusqlite::Connection, venta: i64, piezas: i32, metodo: &str) {
        let partida: i64 = db.query_row(
            "SELECT id FROM sale_items WHERE sale_id = ?1", params![venta], |r| r.get(0)).unwrap();
        registrar_devolucion(db, 1, CreateReturnDto {
            sale_id: venta,
            reason: None,
            refund_method: metodo.to_string(),
            items: vec![ReturnItemDto { sale_item_id: partida, quantity: piezas }],
        }).unwrap();
    }

    fn hoy(db: &rusqlite::Connection) -> DailySalesReport {
        let fecha: String = db.query_row("SELECT date('now','localtime')", [], |r| r.get(0)).unwrap();
        reporte_diario(db, 30).unwrap().into_iter().find(|d| d.date == fecha)
            .unwrap_or_else(|| panic!("el reporte no trae el día de hoy"))
    }

    #[test]
    fn subir_un_costo_no_reescribe_la_utilidad_de_lo_ya_vendido() {
        // Los renglones de antes de que se guardara el costo al vender caían al
        // costo *de hoy*: subirle el costo a una prenda rehacía hacia atrás la
        // utilidad de los meses en que se vendía más barata. Con el historial de
        // costos se puede saber cuánto costaba el día de la venta.
        let db = tienda();
        let venta = cobrar(&db, 1, vec![("cash", 100.0)]);
        // Un renglón viejo, sin costo propio, como los de antes de la migración 005.
        db.execute("UPDATE sale_items SET unit_cost = 0 WHERE sale_id = ?1", params![venta]).unwrap();

        assert_eq!(panel(&db).unwrap().today_profit, 50.0, "costó 50 y se vendió en 100");

        // Más tarde sube el proveedor y se anota el cambio.
        crate::commands::products::anotar_precio(&db, 1, "costo", 50.0, 80.0, 1).unwrap();
        db.execute(
            "UPDATE price_history SET created_at = datetime(created_at, '+1 hour') WHERE tipo = 'costo'",
            [],
        ).unwrap();
        db.execute("UPDATE products SET purchase_price = 80.0 WHERE id = 1", []).unwrap();

        assert_eq!(
            panel(&db).unwrap().today_profit, 50.0,
            "la utilidad de una venta que ya pasó no se mueve"
        );
    }

    #[test]
    fn un_renglon_viejo_sin_historial_sigue_usando_el_costo_de_hoy() {
        // Es lo único que se puede saber de una venta anterior a que se guardara
        // nada: se mantiene como estaba, no se cuenta utilidad de más.
        let db = tienda();
        let venta = cobrar(&db, 1, vec![("cash", 100.0)]);
        db.execute("UPDATE sale_items SET unit_cost = 0 WHERE sale_id = ?1", params![venta]).unwrap();

        assert_eq!(panel(&db).unwrap().today_profit, 50.0);
    }

    fn hace(db: &rusqlite::Connection, dias: i32) -> String {
        db.query_row(&format!("SELECT date('now','localtime','-{} days')", dias), [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn una_venta_mixta_aparece_repartida_entre_sus_metodos() {
        let db = tienda();
        cobrar(&db, 3, vec![("card", 200.0), ("cash", 150.0)]);

        let d = hoy(&db);

        assert_eq!(d.total_sales, 300.0);
        assert_eq!(d.total_card, 200.0);
        assert_eq!(d.total_cash, 100.0);
        assert_eq!(d.total_transfer, 0.0);
    }

    #[test]
    fn el_desglose_suma_el_total_con_ventas_de_todo_tipo() {
        let db = tienda();
        cobrar(&db, 1, vec![("cash", 500.0)]);
        cobrar(&db, 2, vec![("card", 200.0)]);
        cobrar(&db, 1, vec![("transfer", 100.0)]);
        cobrar(&db, 4, vec![("card", 300.0), ("transfer", 50.0), ("cash", 100.0)]);

        let d = hoy(&db);

        assert_eq!(d.total_sales, 800.0);
        assert_eq!(d.total_cash + d.total_card + d.total_transfer, d.total_sales);
    }

    #[test]
    fn una_venta_cancelada_sale_del_reporte() {
        let db = tienda();
        cobrar(&db, 1, vec![("cash", 100.0)]);
        let cancelada = cobrar(&db, 2, vec![("card", 200.0)]);
        cancelar_venta(&db, 1, cancelada).unwrap();

        let d = hoy(&db);

        assert_eq!(d.total_sales, 100.0);
        assert_eq!(d.sale_count, 1);
        assert_eq!(d.total_card, 0.0, "lo cancelado no debe seguir contando");
    }

    #[test]
    fn una_devolucion_parcial_sale_de_ingresos_utilidad_y_efectivo() {
        // Se cobraban $300 y $150 de utilidad por una venta de la que ya se
        // habían regresado $100: el reporte decía más de lo que quedó.
        let db = tienda();
        let venta = cobrar(&db, 3, vec![("cash", 300.0)]);
        devolver(&db, venta, 1, "cash");

        let d = hoy(&db);
        assert_eq!(d.total_sales, 200.0);
        assert_eq!(d.gross_profit, 100.0, "dos piezas de $100 que costaron $50");
        assert_eq!(d.total_cash, 200.0);

        let panel = panel(&db).unwrap();
        assert_eq!(panel.today_sales, 200.0);
        assert_eq!(panel.today_profit, 100.0);

        let top = mas_vendidos(&db, 30, 10).unwrap();
        assert_eq!(top[0].total_quantity, 2);
        assert_eq!(top[0].total_revenue, 200.0);

        let cajeros = por_cajero(&db, 30).unwrap();
        assert_eq!(cajeros[0].total_sales, 200.0);
        assert_eq!(cajeros[0].gross_profit, 100.0);
    }

    #[test]
    fn una_venta_devuelta_entera_no_queda_en_nada() {
        let db = tienda();
        let venta = cobrar(&db, 2, vec![("cash", 200.0)]);
        devolver(&db, venta, 2, "cash");

        let d = hoy(&db);
        assert_eq!(d.total_sales, 0.0);
        assert_eq!(d.total_cash, 0.0, "entró y salió el mismo día");
        assert!(mas_vendidos(&db, 30, 10).unwrap().is_empty());
    }

    #[test]
    fn un_reembolso_sale_el_dia_que_sale_y_por_donde_sale() {
        // Pagó con tarjeta hace dos días; hoy se le regresa en efectivo.
        let db = tienda();
        let venta = cobrar(&db, 1, vec![("card", 100.0)]);
        db.execute("UPDATE sales SET created_at = datetime('now','localtime','-2 days')", []).unwrap();
        devolver(&db, venta, 1, "cash");

        let reporte = reporte_diario(&db, 30).unwrap();
        let entonces = reporte.iter().find(|d| d.date == hace(&db, 2)).unwrap();
        assert_eq!(entonces.total_card, 100.0, "la tarjeta sí se cobró ese día");
        assert_eq!(hoy(&db).total_cash, -100.0, "el efectivo salió hoy");
    }

    #[test]
    fn los_abonos_cuentan_el_dia_que_entran_y_la_entrega_no_los_repite() {
        // Antes los abonos no aparecían hasta la entrega, y ese día aparecía de
        // golpe todo el dinero de semanas.
        let db = tienda();
        let ap = registrar_apartado(&db, 1, CreateLayawayDto {
            customer_id: None, notes: None, due_date: None,
            initial_payment: 150.0, payment_method: "cash".to_string(),
            items: vec![CreateLayawayItemDto { product_id: 1, quantity: 3, unit_price: 0.0, variant_id: None }],
        }).unwrap();
        db.execute("UPDATE layaway_payments SET created_at = datetime('now','localtime','-3 days')", []).unwrap();
        abonar_apartado(&db, 1, ap.id, 150.0, "card").unwrap();
        entregar_apartado(&db, ap.id).unwrap();

        let reporte = reporte_diario(&db, 30).unwrap();
        let dia_del_anticipo = reporte.iter().find(|d| d.date == hace(&db, 3))
            .expect("un día con solo un abono también es un día del reporte");
        assert_eq!(dia_del_anticipo.total_cash, 150.0);
        assert_eq!(dia_del_anticipo.total_sales, 0.0, "la venta se reconoce al entregar");

        let d = hoy(&db);
        assert_eq!(d.total_sales, 300.0, "entregado hoy");
        assert_eq!(d.total_card, 150.0, "solo el abono de hoy");
        assert_eq!(d.total_cash, 0.0, "el anticipo no se vuelve a contar");
    }

    #[test]
    fn un_dia_con_solo_gastos_aparece() {
        let db = tienda();
        db.execute(
            "INSERT INTO expenses (cash_register_id, category, description, amount, user_id, created_at)
             VALUES (1, 'otros', 'Bolsas', 80.0, 1, datetime('now','localtime','-1 days'))",
            [],
        ).unwrap();

        let reporte = reporte_diario(&db, 30).unwrap();
        let ayer = reporte.iter().find(|d| d.date == hace(&db, 1)).unwrap();
        assert_eq!(ayer.total_expenses, 80.0);
    }

    #[test]
    fn el_reporte_va_del_dia_mas_reciente_al_mas_viejo() {
        let db = tienda();
        cobrar(&db, 1, vec![("cash", 100.0)]);
        cobrar(&db, 1, vec![("cash", 100.0)]);
        db.execute("UPDATE sales SET created_at = datetime('now','localtime','-5 days') WHERE id = 1", []).unwrap();

        let fechas: Vec<String> = reporte_diario(&db, 30).unwrap().into_iter().map(|d| d.date).collect();
        let mut ordenadas = fechas.clone();
        ordenadas.sort_by(|a, b| b.cmp(a));
        assert_eq!(fechas, ordenadas);
    }
}
