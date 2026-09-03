use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: i64,
    pub product_id: Option<i64>,
    pub message: String,
    pub target_date: Option<String>,
    pub is_read: bool,
    pub notification_type: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateReminderDto {
    pub product_id: i64,
    pub target_date: String,
}
