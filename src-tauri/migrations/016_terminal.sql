-- Things Shop POS - Identidad de terminal y folios a prueba de colisiones
-- Version: 016
--
-- Hoy el sistema corre en un solo equipo. Registrar desde qué terminal salió
-- cada venta cuesta casi nada ahora y es imprescindible si algún día hay una
-- segunda caja: sin ese dato, dos bases no se pueden reconciliar.

ALTER TABLE sales ADD COLUMN terminal_id TEXT NOT NULL DEFAULT '01';
ALTER TABLE cash_registers ADD COLUMN terminal_id TEXT NOT NULL DEFAULT '01';

CREATE INDEX IF NOT EXISTS idx_sales_terminal ON sales(terminal_id);

INSERT OR IGNORE INTO system_config (key, value, description) VALUES
    ('terminal_id', '01', 'Identificador de esta caja. Cambiarlo solo si hay más de un equipo.');
