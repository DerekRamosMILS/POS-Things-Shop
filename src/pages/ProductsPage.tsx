import { useEffect, useRef, useState } from 'react';
import { formatCurrency } from '../utils';
import * as api from '../api';
import type { Product, Category, CreateProductDto, UpdateProductDto, Notification, Supplier } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';

// ─── Gradient avatar helper ──────────────────────────────────────────────────
const GRAD_PAIRS: [string, string][] = [
    ['#8B78F5','#F0C547'],['#F45270','#F0C547'],['#22D3A0','#8B78F5'],
    ['#F5A842','#F45270'],['#8B78F5','#22D3A0'],['#F0C547','#22D3A0'],
    ['#F45270','#8B78F5'],['#22D3A0','#F5A842'],
];
function getGrad(name: string): [string, string] {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0;
    return GRAD_PAIRS[h % GRAD_PAIRS.length];
}
function initials(name: string) {
    const parts = name.trim().split(/\s+/);
    return parts.length > 1
        ? (parts[0][0] + parts[1][0]).toUpperCase()
        : name.slice(0, 2).toUpperCase();
}

// ─── Product thumbnail (table + form preview) ───────────────────────────────
function ProductAvatar({ product, size = 40, radius = 10 }: { product: Product; size?: number; radius?: number }) {
    const [err, setErr] = useState(false);
    const [a, b] = getGrad(product.name);
    if (product.image_url && !err) {
        return (
            <img
                src={product.image_url}
                alt={product.name}
                onError={() => setErr(true)}
                style={{ width: size, height: size, borderRadius: radius, objectFit: 'cover', flexShrink: 0 }}
            />
        );
    }
    return (
        <div style={{
            width: size, height: size, borderRadius: radius, flexShrink: 0,
            background: `linear-gradient(135deg,${a},${b})`,
            display: 'grid', placeItems: 'center',
            color: '#fff', fontWeight: 800, fontSize: size * 0.3,
        }}>
            {initials(product.name)}
        </div>
    );
}

// ─── Inline SVG icons ────────────────────────────────────────────────────────
const IcoPlus = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>;
const IcoSearch = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>;
const IcoEdit = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>;
const IcoTrash = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4h6v2"/></svg>;
const IcoX = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoBell = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/><line x1="12" y1="2" x2="12" y2="3"/></svg>;
const IcoAlert = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>;
const IcoFilter = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/></svg>;
const IcoCamera = () => <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z"/><circle cx="12" cy="13" r="4"/></svg>;
const IcoLoader = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;
const IcoCalendar = () => <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg>;
const IcoPackage = () => <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" className="opacity-40"><path d="M16.5 9.4l-9-5.19"/><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg>;

// ─── Icon button ─────────────────────────────────────────────────────────────
function IcoBtn({ onClick, title, children, hoverColor = '#8B78F5', hoverBg = 'rgba(139,120,245,0.10)' }: {
    onClick: () => void; title: string; children: React.ReactNode;
    hoverColor?: string; hoverBg?: string;
}) {
    return (
        <button
            onClick={onClick}
            title={title}
            className="p-2 rounded-lg transition-colors"
            style={{ color: '#7580A0' }}
            onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.color = hoverColor; (e.currentTarget as HTMLButtonElement).style.background = hoverBg; }}
            onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.color = '#7580A0'; (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
        >{children}</button>
    );
}

