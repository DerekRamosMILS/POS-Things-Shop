use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::db::connection::DbState;

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
pub struct DashboardStats {
    pub today_sales: f64,
    pub today_count: i64,
    pub month_sales: f64,
    pub month_count: i64,
    pub total_products: i64,
    pub low_stock_count: i64,
    pub today_profit: f64,
}

#[tauri::command]
pub fn get_dashboard_stats(state: State<DbState>) -> Result<DashboardStats, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

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
        "SELECT COUNT(*) FROM products WHERE stock <= min_stock AND is_active = 1",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let today_profit: f64 = db.query_row(
        "SELECT COALESCE(SUM(si.subtotal - (p.purchase_price * si.quantity)), 0)
         FROM sale_items si
         JOIN sales s ON si.sale_id = s.id
         JOIN products p ON si.product_id = p.id
         WHERE date(s.created_at) = date('now','localtime') AND s.status = 'completed'",
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
pub fn get_daily_sales_report(state: State<DbState>, days: Option<i32>) -> Result<Vec<DailySalesReport>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let days = days.unwrap_or(30);

    let mut stmt = db.prepare(
        "SELECT
            date(s.created_at) as sale_date,
            COALESCE(SUM(s.total), 0) as total_sales,
            COUNT(*) as sale_count,
            COALESCE(SUM(CASE WHEN s.payment_method = 'cash' THEN s.total ELSE 0 END), 0) as total_cash,
            COALESCE(SUM(CASE WHEN s.payment_method = 'card' THEN s.total ELSE 0 END), 0) as total_card,
            COALESCE(SUM(CASE WHEN s.payment_method = 'transfer' THEN s.total ELSE 0 END), 0) as total_transfer
         FROM sales s
         WHERE s.status = 'completed'
           AND s.created_at >= datetime('now', ?1 || ' days', 'localtime')
         GROUP BY date(s.created_at)
         ORDER BY sale_date DESC"
    ).map_err(|e| e.to_string())?;

    let reports = stmt
        .query_map(params![format!("-{}", days)], |row| {
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

    Ok(reports)
}

#[tauri::command]
pub fn get_top_products(state: State<DbState>, days: Option<i32>, limit: Option<i32>) -> Result<Vec<TopProduct>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
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
