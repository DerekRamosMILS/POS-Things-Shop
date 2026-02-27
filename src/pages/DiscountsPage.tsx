import { useEffect, useState } from 'react';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { Promotion, Category, Product } from '../types';
import { BadgePercent, Plus, Edit2, Trash2, X, Loader2, ToggleLeft, ToggleRight, Tag, Package } from 'lucide-react';

const DISCOUNT_TYPES = [
    { key: 'percentage', label: 'Porcentaje (%)' },
    { key: 'fixed', label: 'Monto Fijo ($)' },
];

const APPLIES_TO = [
    { key: 'all', label: 'Todos los productos' },
    { key: 'category', label: 'Categoría específica' },
    { key: 'product', label: 'Producto específico' },
];

export default function DiscountsPage() {
    const { user } = useSessionStore();
    const isAdmin = user?.role === 'admin';
    const [promotions, setPromotions] = useState<Promotion[]>([]);
    const [categories, setCategories] = useState<Category[]>([]);
    const [products, setProducts] = useState<Product[]>([]);
    const [loading, setLoading] = useState(true);
    const [showForm, setShowForm] = useState(false);
    const [editing, setEditing] = useState<Promotion | null>(null);
    const [form, setForm] = useState({
        name: '', description: '', discount_type: 'percentage', discount_value: '',
        start_date: '', end_date: '', applies_to: 'all', target_id: '',
    });
    const [processing, setProcessing] = useState(false);

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [promos, cats, prods] = await Promise.all([
                api.getPromotions(), api.getCategories(), api.getProducts({ is_active: true }),
            ]);
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
        setForm({
            name: p.name, description: p.description || '', discount_type: p.discount_type,
            discount_value: p.discount_value.toString(), start_date: p.start_date.split('T')[0] || p.start_date,
            end_date: p.end_date.split('T')[0] || p.end_date, applies_to: p.applies_to, target_id: p.target_id?.toString() || '',
        });
        setShowForm(true);
    };

    const handleSave = async () => {
        if (!form.name || !form.discount_value) return;
        setProcessing(true);
        try {
            const payload = {
                name: form.name, description: form.description || null, discount_type: form.discount_type,
                discount_value: parseFloat(form.discount_value) || 0, start_date: form.start_date, end_date: form.end_date,
                applies_to: form.applies_to, target_id: form.target_id ? parseInt(form.target_id) : null,
            };
            if (editing) { await api.updatePromotion({ ...editing, ...payload }); }
            else { await api.createPromotion(payload); }
            setShowForm(false); loadData();
        } catch (e) { alert(String(e)); } finally { setProcessing(false); }
    };

    const handleToggle = async (promo: Promotion) => {
        try { await api.updatePromotion({ ...promo, is_active: !promo.is_active }); loadData(); } catch (e) { alert(String(e)); }
    };

    const handleDelete = async (id: number) => {
        if (!confirm('¿Eliminar esta promoción/descuento?')) return;
        try { await api.deletePromotion(id); loadData(); } catch (e) { alert(String(e)); }
    };

    const getTargetLabel = (promo: Promotion) => {
        if (promo.applies_to === 'all') return 'Todos los productos';
        if (promo.applies_to === 'category') { const cat = categories.find(c => c.id === promo.target_id); return cat ? `Categoría: ${cat.name}` : 'Categoría (no encontrada)'; }
        if (promo.applies_to === 'product') { const prod = products.find(p => p.id === promo.target_id); return prod ? `Producto: ${prod.name}` : 'Producto (no encontrado)'; }
        return promo.applies_to;
    };

    const isActive = (promo: Promotion) => {
        if (!promo.is_active) return false;
        const now = new Date();
        return new Date(promo.start_date) <= now && now <= new Date(promo.end_date);
    };

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 space-y-8 animate-fade-in">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold text-text-primary">Descuentos y Promociones</h1>
                    <p className="text-text-secondary text-sm mt-1">{promotions.filter(p => isActive(p)).length} promociones activas de {promotions.length} total</p>
                </div>
                {isAdmin && (
                    <button onClick={openCreate} className="flex items-center gap-2 px-6 py-3.5 bg-primary hover:bg-primary-hover text-white rounded-2xl font-medium text-sm transition-colors">
                        <Plus size={18} /> Nueva Promoción
                    </button>
                )}
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {promotions.map((promo) => {
                    const active = isActive(promo);
                    return (
                        <div key={promo.id} className={`glass rounded-2xl border p-7 transition-colors ${active ? 'border-primary/30' : 'border-border opacity-70'}`}>
                            <div className="flex items-start justify-between mb-5">
                                <div className="flex items-center gap-3">
                                    <div className={`w-14 h-14 rounded-2xl flex items-center justify-center ${active ? 'bg-primary/10' : 'bg-bg-primary'}`}>
                                        <BadgePercent size={26} className={active ? 'text-primary' : 'text-text-muted'} />
                                    </div>
                                    <div>
                                        <h3 className="font-semibold text-text-primary text-lg">{promo.name}</h3>
                                        {promo.description && <p className="text-xs text-text-muted mt-0.5">{promo.description}</p>}
                                    </div>
                                </div>
                                {isAdmin && (
                                    <div className="flex gap-1">
                                        <button onClick={() => handleToggle(promo)} className={`p-2.5 rounded-xl transition-colors ${promo.is_active ? 'text-success hover:bg-success/10' : 'text-text-muted hover:bg-surface-hover'}`}>
                                            {promo.is_active ? <ToggleRight size={20} /> : <ToggleLeft size={20} />}
                                        </button>
                                        <button onClick={() => openEdit(promo)} className="p-2.5 text-text-muted hover:text-primary rounded-xl hover:bg-primary/10 transition-colors"><Edit2 size={16} /></button>
                                        <button onClick={() => handleDelete(promo.id)} className="p-2.5 text-text-muted hover:text-danger rounded-xl hover:bg-danger/10 transition-colors"><Trash2 size={16} /></button>
                                    </div>
                                )}
                            </div>

                            <div className="bg-bg-primary rounded-xl p-5 mb-5">
                                <div className="text-center">
                                    <p className="text-3xl font-bold text-primary">
                                        {promo.discount_type === 'percentage' ? `${promo.discount_value}%` : `$${promo.discount_value.toFixed(2)}`}
                                    </p>
                                    <p className="text-xs text-text-muted mt-1.5">
                                        {promo.discount_type === 'percentage' ? 'de descuento' : 'de descuento fijo'}
                                    </p>
                                </div>
                            </div>

                            <div className="space-y-3">
                                <div className="flex items-center gap-2 text-sm text-text-secondary">
                                    {promo.applies_to === 'category' ? <Tag size={14} className="text-text-muted" /> : <Package size={14} className="text-text-muted" />}
                                    <span>{getTargetLabel(promo)}</span>
                                </div>
                                <div className="flex items-center justify-between text-xs text-text-muted">
                                    <span>{promo.start_date.split('T')[0]} → {promo.end_date.split('T')[0]}</span>
                                    <span className={`px-3 py-1 rounded-xl text-[11px] font-semibold uppercase ${active ? 'bg-success/10 text-success' : 'bg-text-muted/10 text-text-muted'}`}>
                                        {active ? 'Activa' : promo.is_active ? 'Programada' : 'Inactiva'}
                                    </span>
                                </div>
                            </div>
                        </div>
                    );
                })}

                {promotions.length === 0 && (
                    <div className="col-span-full text-center py-20 text-text-muted">
                        <BadgePercent size={56} className="mx-auto mb-3 opacity-30" />
                        <p className="text-base">No hay promociones registradas</p>
                        {isAdmin && <p className="text-sm mt-1">Crea una nueva promoción para comenzar</p>}
                    </div>
                )}
            </div>

            {/* Create/Edit Modal */}
            {showForm && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
                    <div className="bg-bg-secondary border border-border rounded-2xl w-full max-w-2xl p-10 animate-fade-in max-h-[90vh] overflow-y-auto">
                        <div className="flex justify-between mb-8">
                            <h3 className="text-xl font-bold text-text-primary">{editing ? 'Editar' : 'Nueva'} Promoción</h3>
                            <button onClick={() => setShowForm(false)} className="text-text-muted hover:text-text-primary"><X size={22} /></button>
                        </div>
                        <div className="space-y-6">
                            <div>
                                <label className="text-sm font-medium text-text-secondary block mb-2">Nombre *</label>
                                <input type="text" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" placeholder="Ej: Descuento de Verano" />
                            </div>
                            <div>
                                <label className="text-sm font-medium text-text-secondary block mb-2">Descripción</label>
                                <input type="text" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" placeholder="Descripción opcional..." />
                            </div>
                            <div className="grid grid-cols-2 gap-5">
                                <div>
                                    <label className="text-sm font-medium text-text-secondary block mb-2">Tipo de Descuento</label>
                                    <select value={form.discount_type} onChange={(e) => setForm({ ...form, discount_type: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm">
                                        {DISCOUNT_TYPES.map(t => <option key={t.key} value={t.key}>{t.label}</option>)}
                                    </select>
                                </div>
                                <div>
                                    <label className="text-sm font-medium text-text-secondary block mb-2">Valor *</label>
                                    <input type="number" step="0.01" value={form.discount_value} onChange={(e) => setForm({ ...form, discount_value: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" placeholder={form.discount_type === 'percentage' ? '10' : '50.00'} />
                                </div>
                            </div>
                            <div className="grid grid-cols-2 gap-5">
                                <div>
                                    <label className="text-sm font-medium text-text-secondary block mb-2">Fecha Inicio</label>
                                    <input type="date" value={form.start_date} onChange={(e) => setForm({ ...form, start_date: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" />
                                </div>
                                <div>
                                    <label className="text-sm font-medium text-text-secondary block mb-2">Fecha Fin</label>
                                    <input type="date" value={form.end_date} onChange={(e) => setForm({ ...form, end_date: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" />
                                </div>
                            </div>
                            <div>
                                <label className="text-sm font-medium text-text-secondary block mb-2">Aplica a</label>
                                <select value={form.applies_to} onChange={(e) => setForm({ ...form, applies_to: e.target.value, target_id: '' })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm">
                                    {APPLIES_TO.map(a => <option key={a.key} value={a.key}>{a.label}</option>)}
                                </select>
                            </div>
                            {form.applies_to === 'category' && (
                                <div>
                                    <label className="text-sm font-medium text-text-secondary block mb-2">Categoría</label>
                                    <select value={form.target_id} onChange={(e) => setForm({ ...form, target_id: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm">
                                        <option value="">Seleccionar categoría...</option>
                                        {categories.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
                                    </select>
                                </div>
                            )}
                            {form.applies_to === 'product' && (
                                <div>
                                    <label className="text-sm font-medium text-text-secondary block mb-2">Producto</label>
                                    <select value={form.target_id} onChange={(e) => setForm({ ...form, target_id: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm">
                                        <option value="">Seleccionar producto...</option>
                                        {products.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                                    </select>
                                </div>
                            )}
                        </div>
                        <div className="flex gap-4 mt-8">
                            <button onClick={() => setShowForm(false)} className="flex-1 py-4 bg-bg-primary border border-border rounded-2xl text-text-secondary text-sm font-medium hover:bg-surface-hover transition-colors">Cancelar</button>
                            <button onClick={handleSave} disabled={processing} className="flex-1 py-4 bg-primary text-white rounded-2xl text-sm font-medium flex items-center justify-center gap-2">{processing && <Loader2 size={18} className="animate-spin" />}Guardar</button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
