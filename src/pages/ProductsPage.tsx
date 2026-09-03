import { useEffect, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { formatCurrency, formatDateTime } from '../utils';
import { code128SVG } from '../utils/barcode';
import * as api from '../api';
import type { Product, Category, CreateProductDto, UpdateProductDto, Notification, Supplier, PriceHistoryEntry, SaveVariantDto } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';
import { invalidateProductImage, useProductImage } from '../hooks/useProductImages';

// ─── Fotos ───────────────────────────────────────────────────────────────────
// Las fotos se guardan como archivos, no dentro de la base, así que pueden
// conservarse a resolución de catálogo. Se envían dos tamaños: el bueno y una
// miniatura para los listados. El redimensionado ocurre aquí para que el
// backend no necesite una biblioteca de imágenes.
async function compressImage(file: File, maxDim = 1600, quality = 0.85): Promise<string> {
    const dataUrl: string = await new Promise((resolve, reject) => {
        const r = new FileReader();
        r.onload = () => resolve(r.result as string);
        r.onerror = reject;
        r.readAsDataURL(file);
    });
    const img: HTMLImageElement = await new Promise((resolve, reject) => {
        const im = new Image();
        im.onload = () => resolve(im);
        im.onerror = reject;
        im.src = dataUrl;
    });
    let width = img.width;
    let height = img.height;
    if (width >= height && width > maxDim) { height = Math.round(height * maxDim / width); width = maxDim; }
    else if (height > width && height > maxDim) { width = Math.round(width * maxDim / height); height = maxDim; }
    const canvas = document.createElement('canvas');
    canvas.width = width; canvas.height = height;
    const ctx = canvas.getContext('2d');
    if (!ctx) return dataUrl;
    ctx.drawImage(img, 0, 0, width, height);
    return canvas.toDataURL('image/jpeg', quality);
}

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
    const photo = useProductImage(product.id, product.has_image);
    if (photo && !err) {
        return (
            <img
                src={photo}
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
const IcoBarcode = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M3 5v14M7 5v14M11 5v14M15 5v10M19 5v14M21 5v14"/></svg>;

const escLabel = (s: string) => s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c] || c));
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
    const [searchParams] = useSearchParams();
    const [search, setSearch] = useState(searchParams.get('search') || '');
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
    const [photoThumb, setPhotoThumb] = useState<string | null>(null);     // miniatura para listados
    const [photoChanged, setPhotoChanged] = useState(false);               // whether user changed photo
    const fileInputRef = useRef<HTMLInputElement>(null);

    const [defaultMinStock, setDefaultMinStock] = useState(5);
    const [priceHistory, setPriceHistory] = useState<PriceHistoryEntry[]>([]);
    const [hasVariants, setHasVariants] = useState(false);
    const [variants, setVariants] = useState<SaveVariantDto[]>([]);
    const [labelProduct, setLabelProduct] = useState<Product | null>(null);
    const [labelQty, setLabelQty] = useState('12');

    const [showReminder, setShowReminder] = useState(false);
    const [reminderDate, setReminderDate] = useState('');
    const [reminderProduct, setReminderProduct] = useState<Product | null>(null);

    const { showToast } = useToast();
    const { confirm } = useConfirm();

    useEffect(() => { loadData(); }, []);
    useEffect(() => { const s = searchParams.get('search'); if (s) setSearch(s); }, [searchParams]);

    const loadData = async () => {
        try {
            const [p, c, s, n, threshold] = await Promise.all([
                api.getProducts(), api.getCategories(), api.getSuppliers(), api.getNotifications(),
                api.getConfig('low_stock_threshold').catch(() => ''),
            ]);
            setProducts(p); setCategories(c); setSuppliers(s); setNotifications(n);
            const parsed = parseInt(threshold, 10);
            if (Number.isFinite(parsed) && parsed >= 0) setDefaultMinStock(parsed);
        } catch (err) { showToast(String(err), 'error'); } finally { setLoading(false); }
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
        setForm({ sku: '', barcode: null, name: '', description: null, category_id: null, supplier_id: null, purchase_price: 0, sale_price: 0, stock: 0, min_stock: defaultMinStock });
        setPhotoPreview(null);
        setPhotoThumb(null);
        setPhotoChanged(false);
        setPriceHistory([]);
        setHasVariants(false); setVariants([]);
        setShowForm(true); setError('');
    };

    const photoRequestFor = useRef<number | null>(null);

    const openEditForm = (product: Product) => {
        setEditingProduct(product);
        setForm({ id: product.id, sku: product.sku, barcode: product.barcode, name: product.name, description: product.description, category_id: product.category_id, supplier_id: product.supplier_id, purchase_price: product.purchase_price, sale_price: product.sale_price, stock: product.stock, min_stock: product.min_stock, is_active: product.is_active });
        // El listado ya no trae la foto: se pide solo al abrir la ficha.
        setPhotoPreview(null);
        setPhotoThumb(null);
        if (product.has_image) {
            // Abrir dos fichas seguidas puede resolver las peticiones al revés;
            // el id descarta la respuesta que ya no corresponde.
            photoRequestFor.current = product.id;
            // La ficha muestra la foto buena, no la miniatura del listado.
            api.getProductImageList(product.id)
                .then(imgs => {
                    const principal = imgs.find(i => i.position === 0) ?? imgs[0];
                    if (!principal) return null;
                    return api.getProductPhoto(principal.id);
                })
                .then(url => {
                    if (photoRequestFor.current === product.id) setPhotoPreview(url ?? null);
                })
                .catch(() => {
                    if (photoRequestFor.current === product.id) setPhotoPreview(null);
                });
        }
        setPhotoChanged(false);
        setPriceHistory([]);
        api.getPriceHistory(product.id).then(setPriceHistory).catch(() => setPriceHistory([]));
        setHasVariants(product.has_variants);
        setVariants([]);
        if (product.has_variants) {
            api.getVariants(product.id)
                .then(vs => setVariants(vs.map(v => ({ id: v.id, size: v.size, color: v.color, sku: v.sku, barcode: v.barcode, stock: v.stock }))))
                .catch(() => setVariants([]));
        }
        setShowForm(true); setError('');
    };

    const addVariantRow = () => setVariants(v => [...v, { id: null, size: '', color: '', sku: '', barcode: '', stock: 0 }]);
    const updateVariantRow = (i: number, patch: Partial<SaveVariantDto>) => setVariants(v => v.map((row, idx) => idx === i ? { ...row, ...patch } : row));
    const removeVariantRow = (i: number) => setVariants(v => v.filter((_, idx) => idx !== i));

    const handlePhotoSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;
        if (!file.type.startsWith('image/')) {
            showToast('El archivo debe ser una imagen', 'error');
            e.target.value = ''; return;
        }
        if (file.size > 20 * 1024 * 1024) {
            showToast('La imagen supera el límite de 20 MB', 'error');
            e.target.value = ''; return;
        }
        try {
            const [grande, chica] = await Promise.all([
                compressImage(file),
                compressImage(file, 320, 0.7),
            ]);
            setPhotoPreview(grande);
            setPhotoThumb(chica);
            setPhotoChanged(true);
        } catch {
            showToast('No se pudo procesar la imagen', 'error');
        }
        // Reset input so same file can be re-selected
        e.target.value = '';
    };

    const handleRemovePhoto = () => {
        setPhotoPreview(null);
        setPhotoThumb(null);
        setPhotoChanged(true);
    };

    const validVariants = () => variants.filter(v => (v.size && v.size.trim()) || (v.color && v.color.trim()));

    const handleSave = async () => {
        if (!form.name || !form.sku || form.sale_price <= 0) {
            setError('Nombre, SKU y precio de venta son requeridos');
            return;
        }
        if (hasVariants && validVariants().length === 0) {
            setError('Agrega al menos una variante con talla o color');
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
            // La foto se guarda como archivo. Reemplazar significa quitar las
            // anteriores: el formulario maneja una sola imagen principal.
            if (photoChanged) {
                const previas = await api.getProductImageList(productId);
                for (const img of previas) await api.deleteProductImage(img.id);
                if (photoPreview && photoThumb) {
                    await api.addProductImage({
                        product_id: productId, photo: photoPreview, thumbnail: photoThumb,
                    });
                }
                invalidateProductImage(productId);
            }
            // Save variants (or clear them if variants were turned off)
            if (hasVariants) {
                await api.saveVariants(productId, validVariants());
            } else if (editingProduct?.has_variants) {
                await api.saveVariants(productId, []);
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

    const handlePrintLabels = () => {
        if (!labelProduct) return;
        const code = labelProduct.barcode || labelProduct.sku;
        const qty = Math.max(1, Math.min(120, parseInt(labelQty) || 1));
        const svg = code128SVG(code, { module: 2, height: 42 });
        const one = `<div class="lbl"><div class="nm">${escLabel(labelProduct.name)}</div><div class="pr">${escLabel(formatCurrency(labelProduct.sale_price))}</div><div class="bc">${svg}</div><div class="cd">${escLabel(code)}</div></div>`;
        const html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>Etiquetas</title><style>
            *{margin:0;padding:0;box-sizing:border-box}body{font-family:Arial,Helvetica,sans-serif}
            .sheet{display:flex;flex-wrap:wrap;gap:3mm;padding:5mm}
            .lbl{width:46mm;border:1px solid #e2e2e2;border-radius:2mm;padding:2mm;text-align:center;page-break-inside:avoid}
            .nm{font-size:10px;font-weight:700;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
            .pr{font-size:14px;font-weight:900;margin:1mm 0}
            .bc svg{max-width:100%;height:auto}
            .cd{font-size:9px;font-family:monospace;letter-spacing:1.5px;margin-top:1mm}
            @media print{.lbl{border:none}}
        </style></head><body><div class="sheet">${Array(qty).fill(one).join('')}</div></body></html>`;

        const iframe = document.createElement('iframe');
        iframe.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0';
        document.body.appendChild(iframe);
        const doc = iframe.contentWindow?.document;
        if (!doc) { document.body.removeChild(iframe); return; }
        doc.open(); doc.write(html); doc.close();
        setTimeout(() => {
            iframe.contentWindow?.focus();
            iframe.contentWindow?.print();
            setTimeout(() => { if (iframe.parentNode) document.body.removeChild(iframe); }, 1000);
        }, 250);
        setLabelProduct(null);
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
                                            <IcoBtn onClick={() => { setLabelProduct(p); setLabelQty('12'); }} title="Imprimir etiqueta" hoverColor="var(--success)" hoverBg="rgba(34,211,160,0.10)">
                                                <IcoBarcode />
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
                                    <input type="number" min="0" value={hasVariants ? 0 : form.stock} onChange={e => setForm({ ...form, stock: parseInt(e.target.value) || 0 })} className="input" disabled={!!editingProduct || hasVariants} />
                                    {hasVariants ? <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 4 }}>El stock se maneja por variante</p>
                                        : editingProduct && <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 4 }}>Usa ajuste de inventario para cambiar stock</p>}
                                </div>
                                <div>
                                    <label className="form-label">Stock Mínimo</label>
                                    <input type="number" min="0" value={form.min_stock} onChange={e => setForm({ ...form, min_stock: parseInt(e.target.value) || 0 })} className="input" />
                                </div>
                            </div>

                            {/* Variants (tallas / colores) */}
                            <div style={{ borderRadius: 12, background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.07)', padding: '12px 14px' }}>
                                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                                    <div>
                                        <p style={{ fontSize: 13, fontWeight: 600, color: 'var(--t1)' }}>Variantes (talla / color)</p>
                                        <p style={{ fontSize: 11, color: 'var(--t3)' }}>Controla el stock por talla y color</p>
                                    </div>
                                    <button type="button" onClick={() => { const next = !hasVariants; setHasVariants(next); if (next && variants.length === 0) addVariantRow(); }}
                                        style={{ width: 48, height: 26, borderRadius: 100, cursor: 'pointer', transition: 'background 0.2s', background: hasVariants ? 'var(--success)' : 'rgba(255,255,255,0.12)', position: 'relative', flexShrink: 0, border: 'none' }}>
                                        <span style={{ position: 'absolute', top: 3, left: hasVariants ? 24 : 3, width: 20, height: 20, borderRadius: '50%', background: '#fff', transition: 'left 0.2s', display: 'block' }} />
                                    </button>
                                </div>

                                {hasVariants && (
                                    <div style={{ marginTop: 12, display: 'flex', flexDirection: 'column', gap: 8 }}>
                                        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 70px 28px', gap: 8, fontSize: 10, fontWeight: 700, color: 'var(--t3)', textTransform: 'uppercase', letterSpacing: '0.04em', padding: '0 2px' }}>
                                            <span>Talla</span><span>Color</span><span style={{ textAlign: 'center' }}>Stock</span><span></span>
                                        </div>
                                        {variants.map((v, i) => (
                                            <div key={i} style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 70px 28px', gap: 8, alignItems: 'center' }}>
                                                <input value={v.size || ''} onChange={e => updateVariantRow(i, { size: e.target.value })} className="input" placeholder="M" style={{ padding: '7px 10px' }} />
                                                <input value={v.color || ''} onChange={e => updateVariantRow(i, { color: e.target.value })} className="input" placeholder="Negro" style={{ padding: '7px 10px' }} />
                                                <input type="number" min="0" value={v.stock} onChange={e => updateVariantRow(i, { stock: parseInt(e.target.value) || 0 })} className="input" style={{ padding: '7px 6px', textAlign: 'center' }} />
                                                <button type="button" onClick={() => removeVariantRow(i)} title="Quitar" style={{ color: 'var(--t3)', display: 'grid', placeItems: 'center' }}>
                                                    <IcoTrash />
                                                </button>
                                            </div>
                                        ))}
                                        <button type="button" onClick={addVariantRow} className="btn btn-ghost btn-sm" style={{ justifyContent: 'center', gap: 6, marginTop: 2 }}>
                                            <IcoPlus /> Agregar variante
                                        </button>
                                        <p style={{ fontSize: 11, color: 'var(--t3)' }}>Stock total: {variants.reduce((s, v) => s + (v.stock || 0), 0)} unidades</p>
                                    </div>
                                )}
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

                            {/* Price history */}
                            {editingProduct && priceHistory.length > 0 && (
                                <div style={{ borderRadius: 12, background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.07)', padding: '12px 14px' }}>
                                    <p style={{ fontSize: 12, fontWeight: 700, color: 'var(--t2)', marginBottom: 8 }}>Historial de precios</p>
                                    <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 120, overflowY: 'auto' }}>
                                        {priceHistory.map(h => (
                                            <div key={h.id} style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11, color: 'var(--t3)' }}>
                                                <span>{formatDateTime(h.created_at)}{h.user_name ? ` · ${h.user_name}` : ''}</span>
                                                <span style={{ fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(h.old_price)} → <strong style={{ color: 'var(--t1)' }}>{formatCurrency(h.new_price)}</strong></span>
                                            </div>
                                        ))}
                                    </div>
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

            {/* ── Label print modal ── */}
            {labelProduct && (
                <div className="modal-overlay" style={{ zIndex: 120 }} onClick={() => setLabelProduct(null)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 400, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 18 }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>Imprimir etiquetas</h3>
                            <button onClick={() => setLabelProduct(null)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}><IcoX /></button>
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div style={{ padding: '14px', borderRadius: 12, background: '#fff', textAlign: 'center' }}>
                                <p style={{ fontSize: 11, fontWeight: 700, color: '#111', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{labelProduct.name}</p>
                                <p style={{ fontSize: 15, fontWeight: 900, color: '#111', margin: '4px 0' }}>{formatCurrency(labelProduct.sale_price)}</p>
                                <div dangerouslySetInnerHTML={{ __html: code128SVG(labelProduct.barcode || labelProduct.sku, { module: 2, height: 42 }) }} style={{ display: 'flex', justifyContent: 'center' }} />
                                <p style={{ fontSize: 10, fontFamily: 'monospace', color: '#111', letterSpacing: 1 }}>{labelProduct.barcode || labelProduct.sku}</p>
                            </div>
                            <div>
                                <label className="form-label">Cantidad de etiquetas</label>
                                <input type="number" min={1} max={120} value={labelQty} onChange={e => setLabelQty(e.target.value)} className="input" />
                            </div>
                            <div style={{ display: 'flex', gap: 10 }}>
                                <button onClick={() => setLabelProduct(null)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handlePrintLabels} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>Imprimir</button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
