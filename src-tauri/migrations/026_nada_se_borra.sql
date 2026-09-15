-- Things Shop POS - Lo del negocio no se borra
-- Version: 026
--
-- Hasta aquí, que nada se borrara dependía de que cada comando lo hiciera bien:
-- productos, clientes y proveedores se desactivan en vez de borrarse, las
-- promociones usadas se apagan. Pero bastaba un comando nuevo con un DELETE, o
-- una llave foránea en cascada, para perder ventas o fotos sin remedio.
--
-- Desde esta versión lo garantiza la base misma, pase por donde pase:
--
-- 1. Ventas, productos, inventario, caja, apartados, devoluciones, clientes y
--    usuarios **no se pueden borrar**. El intento aborta la operación entera.
-- 2. Fotos y gastos sí se pueden quitar desde la app —una foto movida, un gasto
--    tecleado dos veces—, pero antes de desaparecer se copian completos a su
--    archivo. No se pierden: dejan de verse.
--
-- Una migración futura que reconstruya alguna de estas tablas (crear la nueva,
-- copiar, borrar la vieja, renombrar) tiene que volver a crear sus disparadores:
-- `DROP TABLE` se los lleva con la tabla.

-- ─── Lo que no se borra nunca ───────────────────────────────────────────────

CREATE TRIGGER IF NOT EXISTS no_borrar_products BEFORE DELETE ON products
BEGIN SELECT RAISE(ABORT, 'Los productos no se borran: se desactivan'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_product_variants BEFORE DELETE ON product_variants
BEGIN SELECT RAISE(ABORT, 'Las tallas no se borran: se desactivan'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_sales BEFORE DELETE ON sales
BEGIN SELECT RAISE(ABORT, 'Las ventas no se borran: se cancelan'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_sale_items BEFORE DELETE ON sale_items
BEGIN SELECT RAISE(ABORT, 'Los renglones de una venta no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_sale_payments BEFORE DELETE ON sale_payments
BEGIN SELECT RAISE(ABORT, 'Los pagos de una venta no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_returns BEFORE DELETE ON returns
BEGIN SELECT RAISE(ABORT, 'Las devoluciones no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_return_items BEFORE DELETE ON return_items
BEGIN SELECT RAISE(ABORT, 'Los renglones de una devolución no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_layaways BEFORE DELETE ON layaways
BEGIN SELECT RAISE(ABORT, 'Los apartados no se borran: se cancelan'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_layaway_items BEFORE DELETE ON layaway_items
BEGIN SELECT RAISE(ABORT, 'Los renglones de un apartado no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_layaway_payments BEFORE DELETE ON layaway_payments
BEGIN SELECT RAISE(ABORT, 'Los abonos no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_cash_registers BEFORE DELETE ON cash_registers
BEGIN SELECT RAISE(ABORT, 'Los turnos de caja no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_inventory_movements BEFORE DELETE ON inventory_movements
BEGIN SELECT RAISE(ABORT, 'Los movimientos de inventario no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_price_history BEFORE DELETE ON price_history
BEGIN SELECT RAISE(ABORT, 'El historial de precios no se borra'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_conteos BEFORE DELETE ON conteos
BEGIN SELECT RAISE(ABORT, 'Los conteos no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_capturas_rechazadas BEFORE DELETE ON capturas_rechazadas
BEGIN SELECT RAISE(ABORT, 'Las capturas rechazadas no se borran'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_customers BEFORE DELETE ON customers
BEGIN SELECT RAISE(ABORT, 'Los clientes no se borran: se desactivan'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_suppliers BEFORE DELETE ON suppliers
BEGIN SELECT RAISE(ABORT, 'Los proveedores no se borran: se desactivan'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_users BEFORE DELETE ON users
BEGIN SELECT RAISE(ABORT, 'Los usuarios no se borran: se desactivan'); END;

-- Una promoción que nunca se usó sí se puede quitar; una usada explica el
-- descuento de tickets que ya existen.
CREATE TRIGGER IF NOT EXISTS no_borrar_promociones_usadas BEFORE DELETE ON promotions
WHEN EXISTS (SELECT 1 FROM sales WHERE promotion_id = OLD.id)
BEGIN SELECT RAISE(ABORT, 'Una promoción usada en ventas no se borra: se desactiva'); END;

-- Igual una categoría vacía; una con productos, no.
CREATE TRIGGER IF NOT EXISTS no_borrar_categorias_con_productos BEFORE DELETE ON categories
WHEN EXISTS (SELECT 1 FROM products WHERE category_id = OLD.id)
BEGIN SELECT RAISE(ABORT, 'Una categoría con productos no se borra'); END;

-- ─── Lo que se quita de la vista pero se guarda ─────────────────────────────

CREATE TABLE IF NOT EXISTS product_images_archivo (
    id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    file_name TEXT,
    thumb_name TEXT,
    position INTEGER,
    created_at TEXT,
    photo BLOB,
    thumbnail BLOB,
    en_la_base INTEGER,
    archivada_en TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE TRIGGER IF NOT EXISTS archivar_foto BEFORE DELETE ON product_images
BEGIN
    INSERT INTO product_images_archivo
        (id, product_id, file_name, thumb_name, position, created_at, photo, thumbnail, en_la_base)
    VALUES
        (OLD.id, OLD.product_id, OLD.file_name, OLD.thumb_name, OLD.position, OLD.created_at,
         OLD.photo, OLD.thumbnail, OLD.en_la_base);
END;

CREATE TABLE IF NOT EXISTS expenses_archivo (
    id INTEGER NOT NULL,
    cash_register_id INTEGER,
    category TEXT,
    description TEXT,
    amount REAL,
    user_id INTEGER,
    created_at TEXT,
    archivado_en TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE TRIGGER IF NOT EXISTS archivar_gasto BEFORE DELETE ON expenses
BEGIN
    INSERT INTO expenses_archivo (id, cash_register_id, category, description, amount, user_id, created_at)
    VALUES (OLD.id, OLD.cash_register_id, OLD.category, OLD.description, OLD.amount, OLD.user_id, OLD.created_at);
END;

-- El archivo tampoco se borra: sería la misma pérdida con un paso más.
CREATE TRIGGER IF NOT EXISTS no_borrar_archivo_fotos BEFORE DELETE ON product_images_archivo
BEGIN SELECT RAISE(ABORT, 'El archivo de fotos no se borra'); END;

CREATE TRIGGER IF NOT EXISTS no_borrar_archivo_gastos BEFORE DELETE ON expenses_archivo
BEGIN SELECT RAISE(ABORT, 'El archivo de gastos no se borra'); END;
