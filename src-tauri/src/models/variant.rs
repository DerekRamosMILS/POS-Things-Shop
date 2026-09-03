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
}
