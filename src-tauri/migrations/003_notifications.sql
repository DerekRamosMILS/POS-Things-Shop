-- Things Shop POS - Notifications Migration
-- Version: 003

-- Add low_stock_ignored to products
ALTER TABLE products ADD COLUMN low_stock_ignored INTEGER NOT NULL DEFAULT 0;

-- Notifications table
CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER REFERENCES products(id),
    message TEXT NOT NULL,
    target_date TEXT,
    is_read INTEGER NOT NULL DEFAULT 0,
    notification_type TEXT NOT NULL CHECK(notification_type IN ('low_stock', 'restock_reminder')),
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_notifications_product ON notifications(product_id);
CREATE INDEX IF NOT EXISTS idx_notifications_type ON notifications(notification_type);
CREATE INDEX IF NOT EXISTS idx_notifications_read ON notifications(is_read);
