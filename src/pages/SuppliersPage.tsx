import { useEffect, useState } from 'react';
import * as api from '../api';
import type { Supplier } from '../types';
import { useToast } from '../contexts/ToastContext';

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoPlus   = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>;
const IcoX      = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoEdit   = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>;
const IcoSearch = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>;
const IcoLoader = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;
const IcoTruck  = () => <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" className="opacity-40"><rect x="1" y="3" width="15" height="13"/><polygon points="16 8 20 8 23 11 23 16 16 16 16 8"/><circle cx="5.5" cy="18.5" r="2.5"/><circle cx="18.5" cy="18.5" r="2.5"/></svg>;
const IcoPhone  = () => <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07A19.5 19.5 0 0 1 4.69 12 19.79 19.79 0 0 1 1.61 3.4 2 2 0 0 1 3.6 1.22h3a2 2 0 0 1 2 1.72c.127.96.361 1.903.7 2.81a2 2 0 0 1-.45 2.11L7.91 8.9a16 16 0 0 0 6.09 6.09l.98-.98a2 2 0 0 1 2.11-.45c.907.339 1.85.573 2.81.7A2 2 0 0 1 22 16.92z"/></svg>;
const IcoMail   = () => <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/><polyline points="22,6 12,13 2,6"/></svg>;
const IcoMap    = () => <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"/><circle cx="12" cy="10" r="3"/></svg>;

// ─── Gradient avatar helper ──────────────────────────────────────────────────
const GRAD_PAIRS: [string, string][] = [
    ['#8B78F5','#F0C547'],['#F45270','#F0C547'],['#22D3A0','#8B78F5'],
    ['#F5A842','#F45270'],['#8B78F5','#22D3A0'],
];
function getGrad(name: string): [string, string] {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0;
    return GRAD_PAIRS[h % GRAD_PAIRS.length];
}

