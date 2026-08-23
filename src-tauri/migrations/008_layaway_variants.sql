-- Things Shop POS - Variantes en apartados
-- Version: 008
ALTER TABLE layaway_items ADD COLUMN variant_id INTEGER;
ALTER TABLE layaway_items ADD COLUMN variant_label TEXT;