// ─── Main component ───────────────────────────────────────────────────────────
export default function ProductsPage() {
    const [products, setProducts] = useState<Product[]>([]);
    const [categories, setCategories] = useState<Category[]>([]);
    const [suppliers, setSuppliers] = useState<Supplier[]>([]);
    const [notifications, setNotifications] = useState<Notification[]>([]);
    const [loading, setLoading] = useState(true);
    const [search, setSearch] = useState('');
    const [filterCategory, setFilterCategory] = useState<number | ''>('');
    const [filterSupplier, setFilterSupplier] = useState<number | ''>('');
    const [filterStatus, setFilterStatus] = useState<'all' | 'active' | 'inactive'>('all');
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

    // Photo state
    const [photoPreview, setPhotoPreview] = useState<string | null>(null); // current preview URL
    const [photoChanged, setPhotoChanged] = useState(false);               // whether user changed photo
    const fileInputRef = useRef<HTMLInputElement>(null);

    const [showReminder, setShowReminder] = useState(false);
    const [reminderDate, setReminderDate] = useState('');
    const [reminderProduct, setReminderProduct] = useState<Product | null>(null);

    const { showToast } = useToast();
    const { confirm } = useConfirm();

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [p, c, s, n] = await Promise.all([api.getProducts(), api.getCategories(), api.getSuppliers(), api.getNotifications()]);
            setProducts(p); setCategories(c); setSuppliers(s); setNotifications(n);
        } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    const filtered = products.filter((p) => {
        const q = search.toLowerCase().trim();
        if (q && ![p.name, p.sku, p.barcode || '', p.category_name || '', p.supplier_name || ''].some(v => v.toLowerCase().includes(q))) return false;
        if (filterCategory && p.category_id !== filterCategory) return false;
        if (filterSupplier && p.supplier_id !== filterSupplier) return false;
        if (filterStatus === 'active' && !p.is_active) return false;
        if (filterStatus === 'inactive' && p.is_active) return false;
        if (showLowStock && p.stock > p.min_stock) return false;
        return true;
    });

    const inventoryValue = filtered.reduce((sum, p) => sum + p.stock * p.purchase_price, 0);
    const potentialRevenue = filtered.reduce((sum, p) => sum + p.stock * p.sale_price, 0);
    const lowStockCount = filtered.filter(p => p.stock <= p.min_stock).length;

    const openCreateForm = () => {
        setEditingProduct(null);
        setForm({ sku: '', barcode: null, name: '', description: null, category_id: null, supplier_id: null, purchase_price: 0, sale_price: 0, stock: 0, min_stock: 5 });
        setPhotoPreview(null);
        setPhotoChanged(false);
        setShowForm(true); setError('');
    };

    const openEditForm = (product: Product) => {
        setEditingProduct(product);
        setForm({ id: product.id, sku: product.sku, barcode: product.barcode, name: product.name, description: product.description, category_id: product.category_id, supplier_id: product.supplier_id, purchase_price: product.purchase_price, sale_price: product.sale_price, stock: product.stock, min_stock: product.min_stock, is_active: product.is_active });
        setPhotoPreview(product.image_url ?? null);
        setPhotoChanged(false);
        setShowForm(true); setError('');
    };

    const handlePhotoSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = () => {
            setPhotoPreview(reader.result as string);
            setPhotoChanged(true);
        };
        reader.readAsDataURL(file);
        // Reset input so same file can be re-selected
        e.target.value = '';
    };

    const handleRemovePhoto = () => {
        setPhotoPreview(null);
        setPhotoChanged(true);
    };

    const handleSave = async () => {
        if (!form.name || !form.sku || form.sale_price <= 0) {
            setError('Nombre, SKU y precio de venta son requeridos');
            return;
        }
        setSaving(true); setError('');
        try {
            let productId: number;
            if (editingProduct) {
                const updated = await api.updateProduct({ ...form, id: editingProduct.id, is_active: form.is_active ?? true } as UpdateProductDto);
                productId = updated.id;
            } else {
                const created = await api.createProduct(form);
                productId = created.id;
            }
            // Save photo if changed
            if (photoChanged) {
                await api.setProductImage(productId, photoPreview);
            }
            showToast(editingProduct ? 'Producto actualizado' : 'Producto creado', 'success');
            setShowForm(false);
            loadData();
        } catch (err) { setError(String(err)); } finally { setSaving(false); }
    };

    const openReminderForm = (product: Product) => {
        setReminderProduct(product);
        setReminderDate(new Date().toISOString().split('T')[0]);
        setShowReminder(true); setError('');
    };

    const handleSaveReminder = async () => {
        if (!reminderProduct || !reminderDate) { setError('Fecha requerida'); return; }
        setSaving(true); setError('');
        try {
            await api.createReminder({ product_id: reminderProduct.id, target_date: reminderDate });
            showToast('Recordatorio creado', 'success');
            setShowReminder(false); loadData();
        } catch (err) { setError(String(err)); } finally { setSaving(false); }
    };

    const handleDelete = async (id: number) => {
        const ok = await confirm({ title: 'Desactivar producto', message: '¿Desactivar este producto? Podrás reactivarlo más tarde.', variant: 'warning', confirmLabel: 'Desactivar' });
        if (!ok) return;
        try { await api.deleteProduct(id); loadData(); } catch (err) { showToast(String(err), 'error'); }
    };

    if (loading) return (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
            <IcoLoader />
        </div>
    );

    const hasFilters = !!(search || filterCategory || filterSupplier || filterStatus !== 'all' || showLowStock);

    return (
        <div className="page-container">
            {/* Header */}
            <div className="page-header">
                <div>
                    <h1 className="page-title">Productos</h1>
                    <p className="page-subtitle">{filtered.length} de {products.length} productos · inventario {formatCurrency(inventoryValue)}</p>
                </div>
                <button onClick={openCreateForm} className="btn btn-primary">
                    <IcoPlus /> Nuevo Producto
                </button>
            </div>

            {/* KPI cards */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 16 }}>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Valor de costo</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(inventoryValue)}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Venta potencial</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--accent)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(potentialRevenue)}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Stock bajo</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: lowStockCount > 0 ? 'var(--warning)' : 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{lowStockCount}</p>
                </div>
            </div>

            {/* Filters */}
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
                <div style={{ flex: 1, minWidth: 240, position: 'relative' }}>
                    <span style={{ position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--t3)', pointerEvents: 'none' }}>
                        <IcoSearch />
                    </span>
                    <input
                        type="text"
                        value={search}
                        onChange={e => setSearch(e.target.value)}
                        placeholder="Buscar por nombre, SKU, barcode, proveedor…"
                        className="input"
                        style={{ paddingLeft: 36 }}
                    />
                </div>
                <select value={filterCategory} onChange={e => setFilterCategory(e.target.value ? Number(e.target.value) : '')} className="input" style={{ width: 'auto', minWidth: 160 }}>
                    <option value="">Todas las categorías</option>
                    {categories.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
                </select>
                <select value={filterSupplier} onChange={e => setFilterSupplier(e.target.value ? Number(e.target.value) : '')} className="input" style={{ width: 'auto', minWidth: 160 }}>
                    <option value="">Todos los proveedores</option>
                    {suppliers.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}
                </select>
                <select value={filterStatus} onChange={e => setFilterStatus(e.target.value as 'all' | 'active' | 'inactive')} className="input" style={{ width: 'auto', minWidth: 130 }}>
                    <option value="all">Todos</option>
                    <option value="active">Activos</option>
                    <option value="inactive">Inactivos</option>
                </select>
                <button
                    onClick={() => setShowLowStock(!showLowStock)}
                    className={`btn btn-sm ${showLowStock ? 'btn-danger' : 'btn-ghost'}`}
                    style={{ gap: 6 }}
                >
                    <IcoAlert /> Stock bajo
                </button>
                {hasFilters && (
                    <button
                        onClick={() => { setSearch(''); setFilterCategory(''); setFilterSupplier(''); setFilterStatus('all'); setShowLowStock(false); }}
                        className="btn btn-sm btn-ghost"
                        style={{ gap: 6 }}
                    >
                        <IcoFilter /> Limpiar
                    </button>
                )}
            </div>

            {/* Table */}
            <div className="card" style={{ flex: 1, padding: 0, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
                <div style={{ overflowX: 'auto', overflowY: 'auto', flex: 1 }}>
                    <table className="table-base">
                        <thead>
                            <tr>
                                <th style={{ width: 52 }}></th>
                                <th>Producto</th>
                                <th>SKU</th>
                                <th>Categoría</th>
                                <th>Proveedor</th>
                                <th style={{ textAlign: 'right' }}>Precio</th>
                                <th style={{ textAlign: 'right' }}>Stock</th>
                                <th style={{ textAlign: 'center' }}>Estado</th>
                                <th style={{ textAlign: 'center', width: 120 }}>Acciones</th>
                            </tr>
                        </thead>
                        <tbody>
                            {filtered.map(p => (
                                <tr key={p.id}>
                                    {/* Avatar / photo thumbnail */}
                                    <td style={{ paddingRight: 0 }}>
                                        <ProductAvatar product={p} size={38} radius={10} />
                                    </td>
                                    <td>
                                        <p style={{ fontWeight: 600, color: 'var(--t1)', fontSize: 13 }}>{p.name}</p>
                                        {p.barcode && (
                                            <p style={{ fontSize: 11, fontFamily: 'monospace', color: 'var(--t3)', marginTop: 2 }}>{p.barcode}</p>
                                        )}
                                        {notifications.filter(n => n.product_id === p.id && n.notification_type === 'restock_reminder').map(n => (
                                            <span key={n.id} className="badge badge-primary" style={{ marginTop: 4, gap: 4 }}>
                                                <IcoCalendar /> {n.target_date}
                                            </span>
                                        ))}
                                    </td>
                                    <td style={{ fontFamily: 'monospace', fontSize: 12, color: 'var(--t2)' }}>{p.sku}</td>
                                    <td style={{ color: 'var(--t2)', fontSize: 13 }}>{p.category_name || '—'}</td>
                                    <td style={{ color: 'var(--t2)', fontSize: 13 }}>{p.supplier_name || '—'}</td>
                                    <td style={{ textAlign: 'right' }}>
                                        <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--primary)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(p.sale_price)}</p>
                                        <p style={{ fontSize: 11, color: 'var(--t3)', fontVariantNumeric: 'tabular-nums' }}>Costo: {formatCurrency(p.purchase_price)}</p>
                                    </td>
                                    <td style={{ textAlign: 'right' }}>
                                        <span className={`badge ${p.stock === 0 ? 'badge-danger' : p.stock <= p.min_stock ? 'badge-warning' : 'badge-success'}`}>
                                            {p.stock} ud.
                                        </span>
                                    </td>
                                    <td style={{ textAlign: 'center' }}>
                                        <span className={`badge ${p.is_active ? 'badge-success' : 'badge-muted'}`}>
                                            {p.is_active ? 'Activo' : 'Inactivo'}
                                        </span>
                                    </td>
                                    <td>
                                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 2 }}>
                                            <IcoBtn onClick={() => openEditForm(p)} title="Editar" hoverColor="var(--primary)" hoverBg="rgba(139,120,245,0.10)">
                                                <IcoEdit />
                                            </IcoBtn>
                                            <IcoBtn onClick={() => openReminderForm(p)} title="Recordatorio" hoverColor="var(--accent)" hoverBg="rgba(240,197,71,0.10)">
                                                <IcoBell />
                                            </IcoBtn>
                                            <IcoBtn onClick={() => handleDelete(p.id)} title="Desactivar" hoverColor="var(--danger)" hoverBg="rgba(244,82,112,0.10)">
                                                <IcoTrash />
                                            </IcoBtn>
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {filtered.length === 0 && (
                        <div style={{ padding: '64px 0', textAlign: 'center', color: 'var(--t3)' }}>
                            <IcoPackage />
                            <p style={{ marginTop: 12, fontSize: 13 }}>No se encontraron productos</p>
                        </div>
                    )}
                </div>
            </div>

            {/* ── Product Form Modal ── */}
            {showForm && (
                <div className="modal-overlay" onClick={() => setShowForm(false)}>
                    <div
                        className="glass-modal animate-scale-in"
                        style={{ width: '100%', maxWidth: 560, maxHeight: '92vh', overflowY: 'auto' }}
                        onClick={e => e.stopPropagation()}
                    >
                        {/* Modal header */}
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '20px 24px', borderBottom: '1px solid rgba(255,255,255,0.07)' }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>
                                {editingProduct ? 'Editar Producto' : 'Nuevo Producto'}
                            </h3>
                            <button
                                onClick={() => setShowForm(false)}
                                style={{ padding: 6, borderRadius: 9, color: 'var(--t3)', transition: 'all 0.15s' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t1)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; }}
                            ><IcoX /></button>
                        </div>

                        <div style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 18 }}>

                            {/* ── PHOTO UPLOAD ── */}
                            <div>
                                <label className="form-label">Foto del producto</label>
                                <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
                                    {/* Preview area */}
                                    <div
                                        onClick={() => fileInputRef.current?.click()}
                                        style={{
                                            width: 96, height: 96, borderRadius: 16, flexShrink: 0, cursor: 'pointer',
                                            background: photoPreview ? 'transparent' : 'rgba(255,255,255,0.04)',
                                            border: `2px dashed ${photoPreview ? 'rgba(139,120,245,0.4)' : 'rgba(255,255,255,0.12)'}`,
                                            display: 'flex', alignItems: 'center', justifyContent: 'center',
                                            overflow: 'hidden', transition: 'border-color 0.15s',
                                            position: 'relative',
                                        }}
                                        onMouseEnter={e => { (e.currentTarget as HTMLDivElement).style.borderColor = 'rgba(139,120,245,0.7)'; }}
                                        onMouseLeave={e => { (e.currentTarget as HTMLDivElement).style.borderColor = photoPreview ? 'rgba(139,120,245,0.4)' : 'rgba(255,255,255,0.12)'; }}
                                    >
                                        {photoPreview ? (
                                            <img src={photoPreview} alt="preview" style={{ width: '100%', height: '100%', objectFit: 'cover' }} />
                                        ) : (
                                            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6, color: 'var(--t3)' }}>
                                                <IcoCamera />
                                                <span style={{ fontSize: 10, fontWeight: 600 }}>Subir foto</span>
                                            </div>
                                        )}
                                    </div>
                                    {/* Actions */}
                                    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                                        <button
                                            type="button"
                                            onClick={() => fileInputRef.current?.click()}
                                            className="btn btn-ghost btn-sm"
                                            style={{ justifyContent: 'flex-start', gap: 7 }}
                                        >
                                            <IcoCamera /> {photoPreview ? 'Cambiar foto' : 'Seleccionar foto'}
                                        </button>
                                        {photoPreview && (
                                            <button
                                                type="button"
                                                onClick={handleRemovePhoto}
                                                className="btn btn-sm btn-danger"
                                                style={{ justifyContent: 'flex-start', gap: 7 }}
                                            >
                                                <IcoTrash /> Eliminar foto
                                            </button>
                                        )}
                                        <p style={{ fontSize: 11, color: 'var(--t3)', lineHeight: 1.4 }}>
                                            JPG, PNG, WEBP · máx. 5 MB<br />Se mostrará en el POS
                                        </p>
                                    </div>
                                    <input
                                        ref={fileInputRef}
                                        type="file"
                                        accept="image/*"
                                        onChange={handlePhotoSelect}
                                        style={{ display: 'none' }}
                                    />
                                </div>
                            </div>

                            <div style={{ height: 1, background: 'rgba(255,255,255,0.06)' }} />

                            {/* Name + SKU */}
                            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                                <div>
                                    <label className="form-label">Nombre *</label>
                                    <input type="text" value={form.name} onChange={e => setForm({ ...form, name: e.target.value })} className="input" placeholder="Nombre del producto" />
                                </div>
                                <div>
                                    <label className="form-label">SKU *</label>
                                    <input type="text" value={form.sku} onChange={e => setForm({ ...form, sku: e.target.value })} className="input" placeholder="ABC-001" />
                                </div>
                            </div>

                            {/* Barcode */}
                            <div>
                                <label className="form-label">Código de Barras</label>
                                <input type="text" value={form.barcode || ''} onChange={e => setForm({ ...form, barcode: e.target.value || null })} className="input" placeholder="EAN-13 / CODE-128" />
                            </div>

                            {/* Description */}
                            <div>
                                <label className="form-label">Descripción</label>
                                <textarea value={form.description || ''} onChange={e => setForm({ ...form, description: e.target.value || null })} className="input" style={{ resize: 'none', height: 68 }} placeholder="Descripción opcional…" />
                            </div>

                            {/* Category + Supplier */}
                            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                                <div>
                                    <label className="form-label">Categoría</label>
                                    <select value={form.category_id || ''} onChange={e => setForm({ ...form, category_id: e.target.value ? Number(e.target.value) : null })} className="input">
                                        <option value="">Sin categoría</option>
                                        {categories.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
                                    </select>
                                </div>
                                <div>
                                    <label className="form-label">Proveedor</label>
                                    <select value={form.supplier_id || ''} onChange={e => setForm({ ...form, supplier_id: e.target.value ? Number(e.target.value) : null })} className="input">
                                        <option value="">Sin proveedor</option>
                                        {suppliers.filter(s => s.is_active).map(s => <option key={s.id} value={s.id}>{s.name}</option>)}
                                    </select>
                                </div>
                            </div>

                            {/* Prices */}
                            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                                <div>
                                    <label className="form-label">Precio de Compra</label>
                                    <input type="number" step="0.01" min="0" value={form.purchase_price} onChange={e => setForm({ ...form, purchase_price: parseFloat(e.target.value) || 0 })} className="input" />
                                </div>
                                <div>
                                    <label className="form-label">Precio de Venta *</label>
                                    <input type="number" step="0.01" min="0" value={form.sale_price} onChange={e => setForm({ ...form, sale_price: parseFloat(e.target.value) || 0 })} className="input" />
                                </div>
                            </div>

                            {/* Stock */}
                            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                                <div>
                                    <label className="form-label">Stock Inicial</label>
                                    <input type="number" min="0" value={form.stock} onChange={e => setForm({ ...form, stock: parseInt(e.target.value) || 0 })} className="input" disabled={!!editingProduct} />
                                    {editingProduct && <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 4 }}>Usa ajuste de inventario para cambiar stock</p>}
                                </div>
                                <div>
                                    <label className="form-label">Stock Mínimo</label>
                                    <input type="number" min="0" value={form.min_stock} onChange={e => setForm({ ...form, min_stock: parseInt(e.target.value) || 0 })} className="input" />
                                </div>
                            </div>

                            {/* Active toggle (edit only) */}
                            {editingProduct && (
                                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 14px', borderRadius: 12, background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.07)' }}>
                                    <div>
                                        <p style={{ fontSize: 13, fontWeight: 600, color: 'var(--t1)' }}>Estado del producto</p>
                                        <p style={{ fontSize: 11, color: 'var(--t3)' }}>Los productos inactivos no aparecen en el POS</p>
                                    </div>
                                    <button
                                        type="button"
                                        onClick={() => setForm({ ...form, is_active: !form.is_active })}
                                        style={{
                                            width: 48, height: 26, borderRadius: 100, cursor: 'pointer', transition: 'background 0.2s',
                                            background: form.is_active ? 'var(--success)' : 'rgba(255,255,255,0.12)',
                                            position: 'relative', flexShrink: 0, border: 'none',
                                        }}
                                    >
                                        <span style={{
                                            position: 'absolute', top: 3, left: form.is_active ? 24 : 3,
                                            width: 20, height: 20, borderRadius: '50%', background: '#fff',
                                            transition: 'left 0.2s', display: 'block',
                                        }} />
                                    </button>
                                </div>
                            )}

                            {/* Error */}
                            {error && (
                                <div style={{ padding: '10px 14px', borderRadius: 10, background: 'rgba(244,82,112,0.08)', border: '1px solid rgba(244,82,112,0.2)', color: 'var(--danger)', fontSize: 13 }}>
                                    {error}
                                </div>
                            )}

                            {/* Actions */}
                            <div style={{ display: 'flex', gap: 12, paddingTop: 4 }}>
                                <button type="button" onClick={() => setShowForm(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button type="button" onClick={handleSave} disabled={saving} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>
                                    {saving && <IcoLoader />}
                                    {editingProduct ? 'Guardar Cambios' : 'Crear Producto'}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {/* ── Reminder Modal ── */}
            {showReminder && reminderProduct && (
                <div className="modal-overlay" style={{ zIndex: 120 }} onClick={() => setShowReminder(false)}>
                    <div
                        className="glass-modal animate-scale-in"
                        style={{ width: '100%', maxWidth: 380, padding: 24 }}
                        onClick={e => e.stopPropagation()}
                    >
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>Crear Recordatorio</h3>
                            <button
                                onClick={() => setShowReminder(false)}
                                style={{ padding: 6, borderRadius: 9, color: 'var(--t3)', transition: 'all 0.15s' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t1)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; }}
                            ><IcoX /></button>
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                            <div style={{ padding: '12px 14px', borderRadius: 12, background: 'rgba(139,120,245,0.08)', border: '1px solid rgba(139,120,245,0.2)' }}>
                                <p style={{ fontSize: 11, color: 'var(--t3)', marginBottom: 2 }}>Producto</p>
                                <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--primary)' }}>{reminderProduct.name}</p>
                            </div>
                            <div>
                                <label className="form-label">Fecha de Compra</label>
                                <input type="date" value={reminderDate} onChange={e => setReminderDate(e.target.value)} className="input" />
                            </div>
                            {error && (
                                <div style={{ padding: '10px 14px', borderRadius: 10, background: 'rgba(244,82,112,0.08)', border: '1px solid rgba(244,82,112,0.2)', color: 'var(--danger)', fontSize: 13 }}>
                                    {error}
                                </div>
                            )}
                            <div style={{ display: 'flex', gap: 10 }}>
                                <button onClick={() => setShowReminder(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handleSaveReminder} disabled={saving} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>
                                    {saving && <IcoLoader />} Guardar
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
