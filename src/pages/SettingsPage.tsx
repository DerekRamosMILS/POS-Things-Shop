import { useEffect, useState } from 'react';
import * as api from '../api';
import type { SystemConfig } from '../types';
import { Settings, Save, Loader2, Database, Download } from 'lucide-react';

export default function SettingsPage() {
    const [configs, setConfigs] = useState<SystemConfig[]>([]);
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [backupList, setBackupList] = useState<string[]>([]);
    const [values, setValues] = useState<Record<string, string>>({});

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [c, b] = await Promise.all([api.getAllConfig(), api.getBackupList()]);
            setConfigs(c); setBackupList(b);
            const vals: Record<string, string> = {};
            c.forEach((cfg) => { vals[cfg.key] = cfg.value; });
            setValues(vals);
        } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    const handleSave = async () => {
        setSaving(true);
        try {
            for (const cfg of configs) {
                if (values[cfg.key] !== cfg.value) await api.setConfig(cfg.key, values[cfg.key]);
            }
            alert('Configuración guardada'); loadData();
        } catch (err) { alert(String(err)); } finally { setSaving(false); }
    };

    const handleBackup = async () => {
        try { const p = await api.createBackup(); alert(`Backup: ${p}`); loadData(); }
        catch (err) { alert(String(err)); }
    };

    const labels: Record<string, string> = {
        store_name: 'Nombre de la Tienda', store_address: 'Dirección', store_phone: 'Teléfono',
        store_email: 'Email', ticket_footer: 'Pie de Ticket', tax_rate: 'Tasa de Impuesto (%)',
        currency_symbol: 'Símbolo de Moneda', low_stock_threshold: 'Umbral de Stock Mínimo',
        auto_backup: 'Backup Automático', max_backups: 'Máximo de Backups',
    };

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 lg:p-14 xl:p-16 h-full flex flex-col animate-fade-in overflow-y-auto">
            <div className="flex flex-col md:flex-row md:items-center justify-between gap-6 mb-10">
                <div>
                    <h1 className="text-4xl font-bold text-text-primary">Configuración</h1>
                    <p className="text-text-secondary text-lg mt-2">Ajustes generales del sistema</p>
                </div>
                <button onClick={handleSave} disabled={saving} className="flex items-center justify-center gap-3 px-8 py-4 bg-primary hover:bg-primary-hover text-[#0B0B0F] rounded-2xl font-bold text-base transition-all shadow-[0_4px_20px_rgba(0,224,90,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,224,90,0.4)] disabled:opacity-50 disabled:hover:translate-y-0 disabled:hover:shadow-none">
                    {saving ? <Loader2 size={22} className="animate-spin" /> : <Save size={22} />} Guardar Cambios
                </button>
            </div>

            <div className="glass rounded-[24px] border border-border p-10 lg:p-12 mb-10 shrink-0">
                <h2 className="text-xl font-semibold text-text-primary mb-10 flex items-center gap-4"><Settings size={26} className="text-primary" /> Datos de la Tienda</h2>
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-8">
                    {configs.map((cfg) => (
                        <div key={cfg.key}>
                            <label className="text-base font-medium text-text-secondary block mb-3">{labels[cfg.key] || cfg.key}</label>
                            <input type="text" value={values[cfg.key] || ''} onChange={(e) => setValues({ ...values, [cfg.key]: e.target.value })} className="w-full px-6 py-5 bg-bg-primary border border-border rounded-2xl text-text-primary text-base lg:text-lg focus:border-primary transition-colors focus:shadow-[0_0_15px_rgba(0,224,90,0.15)]" />
                        </div>
                    ))}
                </div>
            </div>

            <div className="glass rounded-[24px] border border-border p-10 lg:p-12 shrink-0 mb-10">
                <div className="flex flex-col md:flex-row md:items-center justify-between gap-6 mb-8">
                    <h2 className="text-xl font-semibold text-text-primary flex items-center gap-4"><Database size={26} className="text-accent" /> Respaldos del Sistema</h2>
                    <button onClick={handleBackup} className="flex items-center justify-center gap-3 px-8 py-4 bg-accent hover:bg-accent-hover text-[#0B0B0F] rounded-2xl text-base font-bold transition-all shadow-[0_4px_20px_rgba(0,255,163,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,255,163,0.4)]"><Download size={22} /> Crear Respaldo</button>
                </div>
                <div className="space-y-4">
                    {backupList.slice(0, 10).map((b) => <div key={b} className="px-8 py-5 bg-bg-primary rounded-2xl text-base font-mono tracking-wide text-text-secondary border border-white/5">{b}</div>)}
                    {backupList.length === 0 && <p className="text-text-muted text-lg font-medium tracking-wide text-center py-12 bg-bg-primary rounded-2xl border border-white/5">Sin respaldos disponibles</p>}
                </div>
            </div>
        </div>
    );
}
