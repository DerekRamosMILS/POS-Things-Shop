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
    let db = state.conn();
    cargar_demo(&db, user_id)
}

/// Qué impide cargar datos de prueba, o `None` si la instalación está limpia.
///
/// Mirar solo las ventas no alcanzaba. Una tienda pasa días capturando su
/// catálogo antes de abrir: sin una sola venta, la guarda dejaba pasar el clic y
/// le metía diez prendas, dos proveedores y cuatro clientes inventados a su
/// catálogo de verdad. Y desde la migración `026_nada_se_borra` **eso no se
/// puede borrar**: quedan para siempre, y hay que darlas de baja una por una.
pub(crate) fn por_que_no_se_puede_sembrar(db: &rusqlite::Connection) -> Option<String> {
    let cuenta = |sql: &str| -> i64 { db.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };

    let seeded: String = db
        .query_row("SELECT value FROM system_config WHERE key = 'demo_seeded'", [], |r| r.get(0))
        .unwrap_or_default();
    if seeded == "1" {
        return Some("Los datos de prueba ya fueron cargados anteriormente".to_string());
    }

    // Las ventas de ejemplo van fechadas en días pasados y entran a los reportes
    // y a las utilidades como cualquier otra.
    let ventas = cuenta("SELECT COUNT(*) FROM sales WHERE folio NOT LIKE 'DEMO-%'");
    if ventas > 0 {
        return Some(format!(
            "Esta tienda ya tiene {} venta(s) registradas. Los datos de prueba incluyen ventas de ejemplo que se mezclarían con tus reportes, así que solo se pueden cargar en una instalación nueva.",
            ventas
        ));
    }

    // Y lo demás tampoco se puede deshacer.
    let propios: [(&str, &str, &str); 3] = [
        ("producto(s)", "products", "SELECT COUNT(*) FROM products WHERE sku NOT LIKE 'DEMO-%'"),
        ("cliente(s)", "customers", "SELECT COUNT(*) FROM customers"),
        ("proveedor(es)", "suppliers", "SELECT COUNT(*) FROM suppliers"),
    ];
    for (que, _, sql) in propios {
        let n = cuenta(sql);
        if n > 0 {
            return Some(format!(
                "Esta tienda ya tiene {} {} dados de alta. Los datos de prueba agregan productos, clientes y proveedores inventados que después **no se pueden borrar**, así que solo se cargan en una instalación nueva.",
                n, que
            ));
        }
    }

    None
}

/// Núcleo de la carga de datos de prueba, con la conexión explícita.
pub(crate) fn cargar_demo(db: &rusqlite::Connection, user_id: i64) -> Result<String, String> {
    if let Some(motivo) = por_que_no_se_puede_sembrar(db) {
        return Err(motivo);
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

            // El desglose por método sale de sale_payments; sin él, las ventas
            // de ejemplo aparecerían en el total del reporte pero en ningún
            // método, que es justo lo que se corrigió para el pago mixto.
            db.execute(
                "INSERT INTO sale_payments (sale_id, method, amount) VALUES (?1, ?2, ?3)",
                params![sale_id, method, total],
            ).map_err(|e| e.to_string())?;

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
            crate::db::connection::confirmar(db)?;
            Ok("Datos de prueba cargados: 10 productos, 4 clientes, 2 proveedores y 6 ventas".to_string())
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

    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')",
            [],
        ).unwrap();
        // Las categorías del arranque ya vienen sembradas por la 002.
        db
    }

    fn cuantos(db: &rusqlite::Connection, tabla: &str) -> i64 {
        db.query_row(&format!("SELECT COUNT(*) FROM {}", tabla), [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn en_una_instalacion_nueva_si_se_cargan() {
        let db = tienda();
        let msg = cargar_demo(&db, 1).unwrap();

        assert!(msg.contains("10 productos"), "{}", msg);
        assert_eq!(cuantos(&db, "products"), 10);
        assert_eq!(cuantos(&db, "sales"), 6);
    }

    #[test]
    fn no_se_cargan_dos_veces() {
        let db = tienda();
        cargar_demo(&db, 1).unwrap();

        let e = cargar_demo(&db, 1).unwrap_err();
        assert!(e.contains("ya fueron cargados"), "{}", e);
        assert_eq!(cuantos(&db, "products"), 10, "no se duplican");
    }

    #[test]
    fn una_tienda_con_su_catalogo_no_recibe_prendas_inventadas() {
        // El caso que se colaba: días capturando el catálogo antes de abrir, sin
        // una sola venta. La guarda solo miraba las ventas, así que un clic
        // curioso le metía diez prendas inventadas al catálogo de verdad — y
        // desde la 026 esas prendas ya no se pueden borrar.
        let db = tienda();
        db.execute(
            "INSERT INTO products (sku, name, purchase_price, sale_price, stock)
             VALUES ('TS-000001', 'Vestido de la tienda', 100, 250, 3)",
            [],
        ).unwrap();

        let e = cargar_demo(&db, 1).unwrap_err();

        assert!(e.contains("producto(s)"), "mensaje inesperado: {}", e);
        assert!(e.contains("no se pueden borrar"), "hay que decir por qué importa: {}", e);
        assert_eq!(cuantos(&db, "products"), 1, "no se agregó nada");
        assert_eq!(cuantos(&db, "suppliers"), 0);
    }

    #[test]
    fn una_tienda_con_clientes_tampoco() {
        let db = tienda();
        db.execute("INSERT INTO customers (name) VALUES ('Ana')", []).unwrap();

        let e = cargar_demo(&db, 1).unwrap_err();

        assert!(e.contains("cliente(s)"), "{}", e);
        assert_eq!(cuantos(&db, "products"), 0);
    }

    #[test]
    fn una_tienda_con_proveedores_tampoco() {
        let db = tienda();
        db.execute("INSERT INTO suppliers (name) VALUES ('Textiles')", []).unwrap();

        let e = cargar_demo(&db, 1).unwrap_err();

        assert!(e.contains("proveedor(es)"), "{}", e);
        assert_eq!(cuantos(&db, "products"), 0);
    }

    #[test]
    fn una_tienda_que_ya_vendio_tampoco() {
        let db = tienda();
        db.execute(
            "INSERT INTO sales (folio, user_id, subtotal, discount_total, tax, total,
                                payment_method, amount_paid, change_amount)
             VALUES ('V-20260101-001', 1, 100, 0, 0, 100, 'cash', 100, 0)",
            [],
        ).unwrap();

        let e = cargar_demo(&db, 1).unwrap_err();

        assert!(e.contains("venta(s)"), "{}", e);
    }
}
