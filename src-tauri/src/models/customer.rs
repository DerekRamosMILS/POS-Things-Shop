use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
    /// Datos fiscales. Opcionales: no todo cliente pide factura.
    pub rfc: Option<String>,
    pub razon_social: Option<String>,
    pub regimen_fiscal: Option<String>,
    pub cp_fiscal: Option<String>,
    pub uso_cfdi: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub total_purchases: Option<f64>,
    pub purchase_count: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct CreateCustomerDto {
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
    /// Datos fiscales. Opcionales: no todo cliente pide factura.
    pub rfc: Option<String>,
    pub razon_social: Option<String>,
    pub regimen_fiscal: Option<String>,
    pub cp_fiscal: Option<String>,
    pub uso_cfdi: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct UpdateCustomerDto {
    pub id: i64,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub notes: Option<String>,
    /// Datos fiscales. Opcionales: no todo cliente pide factura.
    pub rfc: Option<String>,
    pub razon_social: Option<String>,
    pub regimen_fiscal: Option<String>,
    pub cp_fiscal: Option<String>,
    pub uso_cfdi: Option<String>,
    pub is_active: bool,
}
