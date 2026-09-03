import { useCallback, useEffect, useState } from 'react';
import * as api from '../api';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';
import type { CaptureStatus } from '../types';

/**
 * Captura de productos desde el celular.
 *
 * La caja levanta un servidor en la red de la tienda y el teléfono entra desde
 * su navegador: se toman las fotos, se pone el nombre y el producto queda dado
 * de alta con un código que no se repite nunca. No hay nube ni servidor externo.
 *
 * Como el puerto queda abierto para todo el WiFi mientras está encendido, la
 * pantalla insiste en apagarlo al terminar y muestra siempre que está activo.
 */
export default function CaptureSettings({ compacto = false }: { compacto?: boolean } = {}) {
    const { showToast } = useToast();
    const { confirm } = useConfirm();
    const [estado, setEstado] = useState<CaptureStatus>({
        encendido: false, url: null, codigo: null, qr_svg: null,
        interfaz: null, alternativas: [],
    });
    const [ocupado, setOcupado] = useState(false);

    const consultar = useCallback(async () => {
        try { setEstado(await api.captureServerStatus()); } catch { /* la app web no lo tiene */ }
    }, []);

    useEffect(() => { consultar(); }, [consultar]);

    const encender = async (ip?: string) => {
        setOcupado(true);
        try {
            setEstado(await api.startCaptureServer(ip));
            showToast('Listo. Apunta la cámara del celular al código.');
        } catch (err) { showToast(String(err), 'error'); }
        finally { setOcupado(false); }
    };

    /// Vuelve a levantar el servidor en otra dirección.
    ///
    /// Hace falta cuando el equipo tiene varias redes —un VPN encendido, por
    /// ejemplo— y la que se eligió sola no es la que ve el teléfono.
    const cambiarRed = async (ip: string) => {
        setOcupado(true);
        try {
            await api.stopCaptureServer();
            setEstado(await api.startCaptureServer(ip));
            showToast('Cambiado. Vuelve a apuntar la cámara al código.');
        } catch (err) { showToast(String(err), 'error'); }
        finally { setOcupado(false); }
    };

    const apagar = async () => {
        setOcupado(true);
        try {
            setEstado(await api.stopCaptureServer());
            showToast('Captura apagada');
        } catch (err) { showToast(String(err), 'error'); }
        finally { setOcupado(false); }
    };

    const reiniciarCodigo = async () => {
        const ok = await confirm({
            title: 'Generar un código nuevo',
            message: 'Los celulares que estén capturando tendrán que volver a apuntar la cámara al código nuevo. ¿Continuar?',
            confirmLabel: 'Generar',
        });
        if (!ok) return;
        setOcupado(true);
        try {
            await api.stopCaptureServer();
            setEstado(await api.startCaptureServer());
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

    return (
        <Envoltura>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, marginBottom: 6 }}>
                <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)' }}>
                    Capturar productos desde el celular
                </p>
                {estado.encendido && (
                    <span className="badge badge-success" style={{ flexShrink: 0 }}>
                        <span style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--success)', display: 'inline-block' }} />
                        Encendida
                    </span>
                )}
            </div>

            <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 16, lineHeight: 1.5 }}>
                Toma las fotos con el teléfono y el producto se da de alta solo. El
                celular tiene que estar en el mismo WiFi que esta computadora.
            </p>

            {!estado.encendido ? (
                <>
                    <button onClick={() => encender()} disabled={ocupado} className="btn btn-primary btn-sm">
                        {ocupado ? 'Encendiendo...' : 'Encender captura'}
                    </button>
                    <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 10, lineHeight: 1.5 }}>
                        Apágala cuando termines de capturar.
                    </p>
                </>
            ) : (
                <>
                    <div style={{ display: 'flex', gap: 18, alignItems: 'center', flexWrap: 'wrap', marginBottom: 16 }}>
                        {estado.qr_svg && (
                            <div
                                style={{ width: 168, height: 168, borderRadius: 12, background: '#fff', padding: 10, flexShrink: 0, display: 'grid', placeItems: 'center' }}
                                dangerouslySetInnerHTML={{ __html: estado.qr_svg }}
                            />
                        )}
                        <div style={{ flex: 1, minWidth: 190 }}>
                            <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', marginBottom: 6 }}>
                                Apunta la cámara del celular al código
                            </p>
                            <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 14, lineHeight: 1.5 }}>
                                Se abre solo. Si no, escribe esta dirección en el teléfono:
                            </p>

                            <input readOnly value={estado.url ?? ''} className="input"
                                style={{ fontFamily: 'monospace', fontSize: 11, marginBottom: 10 }}
                                onFocus={e => e.currentTarget.select()} />
                            <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 2 }}>Y este número:</p>
                            <p style={{ fontSize: 26, fontWeight: 900, letterSpacing: '0.14em', color: 'var(--t1)', fontFamily: 'ui-monospace, monospace' }}>
                                {estado.codigo}
                            </p>
                        </div>
                    </div>

                    {estado.alternativas.length > 1 && (
                        <div style={{ padding: '12px 14px', borderRadius: 12, marginBottom: 14, background: 'rgba(245,168,66,0.08)', border: '1px solid rgba(245,168,66,0.22)' }}>
                            <p style={{ fontSize: 12, color: 'var(--t2)', marginBottom: 8, lineHeight: 1.5 }}>
                                Si el celular no abre la página, prueba con otra de estas:
                            </p>
                            <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                                {estado.alternativas.map(d => {
                                    const enUso = estado.url?.includes(d.ip);
                                    return (
                                        <button
                                            key={d.ip}
                                            onClick={() => !enUso && cambiarRed(d.ip)}
                                            disabled={ocupado || enUso}
                                            className={enUso ? 'btn btn-primary btn-sm' : 'btn btn-ghost btn-sm'}
                                            style={{ fontFamily: 'ui-monospace, monospace', fontSize: 11 }}
                                        >
                                            {d.ip} <span style={{ opacity: .65, marginLeft: 4 }}>{d.interfaz}</span>
                                        </button>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                        <button onClick={apagar} disabled={ocupado} className="btn btn-danger btn-sm">
                            Apagar captura
                        </button>
                        <button onClick={reiniciarCodigo} disabled={ocupado} className="btn btn-ghost btn-sm">
                            Generar código nuevo
                        </button>
                    </div>
                </>
            )}
        </Envoltura>
    );
}
