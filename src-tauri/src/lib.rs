mod capture;
mod commands;
mod db;
mod folios;
mod hardware;
mod logging;
mod models;
mod money;
mod photos;
mod session;

use commands::{
    backup, cash_register, categories, config, customers, diagnostics, expenses, fiscal, inventory, layaways, product_photos,
    notifications, products, promotions, reports, returns, sales, seed, suppliers, users, variants,
};
use db::connection::{init_db, purge_old_logs, DbState};
use session::SessionState;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Atiende `--restablecer-admin` antes de levantar la interfaz.
///
/// Una tienda que olvida la contraseña no tiene a quién pedirle un correo de
/// recuperación: todo vive en ese equipo. Esta es la salida, y por eso está en
/// la línea de comandos y no dentro de la aplicación, que es justo a lo que no
/// se puede entrar.
///
/// Devuelve `true` si atendió la petición y no hay que abrir la ventana.
fn atender_recuperacion() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let Some(pos) = args.iter().position(|a| a == "--restablecer-admin") else {
        return false;
    };

    let usuario = args.get(pos + 1).cloned().unwrap_or_else(|| "admin".to_string());
    let nueva = match args.get(pos + 2) {
        Some(v) => v.clone(),
        None => {
            eprintln!("Uso: things-shop --restablecer-admin <usuario> <nueva-contraseña>");
            eprintln!("Ejemplo: things-shop --restablecer-admin admin nuevaclave123");
            std::process::exit(2);
        }
    };

    let conn = match init_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("No se pudo abrir la base de datos: {}", e);
            std::process::exit(1);
        }
    };

    match commands::users::restablecer_admin(&conn, &usuario, &nueva) {
        Ok(mensaje) => {
            println!("{}", mensaje);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

pub fn run() {
    logging::init();

    if atender_recuperacion() {
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Startup can fail on a corrupt or unwritable database. Tell the
            // operator what happened instead of dying with a blank screen.
            let conn = match init_db() {
                Ok(conn) => conn,
                Err(e) => {
                    log::error!("Fallo al inicializar la base de datos: {}", e);
                    app.dialog()
                        .message(format!(
                            "No se pudo abrir la base de datos.\n\n{}\n\nRevisa la bitácora en:\n{}",
                            e,
                            logging::log_path().display()
                        ))
                        .kind(MessageDialogKind::Error)
                        .title("Things Shop POS")
                        .blocking_show();
                    std::process::exit(1);
                }
            };

            match users::ensure_admin_exists(&conn) {
                // Primer arranque: se dice con qué entrar, para no tener que
                // buscarlo en ningún lado.
                Ok(Some(clave)) => {
                    app.dialog()
                        .message(format!(
                            "Esta es la primera vez que se abre Things Shop.\n\n\
                             Usuario:  admin\n\
                             Contraseña:  {}\n\n\
                             Al entrar te va a pedir que la cambies por una tuya.",
                            clave
                        ))
                        .kind(MessageDialogKind::Info)
                        .title("Things Shop POS")
                        .blocking_show();
                }
                Ok(None) => {}
                Err(e) => {
                    log::error!("Fallo al crear el administrador inicial: {}", e);
                    app.dialog()
                        .message(format!("No se pudo crear el usuario administrador.\n\n{}", e))
                        .kind(MessageDialogKind::Error)
                        .title("Things Shop POS")
                        .blocking_show();
                    std::process::exit(1);
                }
            }

            purge_old_logs(&conn);
            // Deja constancia de si esta apertura vino de una actualización. Es
            // lo que permite saber a distancia qué versión trae la tienda.
            config::registrar_version_instalada(&conn);
            product_photos::migrar_fotos_incrustadas(&conn);
            product_photos::incorporar_fotos_en_archivos(&conn);

            // Rehydrate still-valid sessions so logins survive restarts.
            let session_map = session::load_sessions(&conn);

            let db_state = DbState::new(conn);
            let db = Arc::clone(&db_state.db);
            app.manage(db_state);
            app.manage(SessionState::with_map(session_map));

            // Lo capturado con el celular llega solo, cada pocos minutos, sin
            // nada que encender en la tienda.
            let relevo = Arc::new(capture::relevo::RelevoState::default());
            capture::relevo::arrancar(db, Arc::clone(&relevo));
            app.manage(relevo);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Products
            products::get_products,
            products::get_product,
            products::get_product_by_barcode,
            products::get_next_sku,
            products::create_product,
            products::update_product,
            products::delete_product,
            // Fotos de producto
            product_photos::add_product_image,
            product_photos::get_product_images,
            product_photos::get_product_image_list,
            product_photos::get_product_photo,
            product_photos::delete_product_image,
            product_photos::reorder_product_images,

            products::get_price_history,
            // Variants
            variants::get_variants,
            variants::get_variant_by_barcode,
            variants::save_variants,
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
            users::logout,
            users::validate_session,
            users::change_own_password,
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
            reports::get_cashier_report,
            // Backup
            backup::create_backup,
            backup::export_database,
            backup::dias_sin_copia_externa,
            backup::get_log_path,
            diagnostics::generate_diagnostic_report,
            // Datos fiscales
            fiscal::get_catalogos_fiscales,
            fiscal::exportar_pendientes_factura,
            fiscal::marcar_facturada,
            backup::get_backup_list,
            backup::restore_backup,
            // Config
            config::get_all_config,
            config::get_config,
            config::set_config,
            config::registrar_evento_actualizacion,
            // Notifications
            notifications::get_notifications,
            notifications::mark_notification_read,
            notifications::create_reminder,
            // Customers
            customers::get_customers,
            customers::create_customer,
            customers::update_customer,
            customers::delete_customer,
            // Returns
            returns::create_return,
            // Layaways
            layaways::create_layaway,
            layaways::get_layaways,
            layaways::get_layaway_detail,
            layaways::add_layaway_payment,
            layaways::complete_layaway,
            layaways::cancel_layaway,
            // Hardware de mostrador
            hardware::list_printers,
            hardware::open_cash_drawer,
            hardware::test_printer,
            hardware::print_sale_receipt,
            hardware::print_layaway_receipt,
            // Captura desde el celular
            capture::relevo::relevo_estado,
            capture::relevo::relevo_sincronizar,
            capture::relevo::relevo_regenerar,
            // Demo data
            seed::seed_demo_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
