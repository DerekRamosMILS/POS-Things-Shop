-- Things Shop POS - Pago mixto (efectivo + tarjeta + transferencia)
-- Version: 011

CREATE TABLE IF NOT EXISTS sale_payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_id INTEGER NOT NULL,
    method TEXT NOT NULL,
    -- Monto aplicado al total (el cambio en efectivo no se incluye aquí).
    amount REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    FOREIGN KEY (sale_id) REFERENCES sales(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_sale_payments_sale ON sale_payments(sale_id);

-- Las ventas anteriores tenían un solo método: se registran como pago único.
INSERT INTO sale_payments (sale_id, method, amount)
SELECT id, payment_method, total FROM sales WHERE status = 'completed';
