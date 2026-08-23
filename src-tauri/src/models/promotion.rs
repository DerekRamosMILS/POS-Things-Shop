use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Promotion {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub discount_type: String,
    pub discount_value: f64,
    pub start_date: String,
    pub end_date: String,
    pub is_active: bool,
    pub applies_to: String,
    pub target_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePromotionDto {
    pub name: String,
    pub description: Option<String>,
    pub discount_type: String,
    pub discount_value: f64,
    pub start_date: String,
    pub end_date: String,
    pub applies_to: String,
    pub target_id: Option<i64>,
}
