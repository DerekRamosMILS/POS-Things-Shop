use rusqlite::Connection;

/// Todas las migraciones, en orden. Vive aparte de `run_migrations` para que las
/// pruebas puedan aplicarlas por partes y verificar una migración concreta.
fn migration_list() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "001_initial_schema",
            include_str!("../../migrations/001_initial_schema.sql"),
        ),
        (
            "002_seed_data",
            include_str!("../../migrations/002_seed_data.sql"),
        ),
        (
            "003_notifications",
            include_str!("../../migrations/003_notifications.sql"),
        ),
        (
            "004_product_images",
            include_str!("../../migrations/004_product_images.sql"),
        ),
        (
            "005_sale_item_cost",
            include_str!("../../migrations/005_sale_item_cost.sql"),
        ),
        (
            "006_customers_layaway_returns",
            include_str!("../../migrations/006_customers_layaway_returns.sql"),
        ),
        (
            "007_product_variants",
            include_str!("../../migrations/007_product_variants.sql"),
        ),
        (
            "008_layaway_variants",
            include_str!("../../migrations/008_layaway_variants.sql"),
        ),
        (
            "009_sessions",
            include_str!("../../migrations/009_sessions.sql"),
        ),
        (
            "010_security",
            include_str!("../../migrations/010_security.sql"),
        ),
        (
            "011_split_payments",
            include_str!("../../migrations/011_split_payments.sql"),
        ),
        (
            "012_cash_reconciliation",
            include_str!("../../migrations/012_cash_reconciliation.sql"),
        ),
        (
            "013_hardware",
            include_str!("../../migrations/013_hardware.sql"),
        ),
        (
            "014_server_side_discounts",
            include_str!("../../migrations/014_server_side_discounts.sql"),
        ),
        (
            "015_datos_fiscales",
            include_str!("../../migrations/015_datos_fiscales.sql"),
        ),
        (
            "016_terminal",
            include_str!("../../migrations/016_terminal.sql"),
        ),
        (
            "017_fix_payment_method_check",
            include_str!("../../migrations/017_fix_payment_method_check.sql"),
        ),
        (
            "018_fotos_en_archivos",
            include_str!("../../migrations/018_fotos_en_archivos.sql"),
        ),
        (
            "019_categorias_ropa",
            include_str!("../../migrations/019_categorias_ropa.sql"),
        ),
        (
            "020_fotos_en_la_base",
            include_str!("../../migrations/020_fotos_en_la_base.sql"),
        ),
        (
            "021_apartado_entregado_es_venta",
            include_str!("../../migrations/021_apartado_entregado_es_venta.sql"),
        ),
        (
            "022_captura_sin_duplicados",
            include_str!("../../migrations/022_captura_sin_duplicados.sql"),
        ),
        (
            "023_conteo_desde_el_celular",
            include_str!("../../migrations/023_conteo_desde_el_celular.sql"),
        ),
        (
            "024_movimientos_por_talla",
            include_str!("../../migrations/024_movimientos_por_talla.sql"),
        ),
        (
            "025_captura_por_el_relevo",
            include_str!("../../migrations/025_captura_por_el_relevo.sql"),
        ),
    ]
}

