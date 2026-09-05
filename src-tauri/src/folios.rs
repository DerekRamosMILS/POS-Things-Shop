//! Folios consecutivos por día.
//!
//! Contar los renglones existentes parece lo natural y es la forma equivocada:
//! en cuanto falta una fila —una restauración parcial, una limpieza a mano— el
//! consecutivo retrocede y vuelve a emitir un folio ya usado. Como la columna
//! es única, la operación falla justo cuando alguien está en el mostrador.
//!
//! Se toma el consecutivo más alto ya emitido, y con más de una caja el folio
//! lleva además su identificador para que dos terminales no choquen.

use rusqlite::params;

use chrono::Local;

/// Series de folio que emite el sistema. Cada una tiene su tabla y su letra.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Serie {
    Ventas,
    Apartados,
}

impl Serie {
    fn letra(self) -> &'static str {
        match self {
            Serie::Ventas => "V",
            Serie::Apartados => "A",
        }
    }

    /// La consulta queda estática a propósito: el nombre de la tabla nunca se
    /// arma con formato.
    fn consulta(self) -> &'static str {
        match self {
            Serie::Ventas => {
                "SELECT COALESCE(MAX(CAST(substr(folio, ?2) AS INTEGER)), 0)
                 FROM sales WHERE folio LIKE ?1"
            }
            Serie::Apartados => {
                "SELECT COALESCE(MAX(CAST(substr(folio, ?2) AS INTEGER)), 0)
                 FROM layaways WHERE folio LIKE ?1"
            }
        }
    }
}

/// Terminal desde la que se opera. Con una sola caja siempre es "01".
pub fn terminal_id(db: &rusqlite::Connection) -> String {
    db.query_row(
        "SELECT value FROM system_config WHERE key = 'terminal_id'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
    .unwrap_or_else(|| "01".to_string())
}

/// Siguiente folio del día para una serie.
pub fn siguiente(db: &rusqlite::Connection, serie: Serie) -> Result<String, String> {
    let hoy = Local::now().format("%Y%m%d").to_string();
    let terminal = terminal_id(db);

    let prefijo = if terminal == "01" {
        format!("{}-{}-", serie.letra(), hoy)
    } else {
        format!("{}{}-{}-", serie.letra(), terminal, hoy)
    };

    // El consecutivo son los caracteres que siguen al prefijo.
    let ultimo: i64 = db
        .query_row(
            serie.consulta(),
            params![format!("{}%", prefijo), prefijo.len() as i64 + 1],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(format!("{}{:03}", prefijo, ultimo + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_migrations;

    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        run_migrations(&db).unwrap();
        db.execute(
            "INSERT OR IGNORE INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'cajera', 'x', 'Cajera', 'cashier')",
            [],
        )
        .unwrap();
        db
    }

    fn hoy() -> String {
        Local::now().format("%Y%m%d").to_string()
    }

    fn apartado(db: &rusqlite::Connection, folio: &str) {
        db.execute(
            "INSERT INTO layaways (folio, user_id, total, paid, status)
             VALUES (?1, 1, 100.0, 0.0, 'active')",
            params![folio],
        )
        .unwrap();
    }

    #[test]
    fn el_primero_del_dia_es_el_uno() {
        let db = tienda();
        assert_eq!(siguiente(&db, Serie::Apartados).unwrap(), format!("A-{}-001", hoy()));
    }

    #[test]
    fn un_apartado_borrado_no_hace_que_se_repita_el_folio() {
        // Contar renglones daba A-002 otra vez, y el folio es único: el
        // siguiente apartado fallaba al guardarse.
        let db = tienda();
        apartado(&db, &format!("A-{}-001", hoy()));
        apartado(&db, &format!("A-{}-002", hoy()));
        apartado(&db, &format!("A-{}-003", hoy()));
        db.execute("DELETE FROM layaways WHERE folio LIKE '%-002'", []).unwrap();

        assert_eq!(siguiente(&db, Serie::Apartados).unwrap(), format!("A-{}-004", hoy()));
    }

    #[test]
    fn dos_cajas_no_emiten_el_mismo_folio() {
        let db = tienda();
        apartado(&db, &format!("A-{}-001", hoy()));

        db.execute(
            "INSERT OR REPLACE INTO system_config (key, value) VALUES ('terminal_id', '02')",
            [],
        )
        .unwrap();

        // La segunda caja lleva su identificador y arranca su propia cuenta.
        assert_eq!(siguiente(&db, Serie::Apartados).unwrap(), format!("A02-{}-001", hoy()));
    }

    #[test]
    fn las_ventas_y_los_apartados_llevan_cuentas_aparte() {
        let db = tienda();
        apartado(&db, &format!("A-{}-001", hoy()));
        apartado(&db, &format!("A-{}-002", hoy()));

        assert_eq!(siguiente(&db, Serie::Ventas).unwrap(), format!("V-{}-001", hoy()));
    }
}
