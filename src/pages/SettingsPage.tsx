import { useEffect, useState } from 'react';
import * as api from '../api';
import type { SystemConfig } from '../types';
import { useToast } from '../contexts/ToastContext';

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

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [c, b] = await Promise.all([api.getAllConfig(), api.getBackupList()]);
            setConfigs(c); setBackupList(b);
            const vals: Record<string, string> = {};
            c.forEach(cfg => { vals[cfg.key] = cfg.value; });
            setValues(vals);
        } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    const handleSave = async () => {
        const numericKeys = ['tax_rate', 'low_stock_threshold', 'max_backups'];
        for (const key of numericKeys) {
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

    const labels: Record<string, string> = {
        store_name: 'Nombre de la Tienda',
        store_address: 'Dirección',
        store_phone: 'Teléfono',
        store_email: 'Email',
        ticket_footer: 'Pie de Ticket',
        tax_rate: 'Tasa de Impuesto (%)',
        currency_symbol: 'Símbolo de Moneda',
        low_stock_threshold: 'Umbral de Stock Mínimo',
        auto_backup: 'Backup al Cerrar Turno',
        max_backups: 'Límite de Días (Se borra el día 91)',
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
                    {configs.map(cfg => (
                        <div key={cfg.key}>
                            <label className="form-label">{labels[cfg.key] || cfg.key}</label>
                            <input
                                type="text"
                                value={values[cfg.key] || ''}
                                onChange={e => setValues({ ...values, [cfg.key]: e.target.value })}
                                className="input"
                                style={{ borderColor: values[cfg.key] !== cfg.value ? 'rgba(240,197,71,0.5)' : undefined }}
                            />
                        </div>
                    ))}
                </div>
            </div>

            {/* Backups */}
            <div className="card" style={{ padding: '22px 24px' }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 18 }}>
                    <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', display: 'flex', alignItems: 'center', gap: 8 }}>
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2.5" strokeLinecap="round"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
                        Respaldos del Sistema
                    </p>
                    <button onClick={handleBackup} className="btn btn-ghost btn-sm" style={{ gap: 7 }}>
                        <IcoDownload /> Crear Respaldo
                    </button>
                </div>
                {backupList.length === 0 ? (
                    <p style={{ textAlign: 'center', padding: '32px 0', fontSize: 13, color: 'var(--t3)' }}>Sin respaldos disponibles</p>
                ) : (
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                        {backupList.slice(0, 10).map(b => (
                            <div key={b} style={{ padding: '9px 14px', borderRadius: 10, background: 'rgba(255,255,255,0.03)', fontSize: 12, color: 'var(--t3)', fontFamily: 'monospace' }}>
                                {b}
                            </div>
                        ))}
                    </div>
                )}
            </div>
        </div>
    );
}
