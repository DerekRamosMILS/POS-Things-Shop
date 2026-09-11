import { useCallback, useEffect, useState } from 'react';
import * as api from '../api';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';
import { formatDateTime } from '../utils';
import type { VistaRelevo } from '../types';

/**
 * Captura de productos desde el celular.
 *
 * El teléfono y esta computadora no se hablan directo: el teléfono deja lo
 * capturado en un buzón en internet y el punto de venta lo recoge solo cada
 * pocos minutos. No hay nada que encender ni que dejar prendido, y funciona
 * aunque el celular esté fuera de la tienda o con datos móviles.
 *
 * Antes era un servidor en el WiFi de la tienda, y ahí murió: el router aísla a
 * los clientes entre sí. Hacia internet, los dos salen sin problema.
 */
export default function CaptureSettings({ compacto = false }: { compacto?: boolean } = {}) {
    const { showToast } = useToast();
    const { confirm } = useConfirm();
    const [vista, setVista] = useState<VistaRelevo | null>(null);
    const [ocupado, setOcupado] = useState(false);

    const consultar = useCallback(async () => {
        try { setVista(await api.relevoEstado()); } catch { /* la app web no lo tiene */ }
    }, []);

    // Solo lee el estado que ya tiene la aplicación; no le pregunta nada a internet.
    useEffect(() => {
        consultar();
        const t = setInterval(consultar, 10_000);
        return () => clearInterval(t);
    }, [consultar]);

    const traerAhora = async () => {
        setOcupado(true);
        try {
            const v = await api.relevoSincronizar();
            setVista(v);
            if (v.estado.ultimo_error) showToast(v.estado.ultimo_error, 'error');
            else showToast('Listo: ya está aquí todo lo del celular', 'success');
        } catch (err) { showToast(String(err), 'error'); }
        finally { setOcupado(false); }
    };

    const codigoNuevo = async () => {
        const ok = await confirm({
            title: 'Generar un código nuevo',
            message: 'Los celulares emparejados dejan de poder mandar hasta que vuelvan a apuntar la cámara al código nuevo. Antes de cambiarlo se recoge lo que ya mandaron. ¿Continuar?',
            variant: 'warning',
            confirmLabel: 'Generar',
        });
        if (!ok) return;
        setOcupado(true);
        try {
            setVista(await api.relevoRegenerar());
            showToast('Código nuevo generado');
        } catch (err) { showToast(String(err), 'error'); }
        finally { setOcupado(false); }
    };

    // En el modal de Productos el contenedor lo pone quien lo abre.
    const Envoltura = compacto
        ? ({ children }: { children: React.ReactNode }) => <div>{children}</div>
        : ({ children }: { children: React.ReactNode }) => (
            <div className="card" style={{ padding: '22px 24px' }}>{children}</div>
        );

    const estado = vista?.estado;
    const lineaDeEstado = !estado ? 'Consultando…'
        : estado.trayendo ? 'Trayendo lo del celular…'
        : estado.ultimo_error ? `No se pudo recoger: ${estado.ultimo_error}`
        : estado.ultima_vez ? `Revisado ${formatDateTime(estado.ultima_vez.replace(' ', 'T'))}. Se revisa solo cada 3 minutos.`
        : 'Todavía no se ha revisado. Se revisa solo cada 3 minutos.';

    return (
        <Envoltura>
            <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', marginBottom: 6 }}>
                Capturar productos desde el celular
            </p>
            <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 16, lineHeight: 1.5 }}>
                Toma las fotos con el teléfono y el producto se da de alta solo. Funciona sin
                internet en el celular: lo capturado se guarda ahí y se manda en cuanto haya
                señal. Esta computadora lo recoge sola mientras esté abierta.
            </p>

            <div style={{ display: 'flex', gap: 18, alignItems: 'center', flexWrap: 'wrap', marginBottom: 16 }}>
                {vista?.qr_svg && (
                    <div
                        style={{ width: 168, height: 168, borderRadius: 12, background: '#fff', padding: 10, flexShrink: 0, display: 'grid', placeItems: 'center' }}
                        dangerouslySetInnerHTML={{ __html: vista.qr_svg }}
                    />
                )}
                <div style={{ flex: 1, minWidth: 190 }}>
                    <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', marginBottom: 6 }}>
                        Apunta la cámara del celular al código
                    </p>
                    <p style={{ fontSize: 12, color: 'var(--t3)', lineHeight: 1.5 }}>
                        Se abre la captura en el navegador. Ahí mismo te ofrece instalarla en la
                        pantalla de inicio: instálala y ábrela siempre desde ahí. Es una sola vez
                        por teléfono, y no hace falta instalar ningún permiso.
                    </p>
                </div>
            </div>

            <p style={{
                fontSize: 12, marginBottom: 14, lineHeight: 1.5,
                color: estado?.ultimo_error ? 'var(--danger)' : 'var(--t3)',
            }}>
                {lineaDeEstado}
                {estado && estado.recibidas > 0 && ` Recibidos desde que se abrió: ${estado.recibidas}.`}
            </p>

            {vista && vista.rechazadas_total > 0 && (
                <div style={{ padding: '12px 14px', borderRadius: 12, marginBottom: 14, background: 'rgba(245,168,66,0.08)', border: '1px solid rgba(245,168,66,0.22)' }}>
                    <p style={{ fontSize: 12, color: 'var(--t2)', fontWeight: 700, marginBottom: 6 }}>
                        {vista.rechazadas_total === 1
                            ? '1 captura no se pudo dar de alta'
                            : `${vista.rechazadas_total} capturas no se pudieron dar de alta`}
                    </p>
                    <p style={{ fontSize: 11, color: 'var(--t3)', marginBottom: 8, lineHeight: 1.5 }}>
                        No se perdieron: están guardadas completas, fotos incluidas. Hay que
                        capturarlas otra vez corrigiendo lo que dice cada una.
                    </p>
                    {vista.rechazadas.map(r => (
                        <p key={r.captura_id} style={{ fontSize: 11, color: 'var(--t2)', lineHeight: 1.5 }}>
                            <b>{r.tipo === 'conteo' ? 'Conteo' : 'Producto'}</b> · {r.motivo}
                            <span style={{ color: 'var(--t3)' }}> · {formatDateTime(r.recibida_en.replace(' ', 'T'))}</span>
                        </p>
                    ))}
                </div>
            )}

            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                <button onClick={traerAhora} disabled={ocupado || estado?.trayendo} className="btn btn-primary btn-sm">
                    {ocupado ? 'Trayendo…' : 'Traer ahora'}
                </button>
                <button onClick={codigoNuevo} disabled={ocupado} className="btn btn-ghost btn-sm">
                    Generar código nuevo
                </button>
            </div>
        </Envoltura>
    );
}
