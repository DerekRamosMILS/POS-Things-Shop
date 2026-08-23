import { useEffect, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import * as api from '../api';
import HardwareSettings from '../components/HardwareSettings';
import type { SystemConfig } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';
import { useSessionStore } from '../stores/useSessionStore';

const NUMERIC_KEYS = ['tax_rate', 'low_stock_threshold', 'max_backups', 'session_hours',
    'log_retention_days', 'scanner_max_gap_ms', 'scanner_min_length', 'printer_width'];
const MULTILINE_KEYS = ['ticket_footer'];
// Kept out of the editable grid: it is plumbing, not a shop setting.
const HIDDEN_KEYS = ['demo_seeded', 'update_endpoint'];
// Estas se editan en su propia tarjeta, no en la reja genérica de la tienda.
const HARDWARE_KEYS = [
    'printer_name', 'printer_width', 'printer_auto_print',
    'drawer_kick_command', 'drawer_open_on_cash',
    'scanner_enabled', 'scanner_suffix', 'scanner_prefix',
    'scanner_max_gap_ms', 'scanner_min_length',
];

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoSave     = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>;
const IcoDownload = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>;
const IcoLoader   = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;

export default function SettingsPage() {
    const [configs, setConfigs] = useState<SystemConfig[]>([]);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [backupList, setBackupList] = useState<string[]>([]);
    const [values, setValues] = useState<Record<string, string>>({});
    const { showToast } = useToast();
    const { confirm } = useConfirm();
    const { user } = useSessionStore();
    const [logPath, setLogPath] = useState('');
    const [exporting, setExporting] = useState(false);
    const [checkingUpdate, setCheckingUpdate] = useState(false);
    const [pwCurrent, setPwCurrent] = useState('');
    const [pwNext, setPwNext] = useState('');
    const [pwSaving, setPwSaving] = useState(false);

    const handleSeed = async () => {
        if (!user) return;
        const ok = await confirm({
            title: 'Cargar datos de prueba',
            message: 'Se agregarán productos, clientes, proveedores y ventas de ejemplo para probar el sistema. Solo se puede hacer una vez. ¿Continuar?',
            confirmLabel: 'Cargar datos',
        });
        if (!ok) return;
        try {
            const msg = await api.seedDemoData();
            showToast(msg);
            setTimeout(() => window.location.reload(), 900);
        } catch (e) { showToast(String(e), 'error'); }
    };

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [cAll, b] = await Promise.all([api.getAllConfig(), api.getBackupList()]);
            // Hide internal-only flags from the editable settings UI.
            const c = cAll.filter(cfg => !HIDDEN_KEYS.includes(cfg.key));
            api.getLogPath().then(setLogPath).catch(() => {});
            setConfigs(c); setBackupList(b);
            const vals: Record<string, string> = {};
            c.forEach(cfg => { vals[cfg.key] = cfg.value; });
            setValues(vals);
        } catch (err) { showToast(String(err), 'error'); } finally { setLoading(false); }
    };

    const handleSave = async () => {
        for (const key of NUMERIC_KEYS) {
            if (values[key] !== undefined && values[key] !== '' && Number.isNaN(Number(values[key]))) {
                showToast(`${labels[key] || key} debe ser numérico`, 'error'); return;
            }
        }
        setSaving(true);
        try {
            for (const cfg of configs) {
                if (values[cfg.key] !== cfg.value) await api.setConfig(cfg.key, values[cfg.key]);
            }
            showToast('Configuración guardada'); loadData();
        } catch (err) { showToast(String(err), 'error'); } finally { setSaving(false); }
    };

    const handleBackup = async () => {
        try { const p = await api.createBackup(); showToast(`Backup creado: ${p}`); loadData(); }
        catch (err) { showToast(String(err), 'error'); }
    };

    const handleRestore = async (filename: string) => {
        const ok = await confirm({
            title: 'Restaurar respaldo',
            message: `Se reemplazará la base de datos actual con "${filename}". Los datos actuales se perderán. La app deberá reiniciarse. ¿Continuar?`,
            variant: 'danger', confirmLabel: 'Restaurar',
        });
        if (!ok) return;
        try { const msg = await api.restoreBackup(filename); showToast(msg); }
        catch (err) { showToast(String(err), 'error'); }
    };

    /// Copy the live database somewhere the operator chooses (USB, network share).
    const handleExport = async () => {
        setExporting(true);
        try {
            const stamp = new Date().toISOString().slice(0, 10);
            const target = await save({
                title: 'Exportar base de datos',
                defaultPath: `things-shop-${stamp}.db`,
                filters: [{ name: 'Base de datos SQLite', extensions: ['db'] }],
            });
            if (!target) return;
            await api.exportDatabase(target);
            showToast('Base de datos exportada');
        } catch (err) { showToast(String(err), 'error'); }
        finally { setExporting(false); }
    };

    const handleCheckUpdate = async () => {
        setCheckingUpdate(true);
        try {
            const update = await check();
            if (!update) { showToast('Ya tienes la versión más reciente'); return; }

            const ok = await confirm({
                title: `Actualizar a la versión ${update.version}`,
                message: `${update.body || 'Hay una nueva versión disponible.'}\n\nLa aplicación se reiniciará al terminar. Cierra la caja antes de continuar.`,
                confirmLabel: 'Instalar',
            });
            if (!ok) return;

            await update.downloadAndInstall();
            await relaunch();
        } catch (err) { showToast(`No se pudo buscar actualizaciones: ${err}`, 'error'); }
        finally { setCheckingUpdate(false); }
    };

    const handleChangeOwnPassword = async () => {
        if (pwNext.length < 8) { showToast('La nueva contraseña debe tener al menos 8 caracteres', 'error'); return; }
        setPwSaving(true);
        try {
            await api.changeOwnPassword({ current_password: pwCurrent, new_password: pwNext });
            setPwCurrent(''); setPwNext('');
            showToast('Contraseña actualizada');
        } catch (err) { showToast(String(err), 'error'); }
        finally { setPwSaving(false); }
    };

    const labels: Record<string, string> = {
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
    };

    const changedCount = configs.filter(cfg => values[cfg.key] !== cfg.value).length;

    if (loading) return (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
            <IcoLoader />
        </div>
    );

    return (
        <div className="page-container">
            {/* Header */}
            <div className="page-header">
                <div>
                    <h1 className="page-title">Configuración</h1>
                    <p className="page-subtitle">
                        Ajustes generales del sistema
                        {changedCount > 0 && <span style={{ color: 'var(--warning)', fontWeight: 700 }}> · {changedCount} cambios pendientes</span>}
                    </p>
                </div>
                <button onClick={handleSave} disabled={saving} className="btn btn-primary" style={{ gap: 7 }}>
                    {saving ? <IcoLoader /> : <IcoSave />} Guardar Cambios
                </button>
            </div>

            {/* KPIs */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 14 }}>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Configuraciones</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{configs.length}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Cambios pendientes</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: changedCount > 0 ? 'var(--warning)' : 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{changedCount}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Respaldos</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--accent)', fontVariantNumeric: 'tabular-nums' }}>{backupList.length}</p>
                </div>
            </div>

            {/* Store settings */}
            <div className="card" style={{ padding: '22px 24px' }}>
                <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', marginBottom: 18, display: 'flex', alignItems: 'center', gap: 8 }}>
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--primary)" strokeWidth="2.5" strokeLinecap="round"><circle cx="12" cy="12" r="3"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14"/></svg>
                    Datos de la Tienda
                </p>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))', gap: 14 }}>
                    {configs.filter(cfg => !HARDWARE_KEYS.includes(cfg.key)).map(cfg => {
                        const changed = values[cfg.key] !== cfg.value;
                        const borderColor = changed ? 'rgba(240,197,71,0.5)' : undefined;
                        return (
                            <div key={cfg.key}>
                                <label className="form-label">{labels[cfg.key] || cfg.key}</label>
                                {cfg.key === 'auto_backup' ? (
                                    <select
                                        value={values[cfg.key] === '1' ? '1' : '0'}
                                        onChange={e => setValues({ ...values, [cfg.key]: e.target.value })}
                                        className="input"
                                        style={{ borderColor }}
                                    >
                                        <option value="1">Sí</option>
                                        <option value="0">No</option>
                                    </select>
                                ) : MULTILINE_KEYS.includes(cfg.key) ? (
                                    <textarea
                                        rows={2}
                                        value={values[cfg.key] || ''}
                                        onChange={e => setValues({ ...values, [cfg.key]: e.target.value })}
                                        className="input"
                                        style={{ borderColor, resize: 'vertical', fontFamily: 'inherit' }}
                                    />
                                ) : (
                                    <input
                                        type={NUMERIC_KEYS.includes(cfg.key) ? 'number' : 'text'}
                                        value={values[cfg.key] || ''}
                                        onChange={e => setValues({ ...values, [cfg.key]: e.target.value })}
                                        className="input"
                                        style={{ borderColor }}
                                    />
                                )}
                            </div>
                        );
                    })}
                </div>
            </div>

            {/* Backups */}
            <div className="card" style={{ padding: '22px 24px' }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 18 }}>
                    <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', display: 'flex', alignItems: 'center', gap: 8 }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2.5" strokeLinecap="round"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
                        Respaldos del Sistema
                    </p>
                    <div style={{ display: 'flex', gap: 8 }}>
                        <button onClick={handleExport} disabled={exporting} className="btn btn-ghost btn-sm" style={{ gap: 7 }}>
                            {exporting ? <IcoLoader /> : <IcoDownload />} Exportar a archivo
                        </button>
                        <button onClick={handleBackup} className="btn btn-ghost btn-sm" style={{ gap: 7 }}>
                            <IcoDownload /> Crear Respaldo
                        </button>
                    </div>
                </div>
                {backupList.length === 0 ? (
                    <p style={{ textAlign: 'center', padding: '32px 0', fontSize: 13, color: 'var(--t3)' }}>Sin respaldos disponibles</p>
                ) : (
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                        {backupList.slice(0, 10).map(b => (
                            <div key={b} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 10, padding: '9px 14px', borderRadius: 10, background: 'rgba(255,255,255,0.03)' }}>
                                <span style={{ fontSize: 12, color: 'var(--t3)', fontFamily: 'monospace', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{b}</span>
                                <button onClick={() => handleRestore(b)} className="btn btn-ghost btn-sm" style={{ flexShrink: 0 }}>Restaurar</button>
                            </div>
                        ))}
                    </div>
                )}
            </div>

            {/* Hardware del mostrador */}
            <HardwareSettings
                values={values}
                onChange={(key, value) => setValues(v => ({ ...v, [key]: value }))}
            />

            {/* Security + maintenance */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: 14 }}>
                <div className="card" style={{ padding: '22px 24px' }}>
                    <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', marginBottom: 6 }}>Mi contraseña</p>
                    <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 14, lineHeight: 1.5 }}>
                        Cámbiala sin depender de otro administrador. Mínimo 8 caracteres.
                    </p>
                    <label className="form-label">Contraseña actual</label>
                    <input type="password" value={pwCurrent} onChange={e => setPwCurrent(e.target.value)} className="input" style={{ marginBottom: 10 }} />
                    <label className="form-label">Nueva contraseña</label>
                    <input type="password" value={pwNext} onChange={e => setPwNext(e.target.value)} className="input" style={{ marginBottom: 14 }} />
                    <button onClick={handleChangeOwnPassword} disabled={pwSaving || !pwCurrent || !pwNext} className="btn btn-primary btn-sm">
                        {pwSaving ? 'Guardando...' : 'Cambiar contraseña'}
                    </button>
                </div>

                <div className="card" style={{ padding: '22px 24px' }}>
                    <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', marginBottom: 6 }}>Mantenimiento</p>
                    <p style={{ fontSize: 12, color: 'var(--t3)', marginBottom: 14, lineHeight: 1.5 }}>
                        Si algo falla, este es el archivo que hay que enviar a soporte.
                    </p>
                    <label className="form-label">Bitácora de la aplicación</label>
                    <input readOnly value={logPath || '—'} className="input" style={{ fontFamily: 'monospace', fontSize: 11, marginBottom: 14 }}
                        onFocus={e => e.currentTarget.select()} />
                    <button onClick={handleCheckUpdate} disabled={checkingUpdate} className="btn btn-ghost btn-sm" style={{ gap: 7 }}>
                        {checkingUpdate ? <IcoLoader /> : null} Buscar actualizaciones
                    </button>
                </div>
            </div>

            {/* Demo data */}
            <div className="card" style={{ padding: '22px 24px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16, flexWrap: 'wrap' }}>
                <div>
                    <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', display: 'flex', alignItems: 'center', gap: 8 }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--primary)" strokeWidth="2.5" strokeLinecap="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg>
                        Datos de prueba
                    </p>
                    <p style={{ fontSize: 12, color: 'var(--t3)', marginTop: 6, lineHeight: 1.5, maxWidth: 460 }}>
                        Carga productos, clientes, proveedores y ventas de ejemplo para probar el sistema. Solo se puede ejecutar una vez.
                    </p>
                </div>
                <button onClick={handleSeed} className="btn btn-primary btn-sm" style={{ flexShrink: 0 }}>Cargar datos de prueba</button>
            </div>
        </div>
    );
}
