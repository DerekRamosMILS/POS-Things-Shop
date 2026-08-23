use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Product {
    pub id: i64,
    pub sku: String,
    pub barcode: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub supplier_id: Option<i64>,
    pub supplier_name: Option<String>,
    pub purchase_price: f64,
    pub sale_price: f64,
    pub stock: i32,
    pub min_stock: i32,
    pub is_active: bool,
    pub low_stock_ignored: bool,
    pub image_url: Option<String>,
    pub has_variants: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProductDto {
    pub sku: String,
    pub barcode: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub category_id: Option<i64>,
    pub supplier_id: Option<i64>,
    pub purchase_price: f64,
    pub sale_price: f64,
    pub stock: i32,
    pub min_stock: i32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProductDto {
    pub id: i64,
    pub sku: String,
    pub barcode: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub category_id: Option<i64>,
    pub supplier_id: Option<i64>,
    pub purchase_price: f64,
    pub sale_price: f64,
    pub min_stock: i32,
    pub is_active: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct ProductFilters {
    pub search: Option<String>,
    pub category_id: Option<i64>,
    pub is_active: Option<bool>,
    pub low_stock: Option<bool>,
}
