-- Things Shop POS - Clientes, Apartados y Devoluciones
-- Version: 006

-- ─── Clientes ───────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS customers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    phone TEXT,
    email TEXT,
    notes TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

-- Vincular ventas a clientes
ALTER TABLE sales ADD COLUMN customer_id INTEGER REFERENCES customers(id);

-- ─── Devoluciones parciales ─────────────────────────────────────────────────
ALTER TABLE sale_items ADD COLUMN returned_quantity INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS returns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_id INTEGER NOT NULL REFERENCES sales(id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    total_refund REAL NOT NULL,
    reason TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);
CREATE INDEX IF NOT EXISTS idx_returns_sale ON returns(sale_id);

CREATE TABLE IF NOT EXISTS return_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    return_id INTEGER NOT NULL REFERENCES returns(id),
    sale_item_id INTEGER NOT NULL REFERENCES sale_items(id),
    product_id INTEGER NOT NULL REFERENCES products(id),
    quantity INTEGER NOT NULL,
    refund_amount REAL NOT NULL
);

-- ─── Apartados (layaway) ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS layaways (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folio TEXT NOT NULL UNIQUE,
    customer_id INTEGER REFERENCES customers(id),
    user_id INTEGER NOT NULL REFERENCES users(id),
    total REAL NOT NULL,
    paid REAL NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','completed','cancelled')),
    notes TEXT,
    due_date TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    completed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_layaways_status ON layaways(status);

CREATE TABLE IF NOT EXISTS layaway_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    layaway_id INTEGER NOT NULL REFERENCES layaways(id),
    product_id INTEGER NOT NULL REFERENCES products(id),
    product_name TEXT NOT NULL,
    product_sku TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    unit_price REAL NOT NULL,
    unit_cost REAL NOT NULL DEFAULT 0,
    subtotal REAL NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_layaway_items_layaway ON layaway_items(layaway_id);

CREATE TABLE IF NOT EXISTS layaway_payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    layaway_id INTEGER NOT NULL REFERENCES layaways(id),
    amount REAL NOT NULL,
    payment_method TEXT NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);
CREATE INDEX IF NOT EXISTS idx_layaway_payments_layaway ON layaway_payments(layaway_id);
