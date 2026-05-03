import { useEffect, useState } from 'react';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { Promotion, Category, Product } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoPlus    = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>;
const IcoX       = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoEdit    = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>;
const IcoTrash   = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/></svg>;
const IcoSearch  = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>;
const IcoFilter  = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/></svg>;
const IcoLoader  = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;
const IcoPercent = () => <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" className="opacity-40"><line x1="19" y1="5" x2="5" y2="19"/><circle cx="6.5" cy="6.5" r="2.5"/><circle cx="17.5" cy="17.5" r="2.5"/></svg>;
const IcoTag     = () => <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/></svg>;
const IcoBox     = () => <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>;

const DISCOUNT_TYPES = [{ key: 'percentage', label: 'Porcentaje (%)' }, { key: 'fixed', label: 'Monto Fijo ($)' }];
const APPLIES_TO = [{ key: 'all', label: 'Todos los productos' }, { key: 'category', label: 'Categoría específica' }, { key: 'product', label: 'Producto específico' }];

export default function DiscountsPage() {
    const { user } = useSessionStore();
    const isAdmin = user?.role === 'admin';
    const [promotions, setPromotions] = useState<Promotion[]>([]);
    const [categories, setCategories] = useState<Category[]>([]);
    const [products, setProducts] = useState<Product[]>([]);
    const [loading, setLoading] = useState(true);
    const [showForm, setShowForm] = useState(false);
    const [editing, setEditing] = useState<Promotion | null>(null);
    const [form, setForm] = useState({ name: '', description: '', discount_type: 'percentage', discount_value: '', start_date: '', end_date: '', applies_to: 'all', target_id: '' });
    const [search, setSearch] = useState('');
    const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'scheduled' | 'inactive'>('all');
    const [scopeFilter, setScopeFilter] = useState('');
    const [processing, setProcessing] = useState(false);
    const { showToast } = useToast();
    const { confirm } = useConfirm();

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [promos, cats, prods] = await Promise.all([api.getPromotions(), api.getCategories(), api.getProducts({ is_active: true })]);
            setPromotions(promos); setCategories(cats); setProducts(prods);
        } catch (e) { console.error(e); } finally { setLoading(false); }
    };

    const openCreate = () => {
        setEditing(null);
        const today = new Date().toISOString().split('T')[0];
        const nextMonth = new Date(Date.now() + 30 * 86400000).toISOString().split('T')[0];
        setForm({ name: '', description: '', discount_type: 'percentage', discount_value: '', start_date: today, end_date: nextMonth, applies_to: 'all', target_id: '' });
        setShowForm(true);
    };

    const openEdit = (p: Promotion) => {
        setEditing(p);
        setForm({ name: p.name, description: p.description || '', discount_type: p.discount_type, discount_value: p.discount_value.toString(), start_date: p.start_date.split('T')[0] || p.start_date, end_date: p.end_date.split('T')[0] || p.end_date, applies_to: p.applies_to, target_id: p.target_id?.toString() || '' });
        setShowForm(true);
    };

    const handleSave = async () => {
        if (!form.name || !form.discount_value) return;
        setProcessing(true);
        try {
            const payload = { name: form.name, description: form.description || null, discount_type: form.discount_type, discount_value: parseFloat(form.discount_value) || 0, start_date: form.start_date, end_date: form.end_date, applies_to: form.applies_to, target_id: form.target_id ? parseInt(form.target_id) : null };
            if (editing) { await api.updatePromotion({ ...editing, ...payload }); }
            else { await api.createPromotion(payload); }
            setShowForm(false); loadData();
        } catch (e) { showToast(String(e), 'error'); } finally { setProcessing(false); }
    };

    const handleToggle = async (promo: Promotion) => {
        try { await api.updatePromotion({ ...promo, is_active: !promo.is_active }); loadData(); } catch (e) { showToast(String(e), 'error'); }
    };

    const handleDelete = async (id: number) => {
        const ok = await confirm({ title: 'Eliminar promoción', message: '¿Eliminar esta promoción?', variant: 'danger', confirmLabel: 'Eliminar' });
        if (!ok) return;
        try { await api.deletePromotion(id); loadData(); } catch (e) { showToast(String(e), 'error'); }
    };

    const getTargetLabel = (promo: Promotion) => {
        if (promo.applies_to === 'all') return 'Todos los productos';
        if (promo.applies_to === 'category') { const cat = categories.find(c => c.id === promo.target_id); return cat ? `Categoría: ${cat.name}` : 'Categoría'; }
        if (promo.applies_to === 'product') { const prod = products.find(p => p.id === promo.target_id); return prod ? `Producto: ${prod.name}` : 'Producto'; }
        return promo.applies_to;
    };

    const isActive = (promo: Promotion) => {
        if (!promo.is_active) return false;
        const now = new Date();
        return new Date(promo.start_date) <= now && now <= new Date(promo.end_date);
    };

    if (loading) return (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
            <IcoLoader />
        </div>
    );

    const activeCount = promotions.filter(p => isActive(p)).length;
    const filtered = promotions.filter(promo => {
        const active = isActive(promo);
        const q = search.toLowerCase().trim();
        if (q && ![promo.name, promo.description || '', getTargetLabel(promo)].some(v => v.toLowerCase().includes(q))) return false;
        if (scopeFilter && promo.applies_to !== scopeFilter) return false;
        if (statusFilter === 'active' && !active) return false;
        if (statusFilter === 'scheduled' && (active || !promo.is_active)) return false;
        if (statusFilter === 'inactive' && promo.is_active) return false;
        return true;
    });

    const hasFilters = !!(search || statusFilter !== 'all' || scopeFilter);

    return (
        <div className="page-container">
            {/* Header */}
            <div className="page-header">
                <div>
                    <h1 className="page-title">Descuentos y Promociones</h1>
                    <p className="page-subtitle">{activeCount} activas de {promotions.length} total</p>
                </div>
                {isAdmin && (
                    <button onClick={openCreate} className="btn btn-primary" style={{ gap: 7 }}>
                        <IcoPlus /> Nueva Promoción
                    </button>
                )}
            </div>

            {/* KPIs */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 14 }}>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Activas ahora</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{activeCount}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Filtradas</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{filtered.length}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Inactivas</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--danger)', fontVariantNumeric: 'tabular-nums' }}>{promotions.filter(p => !p.is_active).length}</p>
                </div>
            </div>

            {/* Filters */}
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
                <div style={{ flex: 1, minWidth: 240, position: 'relative' }}>
                    <span style={{ position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--t3)', pointerEvents: 'none' }}><IcoSearch /></span>
                    <input value={search} onChange={e => setSearch(e.target.value)} className="input" style={{ paddingLeft: 36 }} placeholder="Buscar promoción o producto objetivo…" />
                </div>
                <select value={statusFilter} onChange={e => setStatusFilter(e.target.value as 'all' | 'active' | 'scheduled' | 'inactive')} className="input" style={{ width: 'auto', minWidth: 150 }}>
                    <option value="all">Todos los estados</option>
                    <option value="active">Activas</option>
                    <option value="scheduled">Programadas</option>
                    <option value="inactive">Inactivas</option>
                </select>
                <select value={scopeFilter} onChange={e => setScopeFilter(e.target.value)} className="input" style={{ width: 'auto', minWidth: 170 }}>
                    <option value="">Todos los alcances</option>
                    {APPLIES_TO.map(a => <option key={a.key} value={a.key}>{a.label}</option>)}
                </select>
                {hasFilters && (
                    <button onClick={() => { setSearch(''); setStatusFilter('all'); setScopeFilter(''); }} className="btn btn-ghost btn-sm" style={{ gap: 6 }}>
                        <IcoFilter /> Limpiar
                    </button>
                )}
            </div>

            {/* Cards grid */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 14 }}>
                {filtered.map(promo => {
                    const active = isActive(promo);
                    return (
                        <div key={promo.id} className="card" style={{ padding: 20, borderColor: active ? 'rgba(139,120,245,0.30)' : undefined, opacity: promo.is_active ? 1 : 0.65, transition: 'all 0.18s' }}>
                            {/* Card header */}
                            <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', marginBottom: 16 }}>
                                <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                                    <div style={{ width: 36, height: 36, borderRadius: 11, display: 'grid', placeItems: 'center', background: active ? 'rgba(139,120,245,0.12)' : 'rgba(255,255,255,0.05)', flexShrink: 0 }}>
                                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke={active ? 'var(--primary)' : 'var(--t3)'} strokeWidth="2" strokeLinecap="round"><line x1="19" y1="5" x2="5" y2="19"/><circle cx="6.5" cy="6.5" r="2.5"/><circle cx="17.5" cy="17.5" r="2.5"/></svg>
                                    </div>
                                    <div>
                                        <h3 style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)' }}>{promo.name}</h3>
                                        {promo.description && <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 2 }}>{promo.description}</p>}
                                    </div>
                                </div>
                                {isAdmin && (
                                    <div style={{ display: 'flex', alignItems: 'center', gap: 4, marginLeft: 8 }}>
                                        {/* Toggle active */}
                                        <button onClick={() => handleToggle(promo)} style={{ padding: 6, borderRadius: 8, color: promo.is_active ? 'var(--success)' : 'var(--t3)', cursor: 'pointer' }}
                                            onMouseEnter={e => (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.06)'}
                                            onMouseLeave={e => (e.currentTarget as HTMLButtonElement).style.background = 'transparent'}
                                        >
                                            {promo.is_active
                                                ? <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="1" y="5" width="22" height="14" rx="7"/><circle cx="16" cy="12" r="3" fill="currentColor"/></svg>
                                                : <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="1" y="5" width="22" height="14" rx="7"/><circle cx="8" cy="12" r="3" fill="currentColor" opacity="0.4"/></svg>
                                            }
                                        </button>
                                        <button onClick={() => openEdit(promo)} style={{ padding: 6, borderRadius: 8, color: 'var(--t3)', cursor: 'pointer' }}
                                            onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--primary)'; (e.currentTarget as HTMLButtonElement).style.background = 'rgba(139,120,245,0.10)'; }}
                                            onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
                                        ><IcoEdit /></button>
                                        <button onClick={() => handleDelete(promo.id)} style={{ padding: 6, borderRadius: 8, color: 'var(--t3)', cursor: 'pointer' }}
                                            onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--danger)'; (e.currentTarget as HTMLButtonElement).style.background = 'rgba(244,82,112,0.10)'; }}
                                            onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
                                        ><IcoTrash /></button>
                                    </div>
                                )}
                            </div>

                            {/* Discount value */}
                            <div style={{ padding: '12px 0', marginBottom: 14, textAlign: 'center', borderRadius: 12, background: 'rgba(255,255,255,0.03)' }}>
                                <p style={{ fontSize: 28, fontWeight: 900, color: active ? 'var(--primary)' : 'var(--t3)', letterSpacing: '-0.02em', fontVariantNumeric: 'tabular-nums' }}>
                                    {promo.discount_type === 'percentage' ? `${promo.discount_value}%` : `$${promo.discount_value.toFixed(2)}`}
                                </p>
                                <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 2 }}>
                                    {promo.discount_type === 'percentage' ? 'de descuento' : 'descuento fijo'}
                                </p>
                            </div>

                            {/* Meta */}
                            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                                <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: 'var(--t3)' }}>
                                    {promo.applies_to === 'category' ? <IcoTag /> : <IcoBox />}
                                    <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{getTargetLabel(promo)}</span>
                                </div>
                                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                                    <span style={{ fontSize: 11, color: 'var(--t3)', fontFamily: 'monospace' }}>
                                        {promo.start_date.split('T')[0]} → {promo.end_date.split('T')[0]}
                                    </span>
                                    <span className={`badge ${active ? 'badge-success' : promo.is_active ? 'badge-primary' : 'badge-muted'}`}>
                                        {active ? 'Activa' : promo.is_active ? 'Programada' : 'Inactiva'}
                                    </span>
                                </div>
                            </div>
                        </div>
                    );
                })}

                {filtered.length === 0 && (
                    <div style={{ gridColumn: '1 / -1', padding: '64px 0', textAlign: 'center', color: 'var(--t3)' }}>
                        <IcoPercent />
                        <p style={{ marginTop: 12, fontSize: 13 }}>No hay promociones registradas</p>
                        {isAdmin && <p style={{ fontSize: 11, marginTop: 4 }}>Crea una nueva para comenzar</p>}
                    </div>
                )}
            </div>

            {/* Form Modal */}
            {showForm && (
                <div className="modal-overlay" onClick={() => setShowForm(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 440, maxHeight: '90vh', overflowY: 'auto' }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '20px 24px', borderBottom: '1px solid rgba(255,255,255,0.07)' }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>{editing ? 'Editar' : 'Nueva'} Promoción</h3>
                            <button onClick={() => setShowForm(false)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t1)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; }}
                            ><IcoX /></button>
                        </div>
                        <div style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div>
                                <label className="form-label">Nombre *</label>
                                <input type="text" value={form.name} onChange={e => setForm({ ...form, name: e.target.value })} className="input" placeholder="Ej: Descuento de Verano" />
                            </div>
                            <div>
                                <label className="form-label">Descripción</label>
                                <input type="text" value={form.description} onChange={e => setForm({ ...form, description: e.target.value })} className="input" placeholder="Opcional…" />
                            </div>
                            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                                <div>
                                    <label className="form-label">Tipo</label>
                                    <select value={form.discount_type} onChange={e => setForm({ ...form, discount_type: e.target.value })} className="input">
                                        {DISCOUNT_TYPES.map(t => <option key={t.key} value={t.key}>{t.label}</option>)}
                                    </select>
                                </div>
                                <div>
                                    <label className="form-label">Valor *</label>
                                    <input type="number" step="0.01" value={form.discount_value} onChange={e => setForm({ ...form, discount_value: e.target.value })} className="input" placeholder={form.discount_type === 'percentage' ? '10' : '50.00'} />
                                </div>
                            </div>
                            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                                <div>
                                    <label className="form-label">Fecha Inicio</label>
                                    <input type="date" value={form.start_date} onChange={e => setForm({ ...form, start_date: e.target.value })} className="input" />
                                </div>
                                <div>
                                    <label className="form-label">Fecha Fin</label>
                                    <input type="date" value={form.end_date} onChange={e => setForm({ ...form, end_date: e.target.value })} className="input" />
                                </div>
                            </div>
                            <div>
                                <label className="form-label">Aplica a</label>
                                <select value={form.applies_to} onChange={e => setForm({ ...form, applies_to: e.target.value, target_id: '' })} className="input">
                                    {APPLIES_TO.map(a => <option key={a.key} value={a.key}>{a.label}</option>)}
                                </select>
                            </div>
                            {form.applies_to === 'category' && (
                                <div>
                                    <label className="form-label">Categoría</label>
                                    <select value={form.target_id} onChange={e => setForm({ ...form, target_id: e.target.value })} className="input">
                                        <option value="">Seleccionar…</option>
                                        {categories.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
                                    </select>
                                </div>
                            )}
                            {form.applies_to === 'product' && (
                                <div>
                                    <label className="form-label">Producto</label>
                                    <select value={form.target_id} onChange={e => setForm({ ...form, target_id: e.target.value })} className="input">
                                        <option value="">Seleccionar…</option>
                                        {products.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                                    </select>
                                </div>
                            )}
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
