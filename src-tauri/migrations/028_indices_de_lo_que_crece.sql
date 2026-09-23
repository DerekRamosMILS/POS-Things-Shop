-- Things Shop POS - Índices para las tablas que crecen y nunca se podan
-- Version: 028
--
-- Desde la 026 el historial del negocio no se borra, así que estas tablas solo
-- crecen: una tienda de años acumula miles de renglones y las consultas que las
-- recorren sin índice se vuelven más lentas cada mes, en la computadora más lenta
-- del negocio, sin que nada avise. Se va notando como "la aplicación está pesada".
--
-- `price_history` es el caso urgente y es de cosecha propia: la utilidad de las
-- partidas viejas —las de antes de que se guardara el costo al vender— se busca
-- aquí con una subconsulta por partida, y la tabla no tenía **ningún** índice.
-- Medido sobre 4000 cambios de costo y 3000 partidas legadas, la consulta de la
-- utilidad pasa de 222 ms a 1 ms. La pantalla de Reportes carga tres de esas.
--
-- El orden de las columnas sigue el de la consulta: producto, tipo y después la
-- fecha, que es por la que ordena y recorta.
CREATE INDEX IF NOT EXISTS idx_price_history_costo
    ON price_history(product_id, tipo, created_at);

-- Los gastos del turno se listan por su caja, y la tabla tampoco tenía índice.
CREATE INDEX IF NOT EXISTS idx_expenses_register
    ON expenses(cash_register_id);

-- Las capturas rechazadas se enseñan de la más nueva a la más vieja.
CREATE INDEX IF NOT EXISTS idx_capturas_rechazadas_recibida
    ON capturas_rechazadas(recibida_en);

-- Los abonos de apartado se suman por fecha en el reporte diario, y la tabla
-- tampoco se poda. Aquí es prevención y no arreglo: medido sobre tres años de
-- abonos —casi veintidós mil renglones— la consulta tarda 0.7 ms sin índice. Se
-- agrega porque cuesta una línea y porque la regla que cuida esto es simple: nada
-- que crezca para siempre se recorre entero.
CREATE INDEX IF NOT EXISTS idx_layaway_payments_created
    ON layaway_payments(created_at);
