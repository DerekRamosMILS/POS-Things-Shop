use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sale {
    pub id: i64,
    pub folio: String,
    pub user_id: i64,
    pub user_name: Option<String>,
    pub cash_register_id: Option<i64>,
    pub subtotal: f64,
    pub discount_total: f64,
    pub tax: f64,
    pub total: f64,
    pub payment_method: String,
    pub amount_paid: f64,
    pub change_amount: f64,
    pub status: String,
    pub notes: Option<String>,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub items: Option<Vec<SaleItem>>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SaleItem {
    pub id: i64,
    pub sale_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub product_sku: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub discount: f64,
    pub subtotal: f64,
    pub returned_quantity: i32,
    pub variant_id: Option<i64>,
    pub variant_label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSaleDto {
    pub items: Vec<CreateSaleItemDto>,
    pub payment_method: String,
    pub amount_paid: f64,
    /// Desglose opcional del cobro. Vacío = un solo método (`payment_method`).
    #[serde(default)]
    pub payments: Vec<PaymentSplitDto>,
    /// Lo que el cliente cree que suman los descuentos. Se acepta por
    /// compatibilidad pero se ignora: el backend recalcula el importe a partir
    /// de las líneas y de la promoción aplicada.
    #[serde(default)]
    #[allow(dead_code)]
    pub discount_total: f64,
    /// Promoción elegida en el punto de venta, si hubo alguna.
    #[serde(default)]
    pub promotion_id: Option<i64>,
    pub notes: Option<String>,
    #[serde(default)]
    pub customer_id: Option<i64>,
    /// Identificador único del intento de cobro, generado por el cliente.
    /// Si el mismo llega dos veces se devuelve la venta original en vez de
    /// duplicarla (doble clic, reintento tras un cuelgue del webview).
    #[serde(default)]
    pub client_request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSaleItemDto {
    pub product_id: i64,
    pub quantity: i32,
    // Sent by the client but ignored server-side; the authoritative price is read
    // from the products table when the sale is created.
    #[allow(dead_code)]
    pub unit_price: f64,
    pub discount: f64,
    #[serde(default)]
    pub variant_id: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SaleFilters {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub status: Option<String>,
    pub payment_method: Option<String>,
    pub user_id: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PaymentSplitDto {
    pub method: String,
    pub amount: f64,
}
