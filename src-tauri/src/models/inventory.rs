use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InventoryMovement {
    pub id: i64,
    pub product_id: i64,
    pub product_name: Option<String>,
    pub movement_type: String,
    pub quantity: i32,
    pub previous_stock: i32,
    pub new_stock: i32,
    pub reference_id: Option<i64>,
    pub reason: Option<String>,
    pub user_id: Option<i64>,
    pub user_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AdjustStockDto {
    pub product_id: i64,
    pub quantity: i32,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterPurchaseDto {
    pub product_id: i64,
    pub quantity: i32,
    pub purchase_price: Option<f64>,
}
