use rusqlite::params;
use tauri::State;

use crate::commands::cash_register::open_register_id;
use crate::db::connection::DbState;
use crate::models::expense::{CreateExpenseDto, Expense};
use crate::session::{require_admin, require_auth, SessionState};

/// Columnas enumeradas a propósito: `e.*` se rompe en silencio en cuanto una
/// migración agrega una columna a la tabla.
const SEL: &str = "SELECT e.id, e.cash_register_id, e.category, e.description, e.amount,
    e.user_id, e.created_at, u.full_name as user_name
    FROM expenses e LEFT JOIN users u ON e.user_id = u.id";

#[tauri::command]
pub fn create_expense(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    cash_register_id: Option<i64>,
    data: CreateExpenseDto,
) -> Result<Expense, String> {
    let user_id = require_auth(&sessions, &token)?;
    let db = state.conn();
    registrar_gasto(&db, user_id, cash_register_id, data)
}

/// Núcleo del gasto, con la conexión explícita.
///
/// Un gasto sale del cajón, así que se apunta al turno abierto igual que un
/// abono en efectivo o una devolución. Antes se guardaba con la caja que
/// mandara el cliente: con la caja cerrada llegaba en blanco y el gasto no
/// entraba al corte, y el dinero faltaba en el cajón sin explicación.
pub fn registrar_gasto(
    db: &rusqlite::Connection,
    user_id: i64,
    cash_register_id: Option<i64>,
    data: CreateExpenseDto,
) -> Result<Expense, String> {
    if !data.amount.is_finite() || data.amount <= 0.0 {
        return Err("El gasto tiene que ser mayor a cero".to_string());
    }
    if data.description.trim().is_empty() {
        return Err("Escribe en qué se gastó".to_string());
    }

    let Some(register_id) = open_register_id(db) else {
        return Err("Abre la caja antes de registrar un gasto.".to_string());
    };
    if let Some(cr) = cash_register_id {
        if cr != register_id {
            return Err("La caja indicada no coincide con la caja abierta.".to_string());
        }
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;
    let resultado = (|| -> Result<(), String> {
        db.execute(
            "INSERT INTO expenses (cash_register_id, category, description, amount, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![register_id, data.category, data.description.trim(), data.amount, user_id],
        ).map_err(|e| e.to_string())?;

        db.execute(
            "UPDATE cash_registers SET total_expenses = total_expenses + ?1 WHERE id = ?2",
            params![data.amount, register_id],
        ).map_err(|e| e.to_string())?;
        Ok(())
    })();

    if let Err(e) = resultado {
        db.execute_batch("ROLLBACK;").ok();
        return Err(e);
    }
    db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;

    let id = db.last_insert_rowid();
    db.query_row(
        &format!("{} WHERE e.id = ?1", SEL),
        params![id],
        |row| {
            Ok(Expense {
                id: row.get(0)?,
                cash_register_id: row.get(1)?,
                category: row.get(2)?,
                description: row.get(3)?,
                amount: row.get(4)?,
                user_id: row.get(5)?,
                created_at: row.get(6)?,
                user_name: row.get(7)?,
            })
        },
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_expenses(state: State<DbState>, sessions: State<SessionState>, token: String, cash_register_id: Option<i64>) -> Result<Vec<Expense>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(cr_id) = cash_register_id {
        (
            format!("{} WHERE e.cash_register_id = ?1 ORDER BY e.created_at DESC", SEL),
            vec![Box::new(cr_id) as Box<dyn rusqlite::types::ToSql>],
        )
    } else {
        (
            format!("{} ORDER BY e.created_at DESC LIMIT 200", SEL),
            vec![],
        )
    };

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.prepare(&sql).map_err(|e| e.to_string())?;
    let expenses = stmt
        .query_map(params_refs.as_slice(), |row| {
            Ok(Expense {
                id: row.get(0)?,
                cash_register_id: row.get(1)?,
                category: row.get(2)?,
                description: row.get(3)?,
                amount: row.get(4)?,
                user_id: row.get(5)?,
                created_at: row.get(6)?,
                user_name: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(expenses)
}

#[tauri::command]
pub fn update_expense(state: State<DbState>, sessions: State<SessionState>, token: String, data: Expense) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    if !data.amount.is_finite() || data.amount <= 0.0 {
        return Err("El gasto tiene que ser mayor a cero".to_string());
    }
    let db = state.conn();

    // Current amount + owning register
    let (old_amount, cr_id): (f64, Option<i64>) = db.query_row(
        "SELECT amount, cash_register_id FROM expenses WHERE id = ?1",
        params![data.id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| e.to_string())?;

    if let Some(cr_id) = cr_id {
        if register_is_closed(&db, cr_id)? {
            return Err("No se puede editar un gasto de una caja ya cerrada".to_string());
        }
    }

    db.execute(
        "UPDATE expenses SET category = ?1, description = ?2, amount = ?3 WHERE id = ?4",
        params![data.category, data.description, data.amount, data.id],
    ).map_err(|e| e.to_string())?;

    // Keep the register's expense total in sync
    if let Some(cr_id) = cr_id {
        let delta = data.amount - old_amount;
        db.execute(
            "UPDATE cash_registers SET total_expenses = total_expenses + ?1 WHERE id = ?2",
            params![delta, cr_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn delete_expense(state: State<DbState>, sessions: State<SessionState>, token: String, id: i64) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    let (amount, cr_id): (f64, Option<i64>) = db.query_row(
        "SELECT amount, cash_register_id FROM expenses WHERE id = ?1",
        params![id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|e| e.to_string())?;

    if let Some(cr_id) = cr_id {
        if register_is_closed(&db, cr_id)? {
            return Err("No se puede eliminar un gasto de una caja ya cerrada".to_string());
        }
    }

    db.execute("DELETE FROM expenses WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    if let Some(cr_id) = cr_id {
        db.execute(
            "UPDATE cash_registers SET total_expenses = total_expenses - ?1 WHERE id = ?2",
            params![amount, cr_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn register_is_closed(db: &rusqlite::Connection, cr_id: i64) -> Result<bool, String> {
    let status: String = db.query_row(
        "SELECT status FROM cash_registers WHERE id = ?1",
        params![cr_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;
    Ok(status == "closed")
}

/// Un gasto es dinero que sale del cajón, así que tiene que aparecer en el
/// corte. Estas pruebas cubren las formas de que no apareciera.
#[cfg(test)]
mod tests {
    use super::*;

    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO users (id, username, password_hash, full_name, role)
             VALUES (1, 'u', 'x', 'U', 'admin')",
            [],
        )
        .unwrap();
        db
    }

    fn abrir_caja(db: &rusqlite::Connection, fondo: f64) -> i64 {
        db.execute(
            "INSERT INTO cash_registers (user_id, opening_amount) VALUES (1, ?1)",
            params![fondo],
        )
        .unwrap();
        db.last_insert_rowid()
    }

    fn gasto(monto: f64) -> CreateExpenseDto {
        CreateExpenseDto {
            category: "Operativos".into(),
            description: "Bolsas".into(),
            amount: monto,
        }
    }

    fn total_gastos(db: &rusqlite::Connection) -> f64 {
        db.query_row("SELECT total_expenses FROM cash_registers LIMIT 1", [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn un_gasto_baja_el_efectivo_que_se_espera_en_el_cajon() {
        let db = tienda();
        let caja = abrir_caja(&db, 500.0);
        registrar_gasto(&db, 1, Some(caja), gasto(120.0)).unwrap();

        assert_eq!(total_gastos(&db), 120.0);

        let esperado: f64 = db.query_row(
            "SELECT opening_amount + total_cash_sales + total_layaway_cash
                    - total_refunds_cash - total_expenses
             FROM cash_registers WHERE id = ?1",
            params![caja], |r| r.get(0)).unwrap();
        assert_eq!(esperado, 380.0, "el corte descuenta el gasto");
    }

    #[test]
    fn sin_caja_abierta_no_se_registra_un_gasto() {
        // Antes se guardaba con la caja en blanco: el dinero salía del cajón y
        // el corte no se enteraba.
        let db = tienda();
        assert!(registrar_gasto(&db, 1, None, gasto(50.0)).is_err());

        let cuantos: i64 = db.query_row("SELECT COUNT(*) FROM expenses", [], |r| r.get(0)).unwrap();
        assert_eq!(cuantos, 0);
    }

    #[test]
    fn un_gasto_no_puede_apuntar_a_otra_caja() {
        let db = tienda();
        let abierta = abrir_caja(&db, 500.0);
        assert!(registrar_gasto(&db, 1, Some(abierta + 99), gasto(50.0)).is_err());
        assert_eq!(total_gastos(&db), 0.0);
    }

    #[test]
    fn un_gasto_negativo_o_de_cero_se_rechaza() {
        // Un gasto negativo subía el efectivo esperado y podía tapar un faltante.
        let db = tienda();
        abrir_caja(&db, 500.0);
        for monto in [-100.0, 0.0] {
            assert!(registrar_gasto(&db, 1, None, gasto(monto)).is_err(), "aceptó {}", monto);
        }
        assert_eq!(total_gastos(&db), 0.0);
    }

    #[test]
    fn un_gasto_sin_descripcion_se_rechaza() {
        let db = tienda();
        abrir_caja(&db, 500.0);
        let mut sin = gasto(50.0);
        sin.description = "   ".into();
        assert!(registrar_gasto(&db, 1, None, sin).is_err());
    }
}
