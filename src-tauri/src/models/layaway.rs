use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Layaway {
    pub id: i64,
    pub folio: String,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub user_id: i64,
    pub user_name: Option<String>,
    pub total: f64,
    pub paid: f64,
    pub status: String,
    pub notes: Option<String>,
    pub due_date: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub items: Option<Vec<LayawayItem>>,
    pub payments: Option<Vec<LayawayPayment>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LayawayItem {
    pub id: i64,
    pub layaway_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_sku: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub subtotal: f64,
    pub variant_label: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LayawayPayment {
    pub id: i64,
    pub layaway_id: i64,
    pub amount: f64,
    pub payment_method: String,
    pub user_id: i64,
    pub user_name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateLayawayDto {
    pub customer_id: Option<i64>,
    pub notes: Option<String>,
    pub due_date: Option<String>,
    pub initial_payment: f64,
    pub payment_method: String,
    pub items: Vec<CreateLayawayItemDto>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLayawayItemDto {
    pub product_id: i64,
    pub quantity: i32,
    // Ignored server-side; the price is read from the products table.
    #[allow(dead_code)]
    pub unit_price: f64,
    #[serde(default)]
    pub variant_id: Option<i64>,
}
