-- Things Shop POS - Fotos como archivos, y varias por producto
-- Version: 018
--
-- Antes cada producto guardaba una sola foto incrustada en su fila como data
-- URL, reducida a 512px. Eso tenía tres problemas: el original se perdía para
-- siempre, no cabía más de una foto, y cada respaldo copiaba todas las fotos
-- junto con la base.
--
-- Ahora las fotos viven como archivos en `fotos/` dentro del directorio de
-- datos y la base solo guarda su nombre. La base queda pequeña y rápida de
-- respaldar, y las fotos pueden conservarse a resolución de catálogo.

CREATE TABLE IF NOT EXISTS product_images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    -- Nombre del archivo dentro de `fotos/`, sin ruta: la carpeta cambia según
    -- el equipo y guardar la ruta completa haría inservible un respaldo movido.
    file_name TEXT NOT NULL,
    thumb_name TEXT NOT NULL,
    -- Orden en que se muestran; la posición 0 es la principal.
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

CREATE INDEX IF NOT EXISTS idx_product_images_product ON product_images(product_id, position);

-- Las fotos que ya estaban incrustadas se migran a archivos al arrancar; la
-- columna se conserva mientras tanto para no perder nada si algo falla.
ALTER TABLE products ADD COLUMN images_migradas INTEGER NOT NULL DEFAULT 0;
