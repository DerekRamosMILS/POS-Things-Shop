use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemConfig {
    pub key: String,
    pub value: String,
    pub description: Option<String>,
}
