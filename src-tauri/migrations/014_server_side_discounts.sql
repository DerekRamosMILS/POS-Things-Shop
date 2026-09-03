-- Things Shop POS - El descuento se calcula en el servidor
-- Version: 014
--
-- Antes el frontend mandaba `discount_total` como número y el backend lo creía.
-- Ahora manda qué promoción se aplicó y el backend recalcula el importe, así que
-- la venta queda registrada con el descuento que la promoción realmente concede.

ALTER TABLE sales ADD COLUMN promotion_id INTEGER REFERENCES promotions(id);
CREATE INDEX IF NOT EXISTS idx_sales_promotion ON sales(promotion_id);
