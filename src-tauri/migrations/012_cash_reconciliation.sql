-- Things Shop POS - Conciliación real del cajón de efectivo
-- Version: 012
--
-- Antes el corte solo contemplaba ventas y gastos, así que los abonos de
-- apartados aparecían como sobrante y las devoluciones como faltante.

ALTER TABLE cash_registers ADD COLUMN total_layaway_cash REAL NOT NULL DEFAULT 0;
ALTER TABLE cash_registers ADD COLUMN total_layaway_card REAL NOT NULL DEFAULT 0;
ALTER TABLE cash_registers ADD COLUMN total_layaway_transfer REAL NOT NULL DEFAULT 0;
ALTER TABLE cash_registers ADD COLUMN total_refunds_cash REAL NOT NULL DEFAULT 0;

-- Cada abono y cada devolución quedan ligados al turno en que ocurrieron.
ALTER TABLE layaway_payments ADD COLUMN cash_register_id INTEGER REFERENCES cash_registers(id);
ALTER TABLE returns ADD COLUMN cash_register_id INTEGER REFERENCES cash_registers(id);
ALTER TABLE returns ADD COLUMN refund_method TEXT NOT NULL DEFAULT 'cash';

CREATE INDEX IF NOT EXISTS idx_layaway_payments_register ON layaway_payments(cash_register_id);
CREATE INDEX IF NOT EXISTS idx_returns_register ON returns(cash_register_id);

-- Idempotencia: dos envíos del mismo cobro no pueden generar dos ventas.
ALTER TABLE sales ADD COLUMN client_request_id TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_sales_client_request
    ON sales(client_request_id) WHERE client_request_id IS NOT NULL;
