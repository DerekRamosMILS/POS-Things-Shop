-- Things Shop POS - Un apartado entregado cuenta como venta
-- Version: 021
--
-- Los abonos entraban al corte de caja el día que se recibían —eso estaba
-- bien— pero el apartado nunca aparecía en el reporte de ventas ni en las
-- utilidades. Una tienda que aparta la mitad de lo que vende leía un reporte
-- que decía la mitad de lo que vendió, y la mercancía salía sin dejar rastro
-- entre las ventas.
--
-- Al entregarlo se registra la venta: es cuando la prenda deja la tienda, que
-- es el momento en que se reconoce el ingreso. No se toca ni el inventario
-- —descontado al apartar— ni el corte —cobrado abono por abono—, así que nada
-- se cuenta dos veces.
--
-- La columna enlaza la venta con su apartado: sirve para rastrearla y para que
-- entregar dos veces no pueda generar dos ventas.

ALTER TABLE sales ADD COLUMN layaway_id INTEGER REFERENCES layaways(id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_sales_layaway
    ON sales(layaway_id) WHERE layaway_id IS NOT NULL;
