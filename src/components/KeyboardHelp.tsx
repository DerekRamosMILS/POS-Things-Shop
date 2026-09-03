import { useState } from 'react';

/**
 * Ayuda del punto de venta: atajos y procedimientos.
 *
 * Se abre con F1. Existe para que el mostrador se pueda operar sin soltar el
 * teclado y para que un cajero nuevo resuelva las dudas del día sin depender de
 * que alguien esté disponible para explicárselas.
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

const PROCEDURES: { title: string; steps: string[] }[] = [
    {
        title: 'Empezar el día',
        steps: [
            'Entra a Caja y elige Abrir Caja.',
            'Cuenta el dinero con el que arrancas y captúralo como fondo.',
            'Sin caja abierta no se puede cobrar: es a propósito, para que el corte cuadre.',
        ],
    },
    {
        title: 'Cobrar una venta',
        steps: [
            'Escanea los productos o búscalos con F2.',
            'Si el producto tiene tallas o colores, elige la variante.',
            'Presiona F10 para cobrar.',
            'Elige la forma de pago. Si el cliente paga con dos, usa Mixto.',
            'En efectivo captura lo que recibiste: el sistema calcula el cambio.',
        ],
    },
    {
        title: 'Cliente que paga con tarjeta y efectivo',
        steps: [
            'En la pantalla de cobro elige Mixto.',
            'Captura cuánto va en tarjeta y cuánto en transferencia.',
            'Lo que falte se cubre en efectivo; ahí se calcula el cambio.',
            'El ticket sale con el desglose y el corte reparte cada monto.',
        ],
    },
    {
        title: 'Apartado',
        steps: [
            'Arma la orden normal y presiona F9.',
            'Captura el anticipo y, si aplica, la fecha límite.',
            'El apartado descuenta el inventario desde el momento en que se crea.',
            'Cada abono imprime un comprobante con el saldo que resta.',
            'Solo se puede entregar cuando el saldo llega a cero.',
        ],
    },
    {
        title: 'Devolución',
        steps: [
            'Entra a Ventas y abre la venta original.',
            'Elige Devolver artículos y marca las cantidades.',
            'Indica cómo le regresaste el dinero: eso determina si sale del cajón.',
            'Para devolver efectivo tiene que haber una caja abierta.',
        ],
    },
    {
        title: 'Cliente que pide factura',
        steps: [
            'El cliente debe existir en Clientes con su RFC capturado.',
            'Selecciónalo en el ticket antes de cobrar.',
            'Marca "Requiere factura": los datos se copian a esa venta.',
            'El administrador exporta las ventas por facturar desde Reportes.',
        ],
    },
    {
        title: 'Cerrar el día',
        steps: [
            'Entra a Caja y elige Cerrar Caja.',
            'El sistema muestra qué debe haber en el cajón y de dónde sale cada monto.',
            'Cuenta el dinero físico y captúralo.',
            'Si hay diferencia se registra: repórtala, no la escondas.',
        ],
    },
    {
        title: 'Si algo falla',
        steps: [
            'Si el lector no responde, ve a Ajustes y recalíbralo escaneando una vez.',
            'Si la impresora no imprime, prueba con Imprimir prueba en Ajustes.',
            'Si el cajón no abre, cambia el comando al del pin 5 en Ajustes.',
            'Para cualquier otra cosa, genera el Reporte de diagnóstico y envíalo.',
        ],
    },
];

type Tab = 'atajos' | 'como';

export default function KeyboardHelp({ onClose }: { onClose: () => void }) {
    const [tab, setTab] = useState<Tab>('atajos');

    const tabStyle = (active: boolean) => ({
        flex: 1,
        padding: '8px 0',
        borderRadius: 9,
        border: 'none',
        cursor: 'pointer',
        fontFamily: 'inherit',
        fontSize: 12,
        fontWeight: 700,
        color: active ? 'var(--t1)' : 'var(--t3)',
        background: active ? 'rgba(255,255,255,0.07)' : 'transparent',
    });

    return (
        <div className="modal-overlay" onClick={onClose}>
            <div
                className="glass-modal scale-in"
                style={{ width: '100%', maxWidth: 500, padding: 26, maxHeight: '85vh', display: 'flex', flexDirection: 'column' }}
                onClick={e => e.stopPropagation()}
            >
                <h2 style={{ fontSize: 18, fontWeight: 900, color: 'var(--t1)', marginBottom: 14 }}>
                    Ayuda
                </h2>

                <div style={{ display: 'flex', gap: 4, padding: 4, borderRadius: 11, background: 'rgba(255,255,255,0.03)', marginBottom: 16 }}>
                    <button style={tabStyle(tab === 'atajos')} onClick={() => setTab('atajos')}>Atajos</button>
                    <button style={tabStyle(tab === 'como')} onClick={() => setTab('como')}>Cómo hacer…</button>
                </div>

                <div style={{ overflowY: 'auto', flex: 1, minHeight: 0 }}>
                    {tab === 'atajos' ? (
                        <>
                            <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 14 }}>
                                El lector de código de barras funciona siempre, sin importar
                                dónde esté el cursor.
                            </p>
                            <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                                {SHORTCUTS.map(s => (
                                    <div key={s.keys} style={{ display: 'flex', alignItems: 'center', gap: 14, padding: '8px 10px', borderRadius: 9, background: 'rgba(255,255,255,0.03)' }}>
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
                        </>
                    ) : (
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            {PROCEDURES.map(p => (
                                <div key={p.title}>
                                    <p style={{ fontSize: 13, fontWeight: 800, color: 'var(--t1)', marginBottom: 6 }}>{p.title}</p>
                                    <ol style={{ margin: 0, paddingLeft: 18, display: 'flex', flexDirection: 'column', gap: 4 }}>
                                        {p.steps.map(step => (
                                            <li key={step} style={{ fontSize: 12.5, color: 'var(--t2)', lineHeight: 1.5 }}>{step}</li>
                                        ))}
                                    </ol>
                                </div>
                            ))}
                        </div>
                    )}
                </div>

                <button onClick={onClose} className="btn btn-ghost" style={{ width: '100%', marginTop: 18, justifyContent: 'center', flexShrink: 0 }}>
                    Cerrar
                </button>
            </div>
        </div>
    );
}
