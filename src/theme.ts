/**
 * Los colores del sistema, en un solo lugar.
 *
 * La paleta vive en `index.css` como variables CSS. Este objeto no las copia:
 * las referencia. Antes los mismos valores estaban escritos tres veces —aquí y
 * en dos pantallas— y cambiar el tema dejaba pedazos de la aplicación con el
 * color anterior sin que nada avisara.
 *
 * Los estilos en línea aceptan `var(--x)` igual que una hoja de estilos, así que
 * no hace falta resolverlos a mano.
 */
export const T = {
    primary: 'var(--primary)',
    primaryD: 'var(--primary-d)',
    primaryG: 'var(--primary-g)',
    accent: 'var(--accent)',
    accentG: 'var(--accent-g)',
    success: 'var(--success)',
    danger: 'var(--danger)',
    warning: 'var(--warning)',
    t1: 'var(--t1)',
    t2: 'var(--t2)',
    t3: 'var(--t3)',
} as const;
