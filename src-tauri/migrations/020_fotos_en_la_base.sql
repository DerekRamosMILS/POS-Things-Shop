-- Things Shop POS - Las fotos vuelven a la base de datos
-- Version: 020
--
-- La 018 las sacó a archivos para que la base quedara chica y rápida de
-- respaldar. El costo apareció en cuanto la tienda empezó a usarla: un archivo
-- que ninguna fila nombra es basura que hay que barrer, y barrer archivos es
-- irreversible. Restaurar un respaldo dejaba sin dueño todo lo fotografiado
-- después y la limpieza se lo llevaba; el respaldo tampoco se llevaba las
-- fotos, así que una copia restaurada en otro equipo traía el catálogo entero
-- sin una sola imagen.
--
-- Dentro de la base no hay nada de eso. Una foto entra y sale con la misma
-- transacción que su producto, un respaldo la lleva siempre, y no queda ningún
-- código que borre archivos.
--
-- El precio es una base más pesada: unos 150 KB por foto. Dos mil fotos son
-- trescientos megabytes, y cada respaldo los copia. Por eso la retención de
-- respaldos baja de treinta a diez, y las fotos se guardan a resolución de
-- catálogo (1280 px) en vez de la máxima que dé el teléfono.

ALTER TABLE product_images ADD COLUMN photo BLOB;
ALTER TABLE product_images ADD COLUMN thumbnail BLOB;

-- Los archivos que ya existan se leen y se meten a la base al arrancar; hasta
-- entonces la fila conserva su nombre de archivo. Cuando esta columna vale 1,
-- los bytes ya están adentro.
ALTER TABLE product_images ADD COLUMN en_la_base INTEGER NOT NULL DEFAULT 0;

UPDATE system_config SET value = '10'
WHERE key = 'max_backups' AND CAST(value AS INTEGER) > 10;
