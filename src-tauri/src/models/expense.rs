use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Expense {
    pub id: i64,
    pub cash_register_id: Option<i64>,
    pub category: String,
    pub description: String,
    pub amount: f64,
    pub user_id: i64,
    pub user_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateExpenseDto {
    pub category: String,
    pub description: String,
    pub amount: f64,
}
