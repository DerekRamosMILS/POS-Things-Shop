-- Things Shop POS - El historial de precios también guarda el costo
-- Version: 027
--
-- El precio de venta dejaba rastro de cada cambio y el costo no: registrar una
-- compra a otro precio lo reescribía en silencio, y con él la utilidad de las
-- ventas viejas que no guardaron su propio costo.
ALTER TABLE price_history ADD COLUMN tipo TEXT NOT NULL DEFAULT 'venta'
    CHECK (tipo IN ('venta', 'costo'));
