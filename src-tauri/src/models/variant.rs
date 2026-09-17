use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProductVariant {
    pub id: i64,
    pub product_id: i64,
    pub size: Option<String>,
    pub color: Option<String>,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub stock: i32,
    pub is_active: bool,
}

/// One row of the variant editor. `id` is present for existing variants.
#[derive(Debug, Deserialize)]
pub struct SaveVariantDto {
    pub id: Option<i64>,
    pub size: Option<String>,
    pub color: Option<String>,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub stock: i32,
    /// La existencia que el formulario cargó al abrirse. Con ella se distingue
    /// "no la toqué" de "la cambié", y se detecta que cambió por otro lado
    /// —una venta, un conteo del celular— mientras el formulario estaba abierto.
    #[serde(default)]
    pub stock_original: Option<i32>,
}
