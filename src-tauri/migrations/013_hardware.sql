-- Things Shop POS - Ajustes de hardware de mostrador
-- Version: 013
--
-- Todo lo que varía entre modelos vive aquí, para que cambiar de impresora,
-- de cajón o de lector no requiera tocar código.

INSERT OR IGNORE INTO system_config (key, value, description) VALUES
    -- Impresora de tickets
    ('printer_name', '', 'Impresora de tickets (vacío = imprimir por diálogo del sistema)'),
    ('printer_width', '32', 'Ancho del papel en caracteres: 32 para 58mm, 48 para 80mm'),
    ('printer_auto_print', '1', 'Imprimir el ticket automáticamente al cobrar'),

    -- Cajón de dinero (se conecta a la impresora, no a la computadora)
    ('drawer_kick_command', '1B 70 00 19 FA', 'Comando ESC/POS que abre el cajón (pin 2 por defecto)'),
    ('drawer_open_on_cash', '1', 'Abrir el cajón cuando la venta incluye efectivo'),

    -- Lector de código de barras
    ('scanner_enabled', '1', 'Capturar lecturas del escáner'),
    ('scanner_suffix', 'enter', 'Tecla con la que el lector termina: enter, tab o none'),
    ('scanner_prefix', '', 'Carácter que el lector antepone, si lo tiene'),
    ('scanner_max_gap_ms', '60', 'Máximo de milisegundos entre teclas de una lectura'),
    ('scanner_min_length', '4', 'Longitud mínima para considerar una lectura válida');
