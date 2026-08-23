use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;

/// (sku, barcode, name, category, supplier, purchase, sale, stock, min_stock)
type DemoProduct<'a> = (&'a str, &'a str, &'a str, Option<i64>, Option<i64>, f64, f64, i32, i32);
/// (folio, method, days_ago, items as (product_index, quantity))
type DemoSale<'a> = (&'a str, &'a str, i64, Vec<(usize, i32)>);
use crate::session::{require_admin, SessionState};

/// Load demo data (products, customers, suppliers and a few sales) for a store
/// that is starting out. Runs only once, guarded by a flag in system_config.
#[tauri::command]
pub fn seed_demo_data(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<String, String> {
    let user_id = require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let seeded: String = db
        .query_row("SELECT value FROM system_config WHERE key = 'demo_seeded'", [], |r| r.get(0))
        .unwrap_or_default();
    if seeded == "1" {
        return Err("Los datos de prueba ya fueron cargados anteriormente".to_string());
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let result = (|| -> Result<(), String> {
        let cat = |name: &str| -> Option<i64> {
            db.query_row("SELECT id FROM categories WHERE name = ?1", params![name], |r| r.get(0)).ok()
        };
        let ropa = cat("Ropa");
        let calzado = cat("Calzado");
        let acc = cat("Accesorios");

        // Suppliers
        db.execute(
            "INSERT INTO suppliers (name, contact_name, phone, email) VALUES ('Textiles del Norte', 'María López', '8112345678', 'ventas@textilesnorte.mx')",
            [],
        ).map_err(|e| e.to_string())?;
        let sup1 = db.last_insert_rowid();
        db.execute(
            "INSERT INTO suppliers (name, contact_name, phone, email) VALUES ('Calzado Premium', 'Juan Pérez', '8187654321', 'contacto@calzadopremium.mx')",
            [],
        ).map_err(|e| e.to_string())?;
        let sup2 = db.last_insert_rowid();

        // (sku, barcode, name, category, supplier, purchase, sale, stock, min_stock)
        let products: Vec<DemoProduct> = vec![
            ("DEMO-CAM-01", "7500000010011", "Camisa Oxford Blanca", ropa, Some(sup1), 145.0, 329.0, 20, 5),
            ("DEMO-PLA-02", "7500000010028", "Playera Básica Negra", ropa, Some(sup1), 60.0, 149.0, 40, 8),
            ("DEMO-PAN-03", "7500000010035", "Pantalón Slim Mezclilla", ropa, Some(sup1), 210.0, 489.0, 15, 4),
            ("DEMO-SUD-04", "7500000010042", "Sudadera con Capucha", ropa, Some(sup1), 240.0, 559.0, 3, 5),
            ("DEMO-VES-05", "7500000010059", "Vestido Floral", ropa, Some(sup1), 190.0, 429.0, 12, 4),
            ("DEMO-TEN-06", "7500000010066", "Tenis Urbanos", calzado, Some(sup2), 420.0, 899.0, 10, 3),
            ("DEMO-BOT-07", "7500000010073", "Botines de Piel", calzado, Some(sup2), 520.0, 1149.0, 2, 3),
            ("DEMO-GOR-08", "7500000010080", "Gorra Snapback", acc, None, 75.0, 189.0, 25, 6),
            ("DEMO-CIN-09", "7500000010097", "Cinturón de Piel", acc, None, 95.0, 239.0, 18, 5),
            ("DEMO-BUF-10", "7500000010103", "Bufanda de Lana", acc, None, 85.0, 199.0, 4, 6),
        ];

        let mut ids: Vec<i64> = Vec::with_capacity(products.len());
        for p in &products {
            db.execute(
                "INSERT INTO products (sku, barcode, name, category_id, supplier_id, purchase_price, sale_price, stock, min_stock)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8],
            ).map_err(|e| e.to_string())?;
            let pid = db.last_insert_rowid();
            ids.push(pid);
            db.execute(
                "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reason)
                 VALUES (?1, 'adjustment', ?2, 0, ?2, 'Stock inicial (demo)')",
                params![pid, p.7],
            ).map_err(|e| e.to_string())?;
        }

        // Customers
        let customers: Vec<(&str, &str, &str)> = vec![
            ("Ana García", "8110002233", "ana.garcia@example.com"),
            ("Carlos Ramírez", "8110004455", "carlos.r@example.com"),
            ("Lucía Fernández", "8110006677", "lucia.f@example.com"),
            ("Miguel Torres", "8110008899", "miguel.t@example.com"),
        ];
        for c in &customers {
            db.execute(
                "INSERT INTO customers (name, phone, email) VALUES (?1, ?2, ?3)",
                params![c.0, c.1, c.2],
            ).map_err(|e| e.to_string())?;
        }

        // (folio, method, days_ago, items[(product_index, qty)])
        let sales: Vec<DemoSale> = vec![
            ("DEMO-V-001", "cash", 0, vec![(1, 2), (7, 1)]),
            ("DEMO-V-002", "card", 1, vec![(5, 1)]),
            ("DEMO-V-003", "cash", 2, vec![(0, 1), (8, 1)]),
            ("DEMO-V-004", "transfer", 3, vec![(2, 1), (1, 1)]),
            ("DEMO-V-005", "cash", 5, vec![(4, 1)]),
            ("DEMO-V-006", "card", 7, vec![(7, 2), (9, 1)]),
        ];

        for (folio, method, days_ago, items) in &sales {
            let mut subtotal = 0.0;
            for (idx, qty) in items {
                subtotal += products[*idx].6 * (*qty as f64);
            }
            let total = subtotal;
            let offset = format!("-{} days", days_ago);

            db.execute(
                "INSERT INTO sales (folio, user_id, cash_register_id, subtotal, discount_total, tax, total, payment_method, amount_paid, change_amount, status, created_at)
                 VALUES (?1, ?2, NULL, ?3, 0, 0, ?4, ?5, ?4, 0, 'completed', datetime('now', ?6, 'localtime'))",
                params![folio, user_id, subtotal, total, method, offset],
            ).map_err(|e| e.to_string())?;
            let sale_id = db.last_insert_rowid();

            for (idx, qty) in items {
                let p = &products[*idx];
                let pid = ids[*idx];
                let line_sub = p.6 * (*qty as f64);
                db.execute(
                    "INSERT INTO sale_items (sale_id, product_id, product_name, product_sku, quantity, unit_price, discount, subtotal, unit_cost)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)",
                    params![sale_id, pid, p.2, p.0, qty, p.6, line_sub, p.5],
                ).map_err(|e| e.to_string())?;

                let cur: i32 = db.query_row("SELECT stock FROM products WHERE id = ?1", params![pid], |r| r.get(0)).map_err(|e| e.to_string())?;
                let new_stock = (cur - qty).max(0);
                db.execute("UPDATE products SET stock = ?1 WHERE id = ?2", params![new_stock, pid]).map_err(|e| e.to_string())?;
                db.execute(
                    "INSERT INTO inventory_movements (product_id, movement_type, quantity, previous_stock, new_stock, reference_id, user_id)
                     VALUES (?1, 'sale', ?2, ?3, ?4, ?5, ?6)",
                    params![pid, -qty, cur, new_stock, sale_id, user_id],
                ).map_err(|e| e.to_string())?;
            }
        }

        db.execute(
            "INSERT OR REPLACE INTO system_config (key, value, description) VALUES ('demo_seeded', '1', 'Datos de prueba cargados')",
            [],
        ).map_err(|e| e.to_string())?;

        Ok(())
    })();

    match result {
        Ok(()) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            Ok("Datos de prueba cargados: 10 productos, 4 clientes, 2 proveedores y 6 ventas".to_string())
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}
