use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CashRegister {
    pub id: i64,
    pub user_id: i64,
    pub user_name: Option<String>,
    pub opening_amount: f64,
    pub closing_amount: Option<f64>,
    pub expected_amount: Option<f64>,
    pub difference: Option<f64>,
    pub total_sales: f64,
    pub total_cash_sales: f64,
    pub total_card_sales: f64,
    pub total_transfer_sales: f64,
    pub total_expenses: f64,
    pub sale_count: i32,
    pub status: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenRegisterDto {
    pub opening_amount: f64,
}

#[derive(Debug, Deserialize)]
pub struct CloseRegisterDto {
    pub closing_amount: f64,
}
