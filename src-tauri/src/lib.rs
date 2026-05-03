mod commands;
mod db;
mod models;

use commands::{
    backup, cash_register, categories, config, expenses, inventory, notifications, products,
    promotions, reports, sales, suppliers, users,
};
use db::connection::{init_db, DbState};
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let conn = init_db().expect("Failed to initialize database");

    // Ensure default admin user exists
    users::ensure_admin_exists(&conn).expect("Failed to create default admin");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(DbState {
            db: Mutex::new(conn),
        })
        .invoke_handler(tauri::generate_handler![
            // Products
            products::get_products,
            products::get_product_by_barcode,
            products::create_product,
            products::update_product,
            products::delete_product,
            products::set_product_image,
            // Categories
            categories::get_categories,
            categories::create_category,
            categories::update_category,
            categories::delete_category,
            // Suppliers
            suppliers::get_suppliers,
            suppliers::create_supplier,
            suppliers::update_supplier,
            suppliers::delete_supplier,
            // Sales
            sales::create_sale,
            sales::cancel_sale,
            sales::get_sales,
            sales::get_sale_detail,
            // Inventory
            inventory::get_inventory_movements,
            inventory::adjust_stock,
            inventory::register_purchase,
            inventory::get_low_stock_products,
            // Users
            users::login,
            users::create_user,
            users::get_users,
            users::update_user,
            users::change_password,
            // Cash Register
            cash_register::open_register,
            cash_register::close_register,
            cash_register::get_open_register,
            cash_register::get_register_history,
            // Expenses
            expenses::create_expense,
            expenses::get_expenses,
            expenses::update_expense,
            expenses::delete_expense,
            // Promotions
            promotions::get_promotions,
            promotions::create_promotion,
            promotions::update_promotion,
            promotions::delete_promotion,
            // Reports
            reports::get_dashboard_stats,
            reports::get_daily_sales_report,
            reports::get_top_products,
            // Backup
            backup::create_backup,
            backup::export_database,
            backup::get_backup_list,
            // Config
            config::get_all_config,
            config::get_config,
            config::set_config,
            // Notifications
            notifications::get_notifications,
            notifications::mark_notification_read,
            notifications::create_reminder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
