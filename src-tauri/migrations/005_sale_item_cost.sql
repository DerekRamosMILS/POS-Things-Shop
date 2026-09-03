-- Things Shop POS - Snapshot del costo unitario en cada línea de venta
-- Version: 005
-- Permite calcular utilidad histórica estable aunque cambie el precio de compra del producto.
ALTER TABLE sale_items ADD COLUMN unit_cost REAL NOT NULL DEFAULT 0;
