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
    ];

    for (name, sql) in migrations {
        let already_applied: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM _migrations WHERE name = ?1",
            [name],
            |row| row.get(0),
        )?;

        if !already_applied {
            conn.execute_batch(sql)?;
            conn.execute("INSERT INTO _migrations (name) VALUES (?1)", [name])?;
            log::info!("Applied migration: {}", name);
        }
    }

    Ok(())
}
