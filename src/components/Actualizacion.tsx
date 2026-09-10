import { useEffect } from 'react';
import { useActualizacionStore, esMomentoSeguro, INTERVALO_MS } from '../stores/useActualizacionStore';
import { useCartStore } from '../stores/useCartStore';
import { useSessionStore } from '../stores/useSessionStore';

/**
 * Busca actualizaciones y las instala en cuanto sea seguro.
 *
 * El efecto que instala se suscribe al carrito y al turno a propósito: así, en el
 * momento en que se vacía el ticket o se cierra la caja, se vuelve a evaluar y
 * entra sin que nadie haga nada. Es lo que hace que "se actualiza solo" sea
 * verdad sin arriesgar una venta a medias.
 */
export function useActualizacionAutomatica() {
    const fase = useActualizacionStore(s => s.fase);
    const buscar = useActualizacionStore(s => s.buscar);
    const instalar = useActualizacionStore(s => s.instalar);

    // Al abrir la aplicación y cada cuatro horas.
    useEffect(() => {
        let vivo = true;
        const mirar = () => { if (vivo) void buscar(); };
        mirar();
        const t = setInterval(mirar, INTERVALO_MS);
        return () => { vivo = false; clearInterval(t); };
    }, [buscar]);

    // Estas tres son las que definen si el momento es seguro; escucharlas es lo
    // que despierta la instalación cuando la tienda se desocupa.
    const items = useCartStore(s => s.items);
    const cashRegisterId = useSessionStore(s => s.cashRegisterId);
    const user = useSessionStore(s => s.user);

    useEffect(() => {
        if (fase !== 'lista') return;
        if (!esMomentoSeguro()) return;
        void instalar();
    }, [fase, items, cashRegisterId, user, instalar]);
}

/**
 * Lo que se ve mientras el instalador trabaja.
 *
 * En Windows el instalador cierra la aplicación, así que esta pantalla dura unos
 * segundos y la app vuelve sola. Sin ella, la ventana se cerraría de golpe y
 * parecería que se cayó.
 */
export function PantallaActualizando() {
    const version = useActualizacionStore(s => s.version);
    return (
        <div style={{
            height: '100%', display: 'grid', placeItems: 'center',
            background: 'var(--bg)', padding: 24,
        }}>
            <div style={{ textAlign: 'center', maxWidth: 380 }}>
                <div style={{
                    width: 28, height: 28, margin: '0 auto 18px',
                    border: '2px solid rgba(255,255,255,.2)', borderTopColor: 'var(--primary)',
                    borderRadius: '50%', animation: 'spin .7s linear infinite',
                }} />
                <p style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)', marginBottom: 8 }}>
                    Instalando la actualización{version ? ` ${version}` : ''}
                </p>
                <p style={{ fontSize: 13, color: 'var(--t3)', lineHeight: 1.5 }}>
                    La aplicación se va a cerrar y abrir sola en unos segundos.
                    No apagues la computadora.
                </p>
            </div>
        </div>
    );
}

/**
 * Aviso de que hay una actualización esperando a que la tienda se desocupe.
 *
 * Discreto a propósito: no hay nada que decidir ni nada que hacer, solo explica
 * por qué la aplicación va a reiniciarse en cuanto cierren la caja, para que
 * cuando pase no parezca que se cayó.
 */
export function AvisoActualizacion() {
    const fase = useActualizacionStore(s => s.fase);
    const version = useActualizacionStore(s => s.version);
    if (fase !== 'lista') return null;

    return (
        <div
            title={`La versión ${version} ya está descargada. Se instala en cuanto cierres la caja.`}
            className="nav-item"
            style={{
                color: 'var(--primary)', background: 'rgba(139,120,245,0.08)',
                border: '1px solid rgba(139,120,245,0.20)', cursor: 'default',
                fontSize: 11, lineHeight: 1.35, display: 'block',
            }}
        >
            Actualización {version} lista
            <span style={{ display: 'block', color: 'var(--t3)', fontWeight: 600 }}>
                Se instala al cerrar la caja
            </span>
        </div>
    );
}