export default function SuppliersPage() {
    const [suppliers, setSuppliers] = useState<Supplier[]>([]);
    const [loading, setLoading] = useState(true);
    const [showForm, setShowForm] = useState(false);
    const [editing, setEditing] = useState<Supplier | null>(null);
    const [form, setForm] = useState({ name: '', contact_name: '', phone: '', email: '', address: '', notes: '' });
    const [search, setSearch] = useState('');
    const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'inactive'>('active');
    const [processing, setProcessing] = useState(false);
    const { showToast } = useToast();

    useEffect(() => { load(); }, []);

    const load = async () => {
        try { setSuppliers(await api.getSuppliers()); } catch (e) { showToast(String(e), 'error'); } finally { setLoading(false); }
    };

    const openCreate = () => {
        setEditing(null); setForm({ name: '', contact_name: '', phone: '', email: '', address: '', notes: '' }); setShowForm(true);
    };
    const openEdit = (s: Supplier) => {
        setEditing(s); setForm({ name: s.name, contact_name: s.contact_name || '', phone: s.phone || '', email: s.email || '', address: s.address || '', notes: s.notes || '' }); setShowForm(true);
    };

    const handleSave = async () => {
        if (!form.name) return;
        setProcessing(true);
        try {
            if (editing) {
                await api.updateSupplier({ ...editing, ...form, contact_name: form.contact_name || null, phone: form.phone || null, email: form.email || null, address: form.address || null, notes: form.notes || null });
            } else {
                await api.createSupplier({ name: form.name, contact_name: form.contact_name || null, phone: form.phone || null, email: form.email || null, address: form.address || null, notes: form.notes || null });
            }
            setShowForm(false); load();
        } catch (e) { showToast(String(e), 'error'); } finally { setProcessing(false); }
    };

    const handleToggleActive = async (supplier: Supplier) => {
        try { await api.updateSupplier({ ...supplier, is_active: !supplier.is_active }); load(); } catch (e) { showToast(String(e), 'error'); }
    };

    const filtered = suppliers.filter(s => {
        const q = search.toLowerCase().trim();
        if (q && ![s.name, s.contact_name || '', s.phone || '', s.email || '', s.address || ''].some(v => v.toLowerCase().includes(q))) return false;
        if (statusFilter === 'active' && !s.is_active) return false;
        if (statusFilter === 'inactive' && s.is_active) return false;
        return true;
    });

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
                    <h1 className="page-title">Proveedores</h1>
                    <p className="page-subtitle">{filtered.length} de {suppliers.length} proveedores</p>
                </div>
                <button onClick={openCreate} className="btn btn-primary" style={{ gap: 7 }}>
                    <IcoPlus /> Nuevo Proveedor
                </button>
            </div>

            {/* Filters */}
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
                <div style={{ flex: 1, minWidth: 240, position: 'relative' }}>
                    <span style={{ position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--t3)', pointerEvents: 'none' }}><IcoSearch /></span>
                    <input value={search} onChange={e => setSearch(e.target.value)} className="input" style={{ paddingLeft: 36 }} placeholder="Buscar proveedor, contacto, teléfono…" />
                </div>
                <select value={statusFilter} onChange={e => setStatusFilter(e.target.value as 'all' | 'active' | 'inactive')} className="input" style={{ width: 'auto', minWidth: 140 }}>
                    <option value="active">Activos</option>
                    <option value="inactive">Inactivos</option>
                    <option value="all">Todos</option>
                </select>
            </div>

            {/* Cards */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 14 }}>
                {filtered.map(s => {
                    const [gradA, gradB] = getGrad(s.name);
                    return (
                        <div key={s.id} className="glass" style={{ padding: 20, transition: 'all 0.18s' }}>
                            {/* Card top */}
                            <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', marginBottom: 14 }}>
                                {/* Avatar */}
                                <div style={{ width: 40, height: 40, borderRadius: 12, background: `linear-gradient(135deg,${gradA},${gradB})`, display: 'grid', placeItems: 'center', flexShrink: 0, color: '#fff', fontWeight: 800, fontSize: 15 }}>
                                    {s.name.slice(0, 2).toUpperCase()}
                                </div>
                                {/* Actions */}
                                <div style={{ display: 'flex', gap: 4 }}>
                                    {/* Toggle */}
                                    <button onClick={() => handleToggleActive(s)} title={s.is_active ? 'Desactivar' : 'Activar'}
                                        style={{ padding: 6, borderRadius: 8, color: s.is_active ? 'var(--success)' : 'var(--t3)', cursor: 'pointer' }}
                                        onMouseEnter={e => (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.06)'}
                                        onMouseLeave={e => (e.currentTarget as HTMLButtonElement).style.background = 'transparent'}
                                    >
                                        {s.is_active
                                            ? <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="1" y="5" width="22" height="14" rx="7"/><circle cx="16" cy="12" r="3" fill="currentColor"/></svg>
                                            : <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="1" y="5" width="22" height="14" rx="7"/><circle cx="8" cy="12" r="3" fill="currentColor" opacity="0.4"/></svg>
                                        }
                                    </button>
                                    <button onClick={() => openEdit(s)} style={{ padding: 6, borderRadius: 8, color: 'var(--t3)', cursor: 'pointer' }}
                                        onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--primary)'; (e.currentTarget as HTMLButtonElement).style.background = 'rgba(139,120,245,0.10)'; }}
                                        onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
                                    ><IcoEdit /></button>
                                </div>
                            </div>

                            {/* Name + status */}
                            <h3 style={{ fontSize: 14, fontWeight: 700, color: 'var(--t1)', marginBottom: 4 }}>{s.name}</h3>
                            <span className={`badge ${s.is_active ? 'badge-success' : 'badge-muted'}`} style={{ marginBottom: s.contact_name ? 8 : 12 }}>
                                {s.is_active ? 'Activo' : 'Inactivo'}
                            </span>
                            {s.contact_name && <p style={{ fontSize: 12, color: 'var(--t2)', marginTop: 6, marginBottom: 10 }}>{s.contact_name}</p>}

                            {/* Contact info */}
                            <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginTop: s.contact_name ? 0 : 8 }}>
                                {s.phone && (
                                    <p style={{ fontSize: 12, color: 'var(--t3)', display: 'flex', alignItems: 'center', gap: 7 }}>
                                        <IcoPhone /> {s.phone}
                                    </p>
                                )}
                                {s.email && (
                                    <p style={{ fontSize: 12, color: 'var(--t3)', display: 'flex', alignItems: 'center', gap: 7 }}>
                                        <IcoMail /> {s.email}
                                    </p>
                                )}
                                {s.address && (
                                    <p style={{ fontSize: 12, color: 'var(--t3)', display: 'flex', alignItems: 'center', gap: 7 }}>
                                        <IcoMap /> {s.address}
                                    </p>
                                )}
                            </div>
                        </div>
                    );
                })}

                {filtered.length === 0 && (
                    <div style={{ gridColumn: '1 / -1', padding: '64px 0', textAlign: 'center', color: 'var(--t3)' }}>
                        <IcoTruck />
                        <p style={{ marginTop: 12, fontSize: 13 }}>Sin proveedores registrados</p>
                    </div>
                )}
            </div>

            {/* Form Modal */}
            {showForm && (
                <div className="modal-overlay" onClick={() => setShowForm(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 420, maxHeight: '90vh', overflowY: 'auto' }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '20px 24px', borderBottom: '1px solid rgba(255,255,255,0.07)' }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>{editing ? 'Editar' : 'Nuevo'} Proveedor</h3>
                            <button onClick={() => setShowForm(false)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t1)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; }}
                            ><IcoX /></button>
                        </div>
                        <div style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 14 }}>
                            {([
                                { key: 'name', label: 'Nombre *', placeholder: 'Nombre del proveedor' },
                                { key: 'contact_name', label: 'Persona de Contacto', placeholder: 'Nombre del contacto' },
                                { key: 'phone', label: 'Teléfono', placeholder: '+52 ...' },
                                { key: 'email', label: 'Email', placeholder: 'correo@proveedor.com' },
                                { key: 'address', label: 'Dirección', placeholder: 'Dirección completa' },
                                { key: 'notes', label: 'Notas', placeholder: 'Notas adicionales…' },
                            ] as const).map(f => (
                                <div key={f.key}>
                                    <label className="form-label">{f.label}</label>
                                    <input type="text" value={(form as Record<string, string>)[f.key]} onChange={e => setForm({ ...form, [f.key]: e.target.value })} className="input" placeholder={f.placeholder} />
                                </div>
                            ))}
                            <div style={{ display: 'flex', gap: 10, paddingTop: 4 }}>
                                <button onClick={() => setShowForm(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handleSave} disabled={processing} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>
                                    {processing && <IcoLoader />} Guardar
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
