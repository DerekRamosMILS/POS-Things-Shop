import { useEffect, useState } from 'react';
import { formatCurrency } from '../utils';
import * as api from '../api';
import type { Product, Category, CreateProductDto, UpdateProductDto, Notification } from '../types';
import { Plus, Search, Edit2, Trash2, X, Package, AlertTriangle, Loader2, BellPlus, Calendar } from 'lucide-react';

export default function ProductsPage() {
    const [products, setProducts] = useState<Product[]>([]);
    const [categories, setCategories] = useState<Category[]>([]);
    const [notifications, setNotifications] = useState<Notification[]>([]);
    const [loading, setLoading] = useState(true);
    const [search, setSearch] = useState('');
    const [filterCategory, setFilterCategory] = useState<number | ''>('');
    const [showLowStock, setShowLowStock] = useState(false);
    const [showForm, setShowForm] = useState(false);
    const [editingProduct, setEditingProduct] = useState<Product | null>(null);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState('');
    const [form, setForm] = useState<CreateProductDto & { id?: number; is_active?: boolean }>({
        sku: '', barcode: null, name: '', description: null,
        category_id: null, supplier_id: null, purchase_price: 0,
        sale_price: 0, stock: 0, min_stock: 5,
    });

    // Reminder state
    const [showReminder, setShowReminder] = useState(false);
    const [reminderDate, setReminderDate] = useState('');
    const [reminderProduct, setReminderProduct] = useState<Product | null>(null);

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [p, c, n] = await Promise.all([api.getProducts(), api.getCategories(), api.getNotifications()]);
            setProducts(p); setCategories(c); setNotifications(n);
        } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    const filtered = products.filter((p) => {
        if (search && !p.name.toLowerCase().includes(search.toLowerCase()) && !p.sku.toLowerCase().includes(search.toLowerCase())) return false;
        if (filterCategory && p.category_id !== filterCategory) return false;
        if (showLowStock && p.stock > p.min_stock) return false;
        return true;
    });

    const openCreateForm = () => {
        setEditingProduct(null);
        setForm({ sku: '', barcode: null, name: '', description: null, category_id: null, supplier_id: null, purchase_price: 0, sale_price: 0, stock: 0, min_stock: 5 });
        setShowForm(true); setError('');
    };

    const openEditForm = (product: Product) => {
        setEditingProduct(product);
        setForm({ id: product.id, sku: product.sku, barcode: product.barcode, name: product.name, description: product.description, category_id: product.category_id, supplier_id: product.supplier_id, purchase_price: product.purchase_price, sale_price: product.sale_price, stock: product.stock, min_stock: product.min_stock, is_active: product.is_active });
        setShowForm(true); setError('');
    };

    const handleSave = async () => {
        if (!form.name || !form.sku || form.sale_price <= 0) { setError('Nombre, SKU y precio de venta son requeridos'); return; }
        setSaving(true); setError('');
        try {
            if (editingProduct) { await api.updateProduct({ ...form, id: editingProduct.id, is_active: form.is_active ?? true } as UpdateProductDto); }
            else { await api.createProduct(form); }
            setShowForm(false); loadData();
        } catch (err) { setError(String(err)); } finally { setSaving(false); }
    };

    const openReminderForm = (product: Product) => {
        setReminderProduct(product);
        setReminderDate(new Date().toISOString().split('T')[0]); // Default today
        setShowReminder(true);
        setError('');
    };

    const handleSaveReminder = async () => {
        if (!reminderProduct || !reminderDate) { setError('Fecha requerida'); return; }
        setSaving(true);
        setError('');
        try {
            await api.createReminder({ product_id: reminderProduct.id, target_date: reminderDate });
            setShowReminder(false);
            loadData(); // Reload to show the new reminder in the table
            alert('Recordatorio creado exitosamente');
        } catch (err) {
            setError(String(err));
        } finally {
            setSaving(false);
        }
    };

    const handleDelete = async (id: number) => {
        if (!confirm('¿Desactivar este producto?')) return;
        try { await api.deleteProduct(id); loadData(); } catch (err) { alert(String(err)); }
    };

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 lg:p-14 xl:p-16 h-full flex flex-col animate-fade-in">
            <div className="flex items-center justify-between mb-10">
                <div>
                    <h1 className="text-4xl font-bold text-text-primary">Productos</h1>
                    <p className="text-text-secondary text-lg mt-2">{products.length} productos registrados</p>
                </div>
                <button onClick={openCreateForm} className="flex items-center gap-3 px-8 py-4 bg-primary hover:bg-primary-hover text-[#0B0B0F] rounded-2xl font-bold text-base transition-colors shadow-[0_4px_20px_rgba(0,224,90,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,224,90,0.4)]">
                    <Plus size={22} strokeWidth={2.5} /> Nuevo Producto
                </button>
            </div>

            {/* Filters */}
            <div className="flex gap-4 flex-wrap mb-8">
                <div className="flex-1 min-w-[320px] relative">
                    <Search className="absolute left-6 top-1/2 -translate-y-1/2 text-text-muted" size={28} />
                    <input type="text" value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Buscar por nombre o SKU..." className="w-full pl-[72px] pr-6 py-5 lg:py-6 bg-bg-secondary border border-border rounded-2xl text-text-primary text-xl placeholder-text-muted focus:border-primary transition-colors focus:shadow-[0_0_15px_rgba(0,224,90,0.15)] shadow-[inset_0_2px_10px_rgba(0,0,0,0.2)]" />
                </div>
                <select value={filterCategory} onChange={(e) => setFilterCategory(e.target.value ? Number(e.target.value) : '')} className="px-6 py-5 lg:py-6 bg-bg-secondary border border-border rounded-2xl text-text-primary text-xl focus:border-primary transition-colors min-w-[240px]">
                    <option value="">Todas las categorías</option>
                    {categories.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
                </select>
                <button onClick={() => setShowLowStock(!showLowStock)} className={`flex items-center gap-3 px-8 py-5 lg:py-6 rounded-2xl text-xl font-bold transition-all border ${showLowStock ? 'bg-warning/10 border-warning text-warning shadow-[0_4px_20px_rgba(245,158,11,0.25)]' : 'bg-bg-secondary border-border text-text-secondary hover:bg-surface-hover hover:border-border-light'}`}>
                    <AlertTriangle size={24} /> Stock Bajo
                </button>
            </div>

            {/* Table */}
            <div className="glass rounded-[24px] border border-border flex-1 flex flex-col overflow-hidden">
                <div className="overflow-auto flex-1 p-2">
                    <table className="w-full border-collapse">
                        <thead>
                            <tr className="border-b border-border/60">
                                <th className="text-left text-base font-bold tracking-wide text-text-muted uppercase px-8 py-6">Producto</th>
                                <th className="text-left text-base font-bold tracking-wide text-text-muted uppercase px-8 py-6">SKU</th>
                                <th className="text-left text-base font-bold tracking-wide text-text-muted uppercase px-8 py-6">Categoría</th>
                                <th className="text-right text-base font-bold tracking-wide text-text-muted uppercase px-8 py-6">Precio</th>
                                <th className="text-right text-base font-bold tracking-wide text-text-muted uppercase px-8 py-6">Stock</th>
                                <th className="text-center text-base font-bold tracking-wide text-text-muted uppercase px-8 py-6">Estado</th>
                                <th className="text-center text-base font-bold tracking-wide text-text-muted uppercase px-8 py-6">Acciones</th>
                            </tr>
                        </thead>
                        <tbody>
                            {filtered.map((p) => (
                                <tr key={p.id} className="border-b border-border/30 hover:bg-white/5 transition-colors">
                                    <td className="px-8 py-6">
                                        <p className="text-xl font-bold text-text-primary">{p.name}</p>
                                        {p.barcode && <p className="text-base text-text-muted mt-1 font-mono tracking-wider">{p.barcode}</p>}
                                        {notifications.filter(n => n.product_id === p.id && n.notification_type === 'restock_reminder').map(n => (
                                            <div key={n.id} className="mt-3 flex items-center gap-2 text-xs font-bold text-primary bg-primary/10 inline-flex px-3 py-1.5 rounded-lg border border-primary/20">
                                                <Calendar size={14} /> Recordatorio para: {n.target_date}
                                            </div>
                                        ))}
                                    </td>
                                    <td className="px-8 py-6 text-xl text-text-secondary font-mono tracking-wider">{p.sku}</td>
                                    <td className="px-8 py-6 text-xl text-text-secondary">{p.category_name || '—'}</td>
                                    <td className="px-8 py-6 text-right">
                                        <p className="text-2xl font-black text-primary drop-shadow-[0_0_8px_rgba(0,224,90,0.2)]">{formatCurrency(p.sale_price)}</p>
                                        <p className="text-base text-text-muted mt-0.5">Costo: {formatCurrency(p.purchase_price)}</p>
                                    </td>
                                    <td className="px-8 py-6 text-right">
                                        <span className={`text-2xl font-black px-5 py-2.5 rounded-xl border border-transparent ${p.stock <= p.min_stock ? (p.stock === 0 ? 'text-danger bg-danger/10 border-danger/30 shadow-[0_0_15px_rgba(244,63,94,0.2)]' : 'text-warning bg-warning/10 border-warning/30 shadow-[0_0_15px_rgba(245,158,11,0.2)]') : 'text-success'}`}>{p.stock}</span>
                                    </td>
                                    <td className="px-8 py-6 text-center">
                                        <span className={`text-base font-bold px-5 py-2.5 rounded-xl ${p.is_active ? 'bg-success/10 text-success border border-success/30' : 'bg-danger/10 text-danger border border-danger/30'}`}>{p.is_active ? 'Activo' : 'Inactivo'}</span>
                                    </td>
                                    <td className="px-8 py-6">
                                        <div className="flex items-center justify-center gap-4">
                                            <button onClick={() => openEditForm(p)} title="Editar" className="p-4 text-text-muted hover:text-primary hover:bg-primary/10 rounded-xl border border-transparent hover:border-primary/30 transition-all hover:scale-110 shadow-[0_4px_10px_rgba(0,0,0,0.1)]"><Edit2 size={24} /></button>
                                            <button onClick={() => openReminderForm(p)} title="Recordatorio" className="p-4 text-text-muted hover:text-accent hover:bg-accent/10 rounded-xl border border-transparent hover:border-accent/30 transition-all hover:scale-110 shadow-[0_4px_10px_rgba(0,0,0,0.1)]"><BellPlus size={24} /></button>
                                            <button onClick={() => handleDelete(p.id)} title="Desactivar" className="p-4 text-text-muted hover:text-danger hover:bg-danger/10 rounded-xl border border-transparent hover:border-danger/30 transition-all hover:scale-110 shadow-[0_4px_10px_rgba(0,0,0,0.1)]"><Trash2 size={24} /></button>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {filtered.length === 0 && (
                        <div className="py-24 text-center text-text-muted"><Package size={72} strokeWidth={1.5} className="mx-auto mb-5 opacity-30" /><p className="text-xl font-medium tracking-wide">No se encontraron productos</p></div>
                    )}
                </div>
            </div>

            {/* Product Form Modal */}
            {showForm && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
                    <div className="bg-bg-secondary border border-border rounded-2xl w-full max-w-2xl p-10 max-h-[90vh] overflow-y-auto animate-fade-in">
                        <div className="flex items-center justify-between mb-8">
                            <h3 className="text-xl font-bold text-text-primary">{editingProduct ? 'Editar Producto' : 'Nuevo Producto'}</h3>
                            <button onClick={() => setShowForm(false)} className="text-text-muted hover:text-text-primary"><X size={22} /></button>
                        </div>

                        <div className="space-y-6">
                            <div className="grid grid-cols-2 gap-5">
                                <div>
                                    <label className="text-sm font-medium text-text-secondary mb-2 block">Nombre *</label>
                                    <input type="text" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm focus:border-primary" />
                                </div>
                                <div>
                                    <label className="text-sm font-medium text-text-secondary mb-2 block">SKU *</label>
                                    <input type="text" value={form.sku} onChange={(e) => setForm({ ...form, sku: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm focus:border-primary" />
                                </div>
                            </div>

                            <div>
                                <label className="text-sm font-medium text-text-secondary mb-2 block">Código de Barras</label>
                                <input type="text" value={form.barcode || ''} onChange={(e) => setForm({ ...form, barcode: e.target.value || null })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm focus:border-primary" placeholder="EAN-13 / CODE-128" />
                            </div>

                            <div>
                                <label className="text-sm font-medium text-text-secondary mb-2 block">Descripción</label>
                                <textarea value={form.description || ''} onChange={(e) => setForm({ ...form, description: e.target.value || null })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm focus:border-primary resize-none" rows={2} />
                            </div>

                            <div>
                                <label className="text-sm font-medium text-text-secondary mb-2 block">Categoría</label>
                                <select value={form.category_id || ''} onChange={(e) => setForm({ ...form, category_id: e.target.value ? Number(e.target.value) : null })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm focus:border-primary">
                                    <option value="">Sin categoría</option>
                                    {categories.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
                                </select>
                            </div>

                            <div className="grid grid-cols-2 gap-5">
                                <div>
                                    <label className="text-sm font-medium text-text-secondary mb-2 block">Precio de Compra</label>
                                    <input type="number" step="0.01" value={form.purchase_price} onChange={(e) => setForm({ ...form, purchase_price: parseFloat(e.target.value) || 0 })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm focus:border-primary" />
                                </div>
                                <div>
                                    <label className="text-sm font-medium text-text-secondary mb-2 block">Precio de Venta *</label>
                                    <input type="number" step="0.01" value={form.sale_price} onChange={(e) => setForm({ ...form, sale_price: parseFloat(e.target.value) || 0 })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm focus:border-primary" />
                                </div>
                            </div>

                            <div className="grid grid-cols-2 gap-5">
                                <div>
                                    <label className="text-sm font-medium text-text-secondary mb-2 block">Stock Inicial</label>
                                    <input type="number" value={form.stock} onChange={(e) => setForm({ ...form, stock: parseInt(e.target.value) || 0 })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm focus:border-primary" disabled={!!editingProduct} />
                                </div>
                                <div>
                                    <label className="text-sm font-medium text-text-secondary mb-2 block">Stock Mínimo</label>
                                    <input type="number" value={form.min_stock} onChange={(e) => setForm({ ...form, min_stock: parseInt(e.target.value) || 0 })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm focus:border-primary" />
                                </div>
                            </div>

                            {error && <div className="bg-danger/10 border border-danger/20 text-danger rounded-2xl px-5 py-4 text-sm">{error}</div>}

                            <div className="flex gap-4 pt-2">
                                <button onClick={() => setShowForm(false)} className="flex-1 py-4 bg-bg-primary border border-border rounded-2xl text-text-secondary font-medium text-sm hover:bg-surface-hover transition-colors">Cancelar</button>
                                <button onClick={handleSave} disabled={saving} className="flex-1 py-4 bg-primary hover:bg-primary-hover text-white rounded-2xl font-medium text-sm transition-colors flex items-center justify-center gap-2 disabled:opacity-50">
                                    {saving ? <Loader2 size={18} className="animate-spin" /> : null}
                                    {editingProduct ? 'Guardar Cambios' : 'Crear Producto'}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {/* Reminder Form Modal */}
            {showReminder && reminderProduct && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-[60] animate-fade-in p-4">
                    <div className="bg-bg-secondary border border-border rounded-3xl w-full max-w-lg p-10 animate-fade-in shadow-[0_20px_60px_rgba(0,0,0,0.8)]">
                        <div className="flex items-center justify-between mb-8">
                            <h3 className="text-2xl font-bold text-text-primary">Crear Recordatorio</h3>
                            <button onClick={() => setShowReminder(false)} className="text-text-muted hover:text-white transition-colors">
                                <X size={28} />
                            </button>
                        </div>
                        <div className="space-y-6">
                            <div>
                                <p className="text-sm font-medium text-text-secondary uppercase tracking-wider mb-2">Producto</p>
                                <p className="text-xl font-bold text-primary">{reminderProduct.name}</p>
                            </div>
                            <div>
                                <label className="text-sm font-medium text-text-secondary uppercase tracking-wider mb-2 block">Fecha de Compra</label>
                                <input
                                    type="date"
                                    value={reminderDate}
                                    onChange={(e) => setReminderDate(e.target.value)}
                                    className="w-full px-6 py-5 bg-bg-primary border border-border rounded-xl text-text-primary text-xl font-mono focus:border-primary focus:shadow-[0_0_15px_rgba(0,224,90,0.15)] transition-all"
                                />
                            </div>

                            {error && <div className="bg-danger/10 border border-danger/20 text-danger rounded-xl px-5 py-4 text-sm font-medium">{error}</div>}

                            <div className="flex gap-4 pt-6">
                                <button onClick={() => setShowReminder(false)} className="flex-1 py-5 bg-bg-primary border-2 border-border rounded-xl text-text-secondary font-bold text-lg hover:bg-surface-hover hover:border-border-light transition-colors">
                                    Cancelar
                                </button>
                                <button onClick={handleSaveReminder} disabled={saving} className="flex-1 py-5 bg-primary hover:bg-primary-hover text-[#0B0B0F] rounded-xl font-black text-lg transition-all shadow-[0_4px_20px_rgba(0,224,90,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,224,90,0.4)] disabled:opacity-50 disabled:translate-y-0 disabled:shadow-none flex items-center justify-center gap-3">
                                    {saving && <Loader2 size={24} className="animate-spin" />} Guardar
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
