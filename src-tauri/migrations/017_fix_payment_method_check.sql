-- Things Shop POS - El CHECK de payment_method no admitía 'mixed'
-- Version: 017
--
-- La migración 011 introdujo el pago mixto pero dejó intacta la restricción
-- original de `sales`, que solo aceptaba cash, card y transfer. Toda venta con
-- pago mixto fallaba al guardarse. SQLite no permite alterar un CHECK, así que
-- la tabla se reconstruye con la restricción correcta.

PRAGMA foreign_keys = OFF;

CREATE TABLE sales_nueva (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folio TEXT NOT NULL UNIQUE,
    user_id INTEGER NOT NULL REFERENCES users(id),
    cash_register_id INTEGER REFERENCES cash_registers(id),
    subtotal REAL NOT NULL,
    discount_total REAL NOT NULL DEFAULT 0,
    tax REAL NOT NULL DEFAULT 0,
    total REAL NOT NULL,
    payment_method TEXT NOT NULL CHECK(payment_method IN ('cash','card','transfer','mixed')),
    amount_paid REAL NOT NULL DEFAULT 0,
    change_amount REAL NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'completed' CHECK(status IN ('completed','cancelled','returned')),
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    customer_id INTEGER REFERENCES customers(id),
    client_request_id TEXT,
    promotion_id INTEGER REFERENCES promotions(id),
    requiere_factura INTEGER NOT NULL DEFAULT 0,
    fiscal_rfc TEXT,
    fiscal_razon_social TEXT,
    fiscal_regimen TEXT,
    fiscal_cp TEXT,
    fiscal_uso_cfdi TEXT,
    uuid_fiscal TEXT,
    facturada_at TEXT,
    terminal_id TEXT NOT NULL DEFAULT '01'
);

INSERT INTO sales_nueva (
    id, folio, user_id, cash_register_id, subtotal, discount_total, tax, total,
    payment_method, amount_paid, change_amount, status, notes, created_at,
    customer_id, client_request_id, promotion_id, requiere_factura,
    fiscal_rfc, fiscal_razon_social, fiscal_regimen, fiscal_cp, fiscal_uso_cfdi,
    uuid_fiscal, facturada_at, terminal_id
)
SELECT
    id, folio, user_id, cash_register_id, subtotal, discount_total, tax, total,
    payment_method, amount_paid, change_amount, status, notes, created_at,
    customer_id, client_request_id, promotion_id, requiere_factura,
    fiscal_rfc, fiscal_razon_social, fiscal_regimen, fiscal_cp, fiscal_uso_cfdi,
    uuid_fiscal, facturada_at, terminal_id
FROM sales;

DROP TABLE sales;
ALTER TABLE sales_nueva RENAME TO sales;

-- Los índices se van con la tabla vieja: hay que rehacerlos todos.
CREATE INDEX IF NOT EXISTS idx_sales_created ON sales(created_at);
CREATE INDEX IF NOT EXISTS idx_sales_folio ON sales(folio);
CREATE INDEX IF NOT EXISTS idx_sales_status ON sales(status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_sales_client_request
    ON sales(client_request_id) WHERE client_request_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sales_promotion ON sales(promotion_id);
CREATE INDEX IF NOT EXISTS idx_sales_factura ON sales(requiere_factura, uuid_fiscal);
CREATE INDEX IF NOT EXISTS idx_sales_terminal ON sales(terminal_id);

PRAGMA foreign_keys = ON;