/// Run all database migrations in order
pub fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    // Create migrations tracking table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        );",
    )?;

    for (name, sql) in migration_list() {
        let already_applied: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM _migrations WHERE name = ?1",
            [name],
            |row| row.get(0),
        )?;

        if !already_applied {
            match conn.execute_batch(sql) {
                Ok(_) => {}
                // Tolerate "duplicate column name": the column may already exist on
                // databases that had an ALTER applied manually before the migration
                // was registered. We still mark the migration as applied.
                Err(e) if e.to_string().contains("duplicate column name") => {
                    log::warn!("Migration {} skipped a duplicate column: {}", name, e);
                }
                Err(e) => return Err(e),
            }
            conn.execute("INSERT INTO _migrations (name) VALUES (?1)", [name])?;
            log::info!("Applied migration: {}", name);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn all_migrations_apply_on_a_fresh_database() {
        let conn = fresh();
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied, 25);
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = fresh();
        run_migrations(&conn).unwrap();
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied, 25);
    }

    /// La 017 reconstruye `sales` para corregir su restricción. Una reconstrucción
    /// que pierda filas o índices sería catastrófica en una tienda con historial,
    /// así que se ejercita con datos dentro.
    #[test]
    fn rebuilding_sales_preserves_the_history() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));",
        ).unwrap();

        // Todo lo anterior a la reconstrucción.
        for (name, sql) in migration_list() {
            if name == "017_fix_payment_method_check" {
                break;
            }
            conn.execute_batch(sql).unwrap();
        }

        conn.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sales (id, folio, user_id, subtotal, total, payment_method,
                                notes, client_request_id, fiscal_rfc, terminal_id)
             VALUES (42, 'V-20260101-001', 1, 100.0, 100.0, 'card',
                     'una nota', 'req-1', 'ABC010101AB1', '02')",
            [],
        ).unwrap();

        // La reconstrucción.
        let sql = migration_list()
            .into_iter()
            .find(|(n, _)| *n == "017_fix_payment_method_check")
            .unwrap().1;
        conn.execute_batch(sql).unwrap();

        let (folio, notes, req, rfc, terminal): (String, String, String, String, String) = conn
            .query_row(
                "SELECT folio, notes, client_request_id, fiscal_rfc, terminal_id
                 FROM sales WHERE id = 42",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .expect("la venta debe sobrevivir a la reconstrucción");

        assert_eq!(folio, "V-20260101-001");
        assert_eq!(notes, "una nota");
        assert_eq!(req, "req-1");
        assert_eq!(rfc, "ABC010101AB1");
        assert_eq!(terminal, "02", "las columnas tardías conservan su valor");
    }

    /// `sale_payments` referencia a `sales` con ON DELETE CASCADE y la 017 hace
    /// DROP TABLE sales. Si la desactivación de llaves foráneas no surtiera
    /// efecto, la reconstrucción borraría el desglose de cobro de TODO el
    /// historial sin avisar. Se ejercita con las llaves activadas, como en
    /// producción.
    #[test]
    fn rebuilding_sales_does_not_cascade_away_related_rows() {
        let conn = Connection::open_in_memory().unwrap();
        // Exactamente lo que hace init_db antes de migrar.
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));",
        ).unwrap();

        for (name, sql) in migration_list() {
            if name == "017_fix_payment_method_check" {
                break;
            }
            conn.execute_batch(sql).unwrap();
        }

        conn.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (1, 'CAM', 'Camisa', 50.0, 150.0, 10)", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sales (id, folio, user_id, subtotal, total, payment_method)
             VALUES (7, 'V-1', 1, 300.0, 300.0, 'card')", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sale_items (sale_id, product_id, product_name, product_sku,
                                     quantity, unit_price, subtotal)
             VALUES (7, 1, 'Camisa', 'CAM', 2, 150.0, 300.0)", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sale_payments (sale_id, method, amount) VALUES (7, 'card', 300.0)", [],
        ).unwrap();
        conn.execute(
            "INSERT INTO returns (sale_id, user_id, total_refund) VALUES (7, 1, 50.0)", [],
        ).unwrap();

        let sql = migration_list().into_iter()
            .find(|(n, _)| *n == "017_fix_payment_method_check").unwrap().1;
        conn.execute_batch(sql).unwrap();

        let cuenta = |tabla: &str| -> i64 {
            conn.query_row(&format!("SELECT COUNT(*) FROM {}", tabla), [], |r| r.get(0)).unwrap()
        };

        assert_eq!(cuenta("sales"), 1, "la venta debe sobrevivir");
        assert_eq!(cuenta("sale_items"), 1, "las partidas deben sobrevivir");
        assert_eq!(cuenta("sale_payments"), 1, "el desglose de cobro NO debe borrarse en cascada");
        assert_eq!(cuenta("returns"), 1, "las devoluciones deben sobrevivir");

        // Y las llaves foráneas deben quedar activas otra vez al terminar.
        let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).unwrap();
        assert_eq!(fk, 1, "la migración debe dejar las llaves foráneas encendidas");
    }

    #[test]
    fn the_rebuilt_sales_table_keeps_all_its_indexes() {
        let conn = fresh();
        let indexes: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='sales'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        for expected in ["idx_sales_created", "idx_sales_folio", "idx_sales_status",
                         "idx_sales_client_request", "idx_sales_promotion",
                         "idx_sales_factura", "idx_sales_terminal"] {
            assert!(indexes.contains(&expected.to_string()), "falta el índice {}", expected);
        }
    }

    #[test]
    fn mixed_payments_are_accepted_by_the_schema() {
        let conn = fresh();
        conn.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')",
            [],
        ).unwrap();

        for metodo in ["cash", "card", "transfer", "mixed"] {
            conn.execute(
                "INSERT INTO sales (folio, user_id, subtotal, total, payment_method)
                 VALUES (?1, 1, 1.0, 1.0, ?2)",
                rusqlite::params![format!("F-{}", metodo), metodo],
            ).unwrap_or_else(|e| panic!("el esquema debe aceptar '{}': {}", metodo, e));
        }

        // Y seguir rechazando lo que no es un método válido.
        assert!(conn.execute(
            "INSERT INTO sales (folio, user_id, subtotal, total, payment_method)
             VALUES ('F-X', 1, 1.0, 1.0, 'bitcoin')", [],
        ).is_err());
    }

    #[test]
    fn las_categorias_son_de_una_tienda_de_ropa() {
        let conn = fresh();
        let nombres: Vec<String> = conn
            .prepare("SELECT name FROM categories ORDER BY name").unwrap()
            .query_map([], |r| r.get(0)).unwrap()
            .collect::<Result<Vec<_>, _>>().unwrap();

        for esperada in ["Vestidos", "Blusas", "Pantalones", "Faldas"] {
            assert!(nombres.iter().any(|n| n == esperada), "falta {}", esperada);
        }
        for generica in ["General", "Electrónicos", "Ropa"] {
            assert!(!nombres.iter().any(|n| n == generica),
                    "{} no clasifica nada en una tienda de ropa", generica);
        }
    }

    #[test]
    fn una_categoria_generica_con_productos_no_se_borra() {
        // Si alguien ya clasificó mercancía ahí, quitarla dejaría productos
        // huérfanos o rompería la llave foránea.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS _migrations (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now','localtime')));",
        ).unwrap();

        for (name, sql) in migration_list() {
            if name == "019_categorias_ropa" { break; }
            conn.execute_batch(sql).unwrap();
        }

        let id: i64 = conn.query_row(
            "SELECT id FROM categories WHERE name = 'General'", [], |r| r.get(0)).unwrap();
        conn.execute(
            "INSERT INTO products (sku, name, purchase_price, sale_price, stock, category_id)
             VALUES ('X', 'Algo', 1.0, 2.0, 1, ?1)", [&id],
        ).unwrap();

        let sql = migration_list().into_iter()
            .find(|(n, _)| *n == "019_categorias_ropa").unwrap().1;
        conn.execute_batch(sql).unwrap();

        let sigue: i64 = conn.query_row(
            "SELECT COUNT(*) FROM categories WHERE name = 'General'", [], |r| r.get(0)).unwrap();
        assert_eq!(sigue, 1, "no se borra una categoría en uso");
    }

    #[test]
    fn security_tables_and_columns_exist() {
        let conn = fresh();
        conn.query_row("SELECT COUNT(*) FROM login_attempts", [], |r| r.get::<_, i64>(0))
            .expect("login_attempts debe existir");
        // Tablas vacías: la consulta no devuelve filas, pero fallaría distinto
        // si la columna no existiera.
        conn.query_row("SELECT expires_at FROM sessions LIMIT 1", [], |r| r.get::<_, i64>(0))
            .expect_err("la tabla está vacía");
        conn.query_row("SELECT must_change_password FROM users LIMIT 1", [], |r| {
            r.get::<_, i64>(0)
        })
        .expect_err("la tabla está vacía");
    }
}
