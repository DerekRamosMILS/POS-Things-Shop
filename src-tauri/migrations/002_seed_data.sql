-- Seed Data: Default configuration and admin user
-- Admin password: admin123 (Argon2 hash generated at runtime on first launch)

-- System configuration defaults
INSERT OR IGNORE INTO system_config (key, value, description) VALUES
    ('store_name', 'Things Shop', 'Nombre de la tienda'),
    ('store_address', '', 'Dirección de la tienda'),
    ('store_phone', '', 'Teléfono de la tienda'),
    ('store_email', '', 'Email de la tienda'),
    ('ticket_footer', '¡Gracias por su compra!', 'Mensaje al pie del ticket'),
    ('tax_rate', '0', 'Tasa de impuesto (0-100)'),
    ('currency_symbol', '$', 'Símbolo de moneda'),
    ('low_stock_threshold', '5', 'Umbral de stock bajo por defecto'),
    ('auto_backup', '1', 'Backup automático al cerrar'),
    ('max_backups', '30', 'Máximo de backups a mantener');

-- Default categories
INSERT OR IGNORE INTO categories (name, description) VALUES
    ('General', 'Categoría general'),
    ('Ropa', 'Prendas de vestir'),
    ('Accesorios', 'Accesorios y complementos'),
    ('Calzado', 'Zapatos y calzado'),
    ('Electrónicos', 'Dispositivos electrónicos');
