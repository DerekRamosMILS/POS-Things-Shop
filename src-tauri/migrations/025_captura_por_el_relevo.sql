-- Things Shop POS - La captura del celular llega por el relevo
-- Version: 025
--
-- El teléfono ya no se conecta a esta computadora: deja lo capturado en un
-- buzón en internet y el punto de venta lo recoge. Dos consecuencias en la base.
--
-- Lo que llega y no se puede dar de alta —un conteo de una prenda que se borró,
-- un producto sin nombre— sale del buzón para no tapar lo que viene detrás, pero
-- no se tira: queda aquí completo, fotos incluidas, con el motivo. Nada de lo
-- capturado se pierde en silencio.
CREATE TABLE IF NOT EXISTS capturas_rechazadas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Único: reintentar una vuelta no aparta dos veces la misma captura.
    captura_id TEXT NOT NULL UNIQUE,
    tipo TEXT NOT NULL,
    motivo TEXT NOT NULL,
    contenido TEXT NOT NULL,
    recibida_en TEXT NOT NULL DEFAULT (datetime('now','localtime'))
);

-- Lo que usaba la red local y ya no sirve. La llave privada de la autoridad
-- certificadora de la tienda no tiene por qué quedarse en la base sin uso.
DELETE FROM system_config WHERE key IN ('captura_codigo', 'captura_ca_cert', 'captura_ca_key');
