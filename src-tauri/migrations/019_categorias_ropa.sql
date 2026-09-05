-- Things Shop POS - Categorías propias de una tienda de ropa
-- Version: 019
--
-- Las categorías iniciales venían de una plantilla genérica: General,
-- Electrónicos, Ropa, Calzado, Accesorios. En una tienda que solo vende ropa,
-- "Ropa" no clasifica nada y "Electrónicos" nunca se usará.
--
-- Solo se retiran las que no tengan ningún producto: si alguien ya clasificó
-- mercancía, su categoría se respeta aunque venga de la plantilla.

INSERT OR IGNORE INTO categories (name, description) VALUES
    ('Vestidos',      'Vestidos de todos los cortes'),
    ('Blusas',        'Blusas y tops'),
    ('Playeras',      'Playeras y camisetas'),
    ('Camisas',       'Camisas de vestir y casuales'),
    ('Pantalones',    'Pantalones y jeans'),
    ('Faldas',        'Faldas de todos los largos'),
    ('Shorts',        'Shorts y bermudas'),
    ('Chamarras',     'Chamarras, sacos y abrigos'),
    ('Suéteres',      'Suéteres y sudaderas'),
    ('Conjuntos',     'Conjuntos de dos o más piezas'),
    ('Ropa interior', 'Ropa interior y pijamas'),
    ('Calzado',       'Zapatos, tenis y sandalias'),
    ('Accesorios',    'Bolsas, cinturones, joyería y demás');

DELETE FROM categories
WHERE name IN ('General', 'Electrónicos', 'Ropa')
  AND NOT EXISTS (SELECT 1 FROM products WHERE category_id = categories.id);
