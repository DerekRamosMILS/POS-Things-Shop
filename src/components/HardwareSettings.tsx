import { useCallback, useEffect, useRef, useState } from 'react';
import * as api from '../api';
import { useToast } from '../contexts/ToastContext';
import {
    configFromSample,
    createCalibrator,
    type CalibrationSample,
    type ScannerSuffix,
} from '../utils/scanner';

interface Props {
    values: Record<string, string>;
    onChange: (key: string, value: string) => void;
}

const SUFFIX_LABELS: Record<ScannerSuffix, string> = {
    enter: 'Enter (lo más común)',
    tab: 'Tabulador',
    none: 'Ninguna',
};

/**
 * Ajustes del hardware del mostrador.
 *
 * Todo aquí está pensado para funcionar sin saber de antemano qué modelo se va a
 * comprar: la impresora se elige de las instaladas en Windows, el comando del
 * cajón es editable y el lector se calibra escaneando cualquier código.
 */
export default function HardwareSettings({ values, onChange }: Props) {
    const { showToast } = useToast();
    const [printers, setPrinters] = useState<string[]>([]);
    const [loadingPrinters, setLoadingPrinters] = useState(false);
    const [testing, setTesting] = useState(false);

    const [sample, setSample] = useState<CalibrationSample | null>(null);
    const calibrationRef = useRef<HTMLInputElement>(null);
    const [calibrating, setCalibrating] = useState(false);

    const loadPrinters = useCallback(async () => {
        setLoadingPrinters(true);
        try { setPrinters(await api.listPrinters()); }
        catch (err) { showToast(String(err), 'error'); }
        finally { setLoadingPrinters(false); }
    }, [showToast]);

    useEffect(() => { loadPrinters(); }, [loadPrinters]);

    // Mientras la caja de calibración está enfocada capturamos la lectura cruda
    // para medir la velocidad real del lector y detectar su sufijo.
    useEffect(() => {
        if (!calibrating) return;
        const handler = createCalibrator((s) => { setSample(s); setCalibrating(false); });
        const node = calibrationRef.current;
        node?.addEventListener('keydown', handler);
        node?.focus();
        return () => node?.removeEventListener('keydown', handler);
    }, [calibrating]);

    const applySample = () => {
        if (!sample) return;
        const cfg = configFromSample(sample);
        onChange('scanner_enabled', '1');
        onChange('scanner_suffix', cfg.suffix);
        onChange('scanner_max_gap_ms', String(cfg.maxGapMs));
        onChange('scanner_min_length', String(cfg.minLength));
        showToast('Lector calibrado. Guarda los cambios para aplicarlo.');
    };

    const runTest = async (openDrawer: boolean) => {
        setTesting(true);
        try {
            await api.testPrinter(values.printer_name || null, openDrawer);
            showToast(openDrawer ? 'Ticket enviado y cajón accionado' : 'Ticket de prueba enviado');
        } catch (err) { showToast(String(err), 'error'); }
        finally { setTesting(false); }
    };

    const label = { fontSize: 12, color: 'var(--t3)', display: 'block', marginBottom: 6 } as const;
    const section = { padding: '22px 24px' } as const;
    const heading = { fontSize: 13, fontWeight: 700, color: 'var(--t1)', marginBottom: 4 } as const;
    const hint = { fontSize: 12, color: 'var(--t3)', marginBottom: 16, lineHeight: 1.5 } as const;

    return (
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(340px, 1fr))', gap: 14 }}>
            {/* ── Impresora y cajón ── */}
            <div className="card" style={section}>
                <p style={heading}>Impresora de tickets y cajón</p>
                <p style={hint}>
                    El cajón de dinero se conecta a la impresora con un cable telefónico,
                    no a la computadora. Por eso ambos se configuran juntos: la app le
                    manda a la impresora la orden de abrirlo.
                </p>

                <label style={label}>Impresora</label>
                <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
                    <select
                        value={values.printer_name || ''}
                        onChange={e => onChange('printer_name', e.target.value)}
                        className="input"
                        style={{ flex: 1 }}
                    >
                        <option value="">Sin impresora — usar diálogo del sistema</option>
                        {printers.map(p => <option key={p} value={p}>{p}</option>)}
                        {values.printer_name && !printers.includes(values.printer_name) && (
                            <option value={values.printer_name}>{values.printer_name} (no detectada)</option>
                        )}
                    </select>
                    <button onClick={loadPrinters} disabled={loadingPrinters} className="btn btn-ghost btn-sm">
                        {loadingPrinters ? '...' : 'Buscar'}
                    </button>
                </div>

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 12 }}>
                    <div>
                        <label style={label}>Ancho del papel</label>
                        <select value={values.printer_width || '32'}
                            onChange={e => onChange('printer_width', e.target.value)} className="input">
                            <option value="32">58 mm (32 caracteres)</option>
                            <option value="48">80 mm (48 caracteres)</option>
                        </select>
                    </div>
                    <div>
                        <label style={label}>Imprimir al cobrar</label>
                        <select value={values.printer_auto_print === '0' ? '0' : '1'}
                            onChange={e => onChange('printer_auto_print', e.target.value)} className="input">
                            <option value="1">Sí, automático</option>
                            <option value="0">No, solo manual</option>
                        </select>
                    </div>
                </div>

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 14 }}>
                    <div>
                        <label style={label}>Abrir cajón con efectivo</label>
                        <select value={values.drawer_open_on_cash === '0' ? '0' : '1'}
                            onChange={e => onChange('drawer_open_on_cash', e.target.value)} className="input">
                            <option value="1">Sí</option>
                            <option value="0">No</option>
                        </select>
                    </div>
                    <div>
                        <label style={label}>Comando del cajón</label>
                        <input value={values.drawer_kick_command ?? '1B 70 00 19 FA'}
                            onChange={e => onChange('drawer_kick_command', e.target.value)}
                            className="input" style={{ fontFamily: 'monospace', fontSize: 12 }} />
                    </div>
                </div>

                <p style={{ ...hint, marginBottom: 14 }}>
                    Si el cajón no responde con el valor por defecto, prueba
                    <code style={{ margin: '0 4px' }}>1B 70 01 19 FA</code>: unos modelos
                    usan el pin 5 en lugar del pin 2.
                </p>

                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                    <button onClick={() => runTest(false)} disabled={testing} className="btn btn-ghost btn-sm">
                        Imprimir prueba
                    </button>
                    <button onClick={() => runTest(true)} disabled={testing} className="btn btn-ghost btn-sm">
                        Probar cajón
                    </button>
                </div>
            </div>

            {/* ── Lector de código de barras ── */}
            <div className="card" style={section}>
                <p style={heading}>Lector de código de barras</p>
                <p style={hint}>
                    Casi todos los lectores se comportan como un teclado y funcionan sin
                    instalar nada. Lo único que cambia entre modelos es la velocidad y
                    con qué tecla terminan: conecta el tuyo, escanea aquí abajo y los
                    ajustes se calculan solos.
                </p>

                <label style={label}>Prueba de lectura</label>
                <input
                    ref={calibrationRef}
                    readOnly
                    onFocus={() => { setCalibrating(true); setSample(null); }}
                    onBlur={() => setCalibrating(false)}
                    placeholder={calibrating ? 'Escanea cualquier producto…' : 'Haz clic aquí y escanea'}
                    className="input"
                    style={{
                        marginBottom: 12,
                        borderColor: calibrating ? 'var(--primary)' : undefined,
                        fontFamily: 'monospace',
                    }}
                />

                {sample && (
                    <div style={{ padding: '12px 14px', borderRadius: 12, marginBottom: 12, background: 'rgba(34,211,160,0.08)', border: '1px solid rgba(34,211,160,0.22)' }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, marginBottom: 4 }}>
                            <span style={{ color: 'var(--t3)' }}>Código leído</span>
                            <span style={{ fontFamily: 'monospace', color: 'var(--t1)' }}>{sample.code || '(vacío)'}</span>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, marginBottom: 4 }}>
                            <span style={{ color: 'var(--t3)' }}>Velocidad</span>
                            <span style={{ color: 'var(--t2)' }}>{Math.round(sample.totalMs)} ms · hueco máx. {Math.round(sample.maxGapMs)} ms</span>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, marginBottom: 10 }}>
                            <span style={{ color: 'var(--t3)' }}>Termina con</span>
                            <span style={{ color: 'var(--t2)' }}>{SUFFIX_LABELS[sample.detectedSuffix]}</span>
                        </div>
                        <button onClick={applySample} className="btn btn-primary btn-sm">Usar esta configuración</button>
                    </div>
                )}

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
                    <div>
                        <label style={label}>Tecla final</label>
                        <select value={values.scanner_suffix || 'enter'}
                            onChange={e => onChange('scanner_suffix', e.target.value)} className="input">
                            {(Object.keys(SUFFIX_LABELS) as ScannerSuffix[]).map(k => (
                                <option key={k} value={k}>{SUFFIX_LABELS[k]}</option>
                            ))}
                        </select>
                    </div>
                    <div>
                        <label style={label}>Prefijo (si lo tiene)</label>
                        <input value={values.scanner_prefix ?? ''}
                            onChange={e => onChange('scanner_prefix', e.target.value)}
                            className="input" placeholder="ninguno" />
                    </div>
                    <div>
                        <label style={label}>Hueco máx. entre teclas (ms)</label>
                        <input type="number" value={values.scanner_max_gap_ms || '60'}
                            onChange={e => onChange('scanner_max_gap_ms', e.target.value)} className="input" />
                    </div>
                    <div>
                        <label style={label}>Longitud mínima</label>
                        <input type="number" value={values.scanner_min_length || '4'}
                            onChange={e => onChange('scanner_min_length', e.target.value)} className="input" />
                    </div>
                </div>
            </div>
        </div>
    );
}
