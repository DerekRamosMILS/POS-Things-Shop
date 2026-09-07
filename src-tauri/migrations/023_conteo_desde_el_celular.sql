-- Things Shop POS - Conteo de inventario desde el celular
-- Version: 023
--
-- Contar la mercancía es caminar la tienda con el teléfono, y la tienda sigue
-- vendiendo mientras se cuenta. Entre que alguien anota "hay doce" a las tres de
-- la tarde y que la caja recibe ese dato a las ocho, pudieron venderse tres.
--
-- Escribir doce sin más borraría esas tres ventas del inventario. El conteo es
-- una foto de un momento: se guarda a qué hora se tomó, y al aplicarlo se le
-- suman los movimientos que ocurrieron después. Doce a las tres con tres ventas
-- después son nueve, no doce.
--
-- El identificador lo pone el teléfono al contar, no al mandar, para que
-- reintentar no cuente dos veces.

CREATE TABLE IF NOT EXISTS conteos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conteo_id TEXT NOT NULL UNIQUE,
    product_id INTEGER NOT NULL REFERENCES products(id),
    variant_id INTEGER REFERENCES product_variants(id),
    -- Lo que la persona contó y cuándo lo contó.
    contado INTEGER NOT NULL,
    contado_en TEXT NOT NULL,
    -- Lo que había según el sistema y lo que quedó al aplicar el conteo.
    stock_antes INTEGER NOT NULL,
    stock_despues INTEGER NOT NULL,
    -- Movimientos ocurridos entre el conteo y su llegada, ya sumados.
    ajuste_por_movimientos INTEGER NOT NULL DEFAULT 0,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_conteos_producto ON conteos(product_id, created_at);
