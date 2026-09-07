-- Things Shop POS - La captura desde el celular no puede duplicar
-- Version: 022
--
-- El teléfono ahora guarda lo que captura y lo manda cuando alcanza la caja,
-- que puede ser horas después. Entre "lo mandé" y "me contestaron" caben un
-- WiFi que se cae, una pantalla que se apaga y un dedo que vuelve a darle a
-- sincronizar. Sin nada que los ate, cada reintento daría de alta el vestido
-- otra vez.
--
-- El identificador lo pone el teléfono cuando se captura, no cuando se manda:
-- así todos los reintentos del mismo producto llevan el mismo, y el segundo
-- encuentra el primero en vez de crear otro. Es el mismo mecanismo que ya
-- protege los cobros repetidos en el punto de venta.

ALTER TABLE products ADD COLUMN captura_id TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_products_captura
    ON products(captura_id) WHERE captura_id IS NOT NULL;
