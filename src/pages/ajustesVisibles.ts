/**
 * Qué claves de `system_config` son ajustes que la tienda puede cambiar.
 *
 * La reja de Ajustes dibujaba toda clave que no fuera de hardware, con el nombre
 * técnico por etiqueta cuando no tenía una. Por ahí se colaban cosas que no son
 * ajustes sino rastro interno: `version_instalada`, que se escribe en cada
 * arranque y es lo que permite saber a distancia si la actualización llegó;
 * `ultima_copia_externa`, de la que depende el aviso de que hace mucho no sale una
 * copia del equipo; la huella del catálogo del relevo. Editarlas a mano hace
 * mentir a lo que se apoya en ellas.
 *
 * La lista de claves escondidas era una lista negra, y a una lista negra siempre
 * le falta algo: cada clave nueva que Rust escriba aparece sola en la pantalla.
 * Aquí se invierte: **solo se ve lo que tiene etiqueta.**
 */

/** Se editan en la tarjeta de hardware, no en la reja general. */
export const HARDWARE_KEYS = [
    'printer_name', 'printer_width', 'printer_auto_print',
    'drawer_kick_command', 'drawer_open_on_cash',
    'scanner_enabled', 'scanner_suffix', 'scanner_prefix',
    'scanner_max_gap_ms', 'scanner_min_length',
];

/** Lo que es un ajuste, con el nombre que se le enseña a la tienda. */
export const ETIQUETAS_DE_AJUSTES: Record<string, string> = {
    store_name: 'Nombre de la Tienda',
    store_address: 'Dirección',
    store_phone: 'Teléfono',
    store_email: 'Email',
    ticket_footer: 'Pie de Ticket',
    tax_rate: 'Tasa de Impuesto (%)',
    currency_symbol: 'Símbolo de Moneda',
    low_stock_threshold: 'Umbral de Stock Mínimo',
    auto_backup: 'Respaldo Automático al Cerrar Turno',
    max_backups: 'Máximo de Respaldos a Conservar',
    session_hours: 'Duración de la Sesión (horas)',
    log_retention_days: 'Días de Bitácora a Conservar',
    // Los datos fiscales del propio negocio, que la 015 siembra vacíos. Estaban en
    // la reja general y la lista blanca los dejó fuera sin etiqueta: la tienda se
    // quedó sin forma de capturar su propio RFC.
    rfc_emisor: 'RFC del Negocio',
    regimen_emisor: 'Régimen Fiscal del Negocio (clave del SAT)',
    cp_emisor: 'Código Postal Fiscal del Negocio',
};

/** Si la clave se dibuja en la reja general de Ajustes. */
export function esAjusteVisible(key: string): boolean {
    return !HARDWARE_KEYS.includes(key) && key in ETIQUETAS_DE_AJUSTES;
}
