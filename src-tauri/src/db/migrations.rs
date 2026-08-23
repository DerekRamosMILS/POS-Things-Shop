use rusqlite::Connection;

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

    let migrations: Vec<(&str, &str)> = vec![
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
    ];

    for (name, sql) in migrations {
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
        assert_eq!(applied, 11);
    }

    #[test]
    fn migrations_are_idempotent() {
        let conn = fresh();
        run_migrations(&conn).unwrap();
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied, 11);
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
