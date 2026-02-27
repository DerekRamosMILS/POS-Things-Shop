use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::expense::{CreateExpenseDto, Expense};

#[tauri::command]
pub fn create_expense(
    state: State<DbState>,
    user_id: i64,
    cash_register_id: Option<i64>,
    data: CreateExpenseDto,
) -> Result<Expense, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    db.execute(
        "INSERT INTO expenses (cash_register_id, category, description, amount, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![cash_register_id, data.category, data.description, data.amount, user_id],
    ).map_err(|e| e.to_string())?;

    // Update cash register expense total
    if let Some(cr_id) = cash_register_id {
        db.execute(
            "UPDATE cash_registers SET total_expenses = total_expenses + ?1 WHERE id = ?2",
            params![data.amount, cr_id],
        ).map_err(|e| e.to_string())?;
    }

    let id = db.last_insert_rowid();
    db.query_row(
        "SELECT e.*, u.full_name as user_name FROM expenses e LEFT JOIN users u ON e.user_id = u.id WHERE e.id = ?1",
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
pub fn get_expenses(state: State<DbState>, cash_register_id: Option<i64>) -> Result<Vec<Expense>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(cr_id) = cash_register_id {
        (
            "SELECT e.*, u.full_name as user_name FROM expenses e
             LEFT JOIN users u ON e.user_id = u.id
             WHERE e.cash_register_id = ?1
             ORDER BY e.created_at DESC".to_string(),
            vec![Box::new(cr_id) as Box<dyn rusqlite::types::ToSql>],
        )
    } else {
        (
            "SELECT e.*, u.full_name as user_name FROM expenses e
             LEFT JOIN users u ON e.user_id = u.id
             ORDER BY e.created_at DESC LIMIT 200".to_string(),
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
pub fn update_expense(state: State<DbState>, data: Expense) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute(
        "UPDATE expenses SET category = ?1, description = ?2, amount = ?3 WHERE id = ?4",
        params![data.category, data.description, data.amount, data.id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_expense(state: State<DbState>, id: i64) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.execute("DELETE FROM expenses WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
