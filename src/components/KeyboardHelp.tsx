/**
 * Referencia de atajos del punto de venta.
 *
 * Se abre con F1 y existe para que el mostrador se pueda operar sin soltar el
 * teclado: en una fila, mover la mano al mouse cuesta segundos por venta.
 */
const SHORTCUTS: { keys: string; action: string }[] = [
    { keys: 'F1', action: 'Mostrar u ocultar esta ayuda' },
    { keys: 'F2', action: 'Ir al buscador de productos' },
    { keys: 'F3', action: 'Ir al nombre del cliente' },
    { keys: 'F4', action: 'Descuento de la línea seleccionada' },
    { keys: 'F8', action: 'Poner la orden en espera' },
    { keys: 'F9', action: 'Convertir la orden en apartado' },
    { keys: 'F10', action: 'Cobrar' },
    { keys: '↑ ↓', action: 'Seleccionar una línea del ticket' },
    { keys: '← →', action: 'Quitar o agregar una unidad' },
    { keys: 'Supr', action: 'Quitar la línea seleccionada' },
    { keys: 'Esc', action: 'Cerrar la ventana o volver al buscador' },
];

export default function KeyboardHelp({ onClose }: { onClose: () => void }) {
    return (
        <div className="modal-overlay" onClick={onClose}>
            <div
                className="glass-modal scale-in"
                style={{ width: '100%', maxWidth: 460, padding: 26 }}
                onClick={e => e.stopPropagation()}
            >
                <h2 style={{ fontSize: 18, fontWeight: 900, color: 'var(--t1)', marginBottom: 4 }}>
                    Atajos del teclado
                </h2>
                <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 18 }}>
                    El lector de código de barras funciona siempre, sin importar dónde
                    esté el cursor.
                </p>

                <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                    {SHORTCUTS.map(s => (
                        <div
                            key={s.keys}
                            style={{
                                display: 'flex', alignItems: 'center', gap: 14,
                                padding: '8px 10px', borderRadius: 9,
                                background: 'rgba(255,255,255,0.03)',
                            }}
                        >
                            <kbd style={{
                                minWidth: 54, textAlign: 'center', padding: '3px 8px',
                                borderRadius: 7, fontSize: 12, fontWeight: 700,
                                fontFamily: 'ui-monospace, monospace', color: 'var(--t1)',
                                background: 'rgba(255,255,255,0.07)',
                                border: '1px solid rgba(255,255,255,0.12)',
                            }}>{s.keys}</kbd>
                            <span style={{ fontSize: 13, color: 'var(--t2)' }}>{s.action}</span>
                        </div>
                    ))}
                </div>

                <button onClick={onClose} className="btn btn-ghost" style={{ width: '100%', marginTop: 18, justifyContent: 'center' }}>
                    Cerrar
                </button>
            </div>
        </div>
    );
}
