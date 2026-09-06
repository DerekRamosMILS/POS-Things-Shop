import { useState } from 'react';
import {
    ESCALA_MAXIMA, ESCALA_MINIMA, ESCALA_POR_DEFECTO,
    aplicarEscala, escalaGuardada, guardarEscala,
} from '../utils/escala';

/**
 * Qué tan grandes se ven las letras y los iconos.
 *
 * El cambio se ve mientras se arrastra, sin guardar: probar un tamaño y
 * arrepentirse es lo normal, y hacerlo a ciegas —mover, guardar, mirar,
 * volver— hace que nadie lo use. Solo al soltar se recuerda.
 */
export default function TamanoSettings() {
    const [escala, setEscala] = useState(escalaGuardada);

    const previsualizar = (valor: number) => {
        setEscala(valor);
        aplicarEscala(valor);
    };

    return (
        <div className="card" style={{ padding: '22px 24px' }}>
            <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', marginBottom: 6 }}>
                Tamaño de letras e iconos
            </p>
            <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 18, lineHeight: 1.5 }}>
                Agranda o achica toda la pantalla. Sirve si la letra se ve chica de lejos o si
                quieres que quepan más productos a la vez. Es de esta computadora: no cambia
                nada de la tienda.
            </p>

            <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
                <span style={{ fontSize: 11, color: 'var(--t3)', flexShrink: 0 }}>A</span>
                <input
                    type="range"
                    min={ESCALA_MINIMA}
                    max={ESCALA_MAXIMA}
                    step={5}
                    value={escala}
                    onChange={e => previsualizar(Number(e.target.value))}
                    onMouseUp={() => guardarEscala(escala)}
                    onTouchEnd={() => guardarEscala(escala)}
                    onKeyUp={() => guardarEscala(escala)}
                    style={{ flex: 1, accentColor: 'var(--primary)', cursor: 'pointer' }}
                    aria-label="Tamaño de letras e iconos"
                />
                <span style={{ fontSize: 20, color: 'var(--t3)', flexShrink: 0 }}>A</span>
                <span style={{
                    fontSize: 13, fontWeight: 800, color: 'var(--t1)', minWidth: 52,
                    textAlign: 'right', fontVariantNumeric: 'tabular-nums',
                }}>
                    {escala}%
                </span>
            </div>

            {escala !== ESCALA_POR_DEFECTO && (
                <button
                    onClick={() => { previsualizar(ESCALA_POR_DEFECTO); guardarEscala(ESCALA_POR_DEFECTO); }}
                    className="btn btn-ghost btn-sm"
                    style={{ marginTop: 14 }}
                >
                    Volver al tamaño normal
                </button>
            )}
        </div>
    );
}
