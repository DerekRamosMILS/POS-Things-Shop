-- Things Shop POS - Variantes de producto (talla / color)
-- Version: 007

-- Marca si un producto maneja variantes. Cuando es 1, products.stock = suma del
-- stock de sus variantes activas.
ALTER TABLE products ADD COLUMN has_variants INTEGER NOT NULL DEFAULT 0;

-- Registro de la variante vendida en cada línea de venta.
ALTER TABLE sale_items ADD COLUMN variant_id INTEGER;
ALTER TABLE sale_items ADD COLUMN variant_label TEXT;

CREATE TABLE IF NOT EXISTS product_variants (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id),
    size TEXT,
    color TEXT,
    sku TEXT UNIQUE,
    barcode TEXT UNIQUE,
    stock INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);
CREATE INDEX IF NOT EXISTS idx_variants_product ON product_variants(product_id);
CREATE INDEX IF NOT EXISTS idx_variants_barcode ON product_variants(barcode);
