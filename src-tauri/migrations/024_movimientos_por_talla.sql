-- Things Shop POS - Los movimientos de inventario saben de qué talla son
-- Version: 024
--
-- Un conteo hecho con el teléfono es la foto de un momento, y al aplicarlo se le
-- suman los movimientos ocurridos después para traerlo al presente. Esa suma
-- miraba todos los movimientos del producto, sin distinguir talla, porque la
-- tabla no guardaba cuál era.
--
-- El resultado era que contar una talla borraba lo vendido de las otras: se
-- anotaba "talla M: 5", se vendían tres piezas de la talla G, y al llegar el
-- conteo la talla M quedaba en 2. Cada conteo de una prenda con tallas movía
-- números que nadie había contado.
--
-- Con la talla en el renglón, la suma se limita a la que se contó. De paso queda
-- el rastro que faltaba: hasta ahora, cambiar la existencia de una talla desde
-- Productos no dejaba ningún movimiento, así que el inventario tenía un hueco de
-- auditoría justo donde el propio sistema mandaba a hacer los ajustes.

ALTER TABLE inventory_movements ADD COLUMN variant_id INTEGER REFERENCES product_variants(id);

-- Los conteos se resuelven preguntando "qué se movió de esta talla después de
-- tal hora": producto, talla y fecha, en ese orden.
CREATE INDEX IF NOT EXISTS idx_inv_movements_variante
    ON inventory_movements(product_id, variant_id, created_at);
