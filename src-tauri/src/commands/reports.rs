use rusqlite::params;
use serde::Serialize;
use std::collections::HashMap;
use tauri::State;

use crate::db::connection::DbState;
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

// Uses the snapshotted cost when available (unit_cost > 0), otherwise the
// product's current purchase price (legacy rows saved before the snapshot).
const PROFIT_EXPR: &str =
    "si.subtotal - COALESCE(NULLIF(si.unit_cost, 0), p.purchase_price) * si.quantity";

#[tauri::command]
pub fn get_dashboard_stats(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<DashboardStats, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    let today_sales: f64 = db.query_row(
        "SELECT COALESCE(SUM(total), 0) FROM sales WHERE date(created_at) = date('now','localtime') AND status = 'completed'",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let today_count: i64 = db.query_row(
        "SELECT COUNT(*) FROM sales WHERE date(created_at) = date('now','localtime') AND status = 'completed'",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let month_sales: f64 = db.query_row(
        "SELECT COALESCE(SUM(total), 0) FROM sales WHERE strftime('%Y-%m', created_at) = strftime('%Y-%m', 'now','localtime') AND status = 'completed'",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let month_count: i64 = db.query_row(
        "SELECT COUNT(*) FROM sales WHERE strftime('%Y-%m', created_at) = strftime('%Y-%m', 'now','localtime') AND status = 'completed'",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let total_products: i64 = db.query_row(
        "SELECT COUNT(*) FROM products WHERE is_active = 1",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let low_stock_count: i64 = db.query_row(
        "SELECT COUNT(*) FROM products WHERE stock <= min_stock AND is_active = 1 AND low_stock_ignored = 0",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let today_profit: f64 = db.query_row(
        &format!(
            "SELECT COALESCE(SUM({}), 0)
             FROM sale_items si
             JOIN sales s ON si.sale_id = s.id
             JOIN products p ON si.product_id = p.id
             WHERE date(s.created_at) = date('now','localtime') AND s.status = 'completed'",
            PROFIT_EXPR
        ),
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    Ok(DashboardStats {
        today_sales,
        today_count,
        month_sales,
        month_count,
        total_products,
        low_stock_count,
        today_profit,
    })
}

#[tauri::command]
pub fn get_daily_sales_report(state: State<DbState>, sessions: State<SessionState>, token: String, days: Option<i32>) -> Result<Vec<DailySalesReport>, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    let days = days.unwrap_or(30);
    let since = format!("-{}", days);

    // 1) Sales aggregates per day
    let mut stmt = db.prepare(
        "SELECT
            date(created_at) as sale_date,
            COALESCE(SUM(total), 0) as total_sales,
            COUNT(*) as sale_count,
            -- El desglose sale de sale_payments, no de payment_method: una venta
            -- con pago mixto reparte su importe entre varios métodos y con
            -- CASE WHEN no aparecería en ninguno.
            COALESCE((SELECT SUM(sp.amount) FROM sale_payments sp
                      JOIN sales s2 ON s2.id = sp.sale_id
                      WHERE sp.method = 'cash' AND s2.status = 'completed'
                        AND date(s2.created_at) = date(sales.created_at)), 0) as total_cash,
            COALESCE((SELECT SUM(sp.amount) FROM sale_payments sp
                      JOIN sales s2 ON s2.id = sp.sale_id
                      WHERE sp.method = 'card' AND s2.status = 'completed'
                        AND date(s2.created_at) = date(sales.created_at)), 0) as total_card,
            COALESCE((SELECT SUM(sp.amount) FROM sale_payments sp
                      JOIN sales s2 ON s2.id = sp.sale_id
                      WHERE sp.method = 'transfer' AND s2.status = 'completed'
                        AND date(s2.created_at) = date(sales.created_at)), 0) as total_transfer
         FROM sales
         WHERE status = 'completed'
           AND created_at >= datetime('now', ?1 || ' days', 'localtime')
         GROUP BY date(created_at)
         ORDER BY sale_date DESC"
    ).map_err(|e| e.to_string())?;

    let mut reports: Vec<DailySalesReport> = stmt
        .query_map(params![since], |row| {
            Ok(DailySalesReport {
                date: row.get(0)?,
                total_sales: row.get(1)?,
                sale_count: row.get(2)?,
                total_cash: row.get(3)?,
                total_card: row.get(4)?,
                total_transfer: row.get(5)?,
                total_expenses: 0.0,
                gross_profit: 0.0,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut idx: HashMap<String, usize> = HashMap::new();
    for (i, r) in reports.iter().enumerate() {
        idx.insert(r.date.clone(), i);
    }

    // 2) Gross profit per day
    let mut pstmt = db.prepare(
        &format!(
            "SELECT date(s.created_at) as d, COALESCE(SUM({}), 0)
             FROM sale_items si
             JOIN sales s ON si.sale_id = s.id
             JOIN products p ON si.product_id = p.id
             WHERE s.status = 'completed'
               AND s.created_at >= datetime('now', ?1 || ' days', 'localtime')
             GROUP BY date(s.created_at)",
            PROFIT_EXPR
        )
    ).map_err(|e| e.to_string())?;
    let profit_rows: Vec<(String, f64)> = pstmt
        .query_map(params![since], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (d, p) in profit_rows {
        if let Some(&i) = idx.get(&d) {
            reports[i].gross_profit = p;
        }
    }

    // 3) Expenses per day
    let mut estmt = db.prepare(
        "SELECT date(created_at) as d, COALESCE(SUM(amount), 0)
         FROM expenses
         WHERE created_at >= datetime('now', ?1 || ' days', 'localtime')
         GROUP BY date(created_at)"
    ).map_err(|e| e.to_string())?;
    let expense_rows: Vec<(String, f64)> = estmt
        .query_map(params![since], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (d, x) in expense_rows {
        if let Some(&i) = idx.get(&d) {
            reports[i].total_expenses = x;
        }
    }

    Ok(reports)
}

#[tauri::command]
pub fn get_top_products(state: State<DbState>, sessions: State<SessionState>, token: String, days: Option<i32>, limit: Option<i32>) -> Result<Vec<TopProduct>, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    let days = days.unwrap_or(30);
    let limit = limit.unwrap_or(10);

    let mut stmt = db.prepare(
        "SELECT si.product_id, si.product_name,
                SUM(si.quantity) as total_qty,
                SUM(si.subtotal) as total_rev
         FROM sale_items si
         JOIN sales s ON si.sale_id = s.id
         WHERE s.status = 'completed'
           AND s.created_at >= datetime('now', ?1 || ' days', 'localtime')
         GROUP BY si.product_id
         ORDER BY total_qty DESC
         LIMIT ?2"
    ).map_err(|e| e.to_string())?;

    let products = stmt
        .query_map(params![format!("-{}", days), limit], |row| {
            Ok(TopProduct {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                total_quantity: row.get(2)?,
                total_revenue: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(products)
}

#[tauri::command]
pub fn get_cashier_report(state: State<DbState>, sessions: State<SessionState>, token: String, days: Option<i32>) -> Result<Vec<CashierReport>, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    let days = days.unwrap_or(30);
    let since = format!("-{}", days);

    let mut stmt = db.prepare(
        "SELECT s.user_id, u.full_name, COUNT(*) as cnt, COALESCE(SUM(s.total), 0) as total
         FROM sales s
         JOIN users u ON s.user_id = u.id
         WHERE s.status = 'completed'
           AND s.created_at >= datetime('now', ?1 || ' days', 'localtime')
         GROUP BY s.user_id
         ORDER BY total DESC"
    ).map_err(|e| e.to_string())?;

    let mut reports: Vec<CashierReport> = stmt
        .query_map(params![since], |row| {
            Ok(CashierReport {
                user_id: row.get(0)?,
                user_name: row.get(1)?,
                sale_count: row.get(2)?,
                total_sales: row.get(3)?,
                gross_profit: 0.0,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut idx: HashMap<i64, usize> = HashMap::new();
    for (i, r) in reports.iter().enumerate() {
        idx.insert(r.user_id, i);
    }

    let mut pstmt = db.prepare(
        &format!(
            "SELECT s.user_id, COALESCE(SUM({}), 0)
             FROM sale_items si
             JOIN sales s ON si.sale_id = s.id
             JOIN products p ON si.product_id = p.id
             WHERE s.status = 'completed'
               AND s.created_at >= datetime('now', ?1 || ' days', 'localtime')
             GROUP BY s.user_id",
            PROFIT_EXPR
        )
    ).map_err(|e| e.to_string())?;
    let profit_rows: Vec<(i64, f64)> = pstmt
        .query_map(params![since], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (uid, p) in profit_rows {
        if let Some(&i) = idx.get(&uid) {
            reports[i].gross_profit = p;
        }
    }

    Ok(reports)
}

#[cfg(test)]
mod tests {
    use crate::commands::sales::registrar_venta;
    use crate::models::sale::{CreateSaleDto, CreateSaleItemDto, PaymentSplitDto};

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

    fn cobrar(db: &rusqlite::Connection, cantidad: i32, pagos: Vec<(&str, f64)>) {
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
        }).unwrap();
    }

    /// Réplica de la consulta del reporte diario, sin la capa de comandos.
    fn desglose_del_dia(db: &rusqlite::Connection) -> (f64, f64, f64, f64) {
        db.query_row(
            "SELECT
                COALESCE(SUM(total), 0),
                COALESCE((SELECT SUM(sp.amount) FROM sale_payments sp
                          JOIN sales s2 ON s2.id = sp.sale_id
                          WHERE sp.method = 'cash' AND s2.status = 'completed'
                            AND date(s2.created_at) = date(sales.created_at)), 0),
                COALESCE((SELECT SUM(sp.amount) FROM sale_payments sp
                          JOIN sales s2 ON s2.id = sp.sale_id
                          WHERE sp.method = 'card' AND s2.status = 'completed'
                            AND date(s2.created_at) = date(sales.created_at)), 0),
                COALESCE((SELECT SUM(sp.amount) FROM sale_payments sp
                          JOIN sales s2 ON s2.id = sp.sale_id
                          WHERE sp.method = 'transfer' AND s2.status = 'completed'
                            AND date(s2.created_at) = date(sales.created_at)), 0)
             FROM sales
             WHERE status = 'completed'
             GROUP BY date(created_at)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).unwrap()
    }

    #[test]
    fn una_venta_mixta_aparece_repartida_entre_sus_metodos() {
        // Con `CASE WHEN payment_method` esta venta no aparecía en ningún método
        // y el desglose no cuadraba contra el total.
        let db = tienda();
        cobrar(&db, 3, vec![("card", 200.0), ("cash", 150.0)]);

        let (total, efectivo, tarjeta, transferencia) = desglose_del_dia(&db);

        assert_eq!(total, 300.0);
        assert_eq!(tarjeta, 200.0);
        assert_eq!(efectivo, 100.0);
        assert_eq!(transferencia, 0.0);
        assert_eq!(efectivo + tarjeta + transferencia, total, "el desglose debe sumar el total");
    }

    #[test]
    fn el_desglose_suma_el_total_con_ventas_de_todo_tipo() {
        let db = tienda();
        cobrar(&db, 1, vec![("cash", 500.0)]);
        cobrar(&db, 2, vec![("card", 200.0)]);
        cobrar(&db, 1, vec![("transfer", 100.0)]);
        cobrar(&db, 4, vec![("card", 300.0), ("transfer", 50.0), ("cash", 100.0)]);

        let (total, efectivo, tarjeta, transferencia) = desglose_del_dia(&db);

        assert_eq!(total, 100.0 + 200.0 + 100.0 + 400.0);
        assert_eq!(efectivo + tarjeta + transferencia, total);
    }

    #[test]
    fn una_venta_cancelada_sale_del_desglose() {
        let db = tienda();
        cobrar(&db, 1, vec![("cash", 100.0)]);
        cobrar(&db, 2, vec![("card", 200.0)]);
        db.execute("UPDATE sales SET status = 'cancelled' WHERE id = 2", []).unwrap();

        let (total, efectivo, tarjeta, _) = desglose_del_dia(&db);

        assert_eq!(total, 100.0);
        assert_eq!(efectivo, 100.0);
        assert_eq!(tarjeta, 0.0, "lo cancelado no debe seguir contando");
    }
}
