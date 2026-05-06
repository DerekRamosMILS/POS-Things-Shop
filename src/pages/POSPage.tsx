import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { useCartStore } from '../stores/useCartStore';
import { useSessionStore } from '../stores/useSessionStore';
import { formatCurrency } from '../utils';
import * as api from '../api';
import type { CartItem, Category, Product, Promotion } from '../types';

// ─── Design tokens ────────────────────────────────────────────────────────────
const T = {
    primary: '#8B78F5', primaryD: '#6B56E0', primaryG: 'rgba(139,120,245,0.3)',
    accent: '#F0C547', accentG: 'rgba(240,197,71,0.28)',
    success: '#22D3A0', danger: '#F45270', warning: '#F5A842',
    t1: '#EDEFFA', t2: '#B4BBCE', t3: '#7580A0',
};

// ─── Gradient Avatar fallback ─────────────────────────────────────────────────
const PALETTE: [string, string][] = [
    ['#8B78F5','#5D47C2'], ['#22D3A0','#15A078'], ['#F0C547','#C8A030'],
    ['#F45270','#B33356'], ['#38BDF8','#0B91D0'], ['#FB923C','#C45C18'],
    ['#A78BFA','#7C3AED'], ['#34D399','#059669'], ['#E879F9','#A21CAF'],
];
function getGrad(name: string): [string, string] {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = name.charCodeAt(i) + ((h << 5) - h);
    return PALETTE[Math.abs(h) % PALETTE.length];
}
function initials(name: string) {
    return (name || '?').split(' ').slice(0, 2).map(w => w[0]).join('').toUpperCase();
}

// ─── Product thumbnail — photo OR gradient fallback ───────────────────────────
function ProductThumb({
    product, width = 100, height = 72, radius = 12, fullWidth = false,
}: { product: Product; width?: number; height?: number; radius?: number; fullWidth?: boolean }) {
    const [imgError, setImgError] = useState(false);
    const [a, b] = getGrad(product.name);
    const w = fullWidth ? '100%' : width;
    const fontSize = Math.round(Math.min(width, height) * 0.32);

    if (product.image_url && !imgError) {
        return (
            <img
                src={product.image_url}
                alt={product.name}
                onError={() => setImgError(true)}
                style={{ width: w, height, borderRadius: radius, objectFit: 'cover', flexShrink: 0, display: 'block' }}
            />
        );
    }
    return (
        <div style={{
            width: w, height, borderRadius: radius, flexShrink: 0,
            background: `linear-gradient(135deg,${a},${b})`,
            display: 'grid', placeItems: 'center',
            color: '#fff', fontWeight: 800,
            fontSize,
            boxShadow: `0 4px 14px ${a}55`,
        }}>
            {initials(product.name)}
        </div>
    );
}

// ─── Cart item thumbnail (smaller) ────────────────────────────────────────────
function CartThumb({ product, size = 40 }: { product: Product; size?: number }) {
    const [imgError, setImgError] = useState(false);
    const [a, b] = getGrad(product.name);
    if (product.image_url && !imgError) {
        return (
            <img
                src={product.image_url} alt={product.name}
                onError={() => setImgError(true)}
                style={{ width: size, height: size, borderRadius: 10, objectFit: 'cover', flexShrink: 0 }}
            />
        );
    }
    return (
        <div style={{
            width: size, height: size, borderRadius: 10, flexShrink: 0,
            background: `linear-gradient(135deg,${a},${b})`,
            display: 'grid', placeItems: 'center',
            color: '#fff', fontWeight: 800, fontSize: Math.round(size * 0.38),
        }}>
            {initials(product.name)}
        </div>
    );
}

// ─── Types ────────────────────────────────────────────────────────────────────
type ServiceType = 'direct' | 'layaway';
const SERVICE_LABELS: Record<ServiceType, string> = { direct: 'Venta directa', layaway: 'Apartado' };

interface HeldCart { items: CartItem[]; customerName: string; orderNotes: string; serviceType: ServiceType; orderNo: number; }
interface Toast { id: number; msg: string; type: 'success' | 'warn' | 'error'; }
interface CompletedSale {
    folio: string; total: number; change: number; items: CartItem[];
    subtotal: number; lineDiscountTotal: number; promoDiscount: number;
    paymentMethod: string; amountPaid: number; serviceType: ServiceType;
    customerName: string; orderNotes: string;
}
let toastSeq = 0;

// ─── Print receipt ─────────────────────────────────────────────────────────────
function buildReceiptHTML(sale: CompletedSale): string {
    const rows = sale.items.map(item =>
        `<tr><td>${item.product.name}</td><td style="text-align:center">${item.quantity}</td>
         <td style="text-align:right">${formatCurrency(item.product.sale_price)}</td>
         <td style="text-align:right">${formatCurrency(item.product.sale_price * item.quantity - item.discount)}</td></tr>`
    ).join('');
    const now = new Date().toLocaleString('es-MX');
    return `<!DOCTYPE html><html><head><meta charset="UTF-8"><title>Ticket ${sale.folio}</title>
<style>*{margin:0;padding:0;box-sizing:border-box}body{font-family:monospace;font-size:12px;width:80mm;margin:0 auto;padding:10px}
h1{font-size:18px;text-align:center;margin-bottom:4px}.c{text-align:center}.hr{border:none;border-top:1px dashed #000;margin:8px 0}
table{width:100%;border-collapse:collapse;margin:6px 0}th{text-align:left;padding:2px 0;border-bottom:1px solid #000}
td{padding:3px 0;vertical-align:top}.grand td{font-weight:bold;font-size:14px;border-top:1px dashed #000;padding-top:6px}
</style></head><body><h1>ThingsShop</h1><p class="c">Folio: <b>${sale.folio}</b></p><p class="c">${now}</p>
<hr class="hr"><p>Tipo: ${SERVICE_LABELS[sale.serviceType]}</p>${sale.customerName ? `<p>Cliente: ${sale.customerName}</p>` : ''}
<hr class="hr"><table><thead><tr><th>Artículo</th><th style="text-align:center">Cant</th><th style="text-align:right">Precio</th><th style="text-align:right">Total</th></tr></thead>
<tbody>${rows}</tbody></table><hr class="hr"><table>
<tr><td>Subtotal</td><td style="text-align:right">${formatCurrency(sale.subtotal)}</td></tr>
${sale.lineDiscountTotal > 0 ? `<tr><td>Desc. línea</td><td style="text-align:right">-${formatCurrency(sale.lineDiscountTotal)}</td></tr>` : ''}
${sale.promoDiscount > 0 ? `<tr><td>Promoción</td><td style="text-align:right">-${formatCurrency(sale.promoDiscount)}</td></tr>` : ''}
<tr class="grand"><td>TOTAL</td><td style="text-align:right">${formatCurrency(sale.total)}</td></tr>
<tr><td>Pago</td><td style="text-align:right">${formatCurrency(sale.amountPaid)}</td></tr>
${sale.change > 0 ? `<tr><td>Cambio</td><td style="text-align:right">${formatCurrency(sale.change)}</td></tr>` : ''}
</table>${sale.orderNotes ? `<hr class="hr"><p>Notas: ${sale.orderNotes}</p>` : ''}
<hr class="hr"><p class="c">¡Gracias por su compra en ThingsShop!</p></body></html>`;
}

// ─── Component ────────────────────────────────────────────────────────────────
export default function POSPage() {
    const {
        items, addItem, removeItem, updateQuantity, clear, restoreItems,
        applyDiscount, getSubtotal, getDiscountTotal, getTotal, getItemCount,
    } = useCartStore();
    const { user, cashRegisterId } = useSessionStore();

    const [categories, setCategories] = useState<Category[]>([]);
    const [allProducts, setAllProducts] = useState<Product[]>([]);
    const [activeCategory, setActiveCategory] = useState<number | 'all'>('all');
    const [loadingProducts, setLoadingProducts] = useState(true);
    const [searchQuery, setSearchQuery] = useState('');
    const [searchResults, setSearchResults] = useState<Product[]>([]);

    const [serviceType, setServiceType] = useState<ServiceType>('direct');
    const [customerName, setCustomerName] = useState('');
    const [orderNotes, setOrderNotes] = useState('');
    const [orderSeq, setOrderSeq] = useState(() => Math.floor(Math.random() * 500) + 1000);

    const [showPayment, setShowPayment] = useState(false);
    const [paymentMethod, setPaymentMethod] = useState('cash');
    const [amountPaid, setAmountPaid] = useState('');
    const [processing, setProcessing] = useState(false);
    const [lastSale, setLastSale] = useState<CompletedSale | null>(null);

    const [toasts, setToasts] = useState<Toast[]>([]);

    const [holds, setHolds] = useState<(HeldCart | null)[]>([null, null, null]);

    const [promos, setPromos] = useState<Promotion[]>([]);
    const [promoInput, setPromoInput] = useState('');
    const [activePromo, setActivePromo] = useState<Promotion | null>(null);
    const [promoDropdown, setPromoDropdown] = useState(false);

    const searchRef = useRef<HTMLInputElement>(null);
    const barcodeBuffer = useRef('');
    const barcodeTimeout = useRef<number | null>(null);
    const liveRef = useRef({ showPayment, items, lastSale });
    useEffect(() => { liveRef.current = { showPayment, items, lastSale }; });

    const showToast = useCallback((msg: string, type: Toast['type'] = 'warn') => {
        const id = ++toastSeq;
        setToasts(prev => [...prev, { id, msg, type }]);
        setTimeout(() => setToasts(prev => prev.filter(t => t.id !== id)), 3200);
    }, []);

    useEffect(() => {
        (async () => {
            try {
                const [cats, prods, promoList] = await Promise.all([
                    api.getCategories(), api.getProducts({ is_active: true }), api.getPromotions(),
                ]);
                setCategories(cats.filter(c => c.is_active));
                setAllProducts(prods);
                setPromos(promoList.filter(p => p.is_active));
            } catch (err) { console.error(err); }
            finally { setLoadingProducts(false); }
        })();
    }, []);

    // Keyboard shortcuts + barcode scanner
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            const target = e.target as HTMLElement;
            const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA';
            const { showPayment: sp, items: its, lastSale: ls } = liveRef.current;
            if (e.key === 'F10') { e.preventDefault(); if (its.length > 0 && !sp && !ls) setShowPayment(true); return; }
            if (e.key === 'Escape' && sp) { e.preventDefault(); setShowPayment(false); return; }
            if (e.key === 'F2') { e.preventDefault(); searchRef.current?.focus(); return; }
            if (isInput && target !== searchRef.current) {
                if (e.key === 'Enter' && barcodeBuffer.current.length >= 4) {
                    e.preventDefault();
                    const code = barcodeBuffer.current; barcodeBuffer.current = '';
                    handleBarcodeScan(code);
                } else if (e.key.length === 1) {
                    barcodeBuffer.current += e.key;
                    if (barcodeTimeout.current) clearTimeout(barcodeTimeout.current);
                    barcodeTimeout.current = window.setTimeout(() => { barcodeBuffer.current = ''; }, 100);
                }
                return;
            }
            if (!isInput) {
                if (e.key === 'Enter' && barcodeBuffer.current.length >= 4) {
                    e.preventDefault();
                    const code = barcodeBuffer.current; barcodeBuffer.current = '';
                    handleBarcodeScan(code);
                } else if (e.key.length === 1) {
                    barcodeBuffer.current += e.key;
                    if (barcodeTimeout.current) clearTimeout(barcodeTimeout.current);
                    barcodeTimeout.current = window.setTimeout(() => { barcodeBuffer.current = ''; }, 100);
                }
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, []); // eslint-disable-line react-hooks/exhaustive-deps

    const handleBarcodeScan = async (code: string) => {
        try {
            const product = await api.getProductByBarcode(code);
            if (product) { handleAddItem(product); setSearchQuery(''); setSearchResults([]); }
            else showToast('Producto no encontrado', 'error');
        } catch (err) { console.error(err); }
    };

    const handleAddItem = useCallback((product: Product) => {
        const existing = liveRef.current.items.find(i => i.product.id === product.id);
        if (product.stock <= 0) { showToast(`Sin stock: ${product.name}`, 'error'); return; }
        if (existing && existing.quantity >= product.stock) { showToast(`"${product.name}" ya alcanzó el máximo (${product.stock})`, 'warn'); return; }
        addItem(product);
    }, [addItem, showToast]);

    const handleSearch = useCallback(async (query: string) => {
        setSearchQuery(query);
        if (query.length < 2) { setSearchResults([]); return; }
        try {
            const products = await api.getProducts({ search: query, is_active: true });
            setSearchResults(products.slice(0, 24));
        } catch (err) { console.error(err); }
    }, []);

    const visibleProducts = useMemo(() => {
        if (searchQuery.length >= 2) return searchResults;
        if (activeCategory === 'all') return allProducts;
        return allProducts.filter(p => p.category_id === activeCategory);
    }, [activeCategory, allProducts, searchQuery.length, searchResults]);

    const filteredPromos = useMemo(() => {
        if (!promoInput) return promos.slice(0, 6);
        const q = promoInput.toLowerCase();
        return promos.filter(p => p.name.toLowerCase().includes(q)).slice(0, 6);
    }, [promos, promoInput]);

    const promoDiscount = useMemo(() => {
        if (!activePromo) return 0;
        const base = getSubtotal() - getDiscountTotal();
        if (base <= 0) return 0;
        if (activePromo.discount_type === 'percentage') return Math.round(base * (activePromo.discount_value / 100) * 100) / 100;
        return Math.min(activePromo.discount_value, base);
    }, [activePromo, getSubtotal, getDiscountTotal]);

    const handleApplyPromo = (promo: Promotion) => {
        setActivePromo(promo); setPromoInput(promo.name); setPromoDropdown(false);
        const label = promo.discount_type === 'percentage' ? `${promo.discount_value}% off` : `${formatCurrency(promo.discount_value)} off`;
        showToast(`Promo "${promo.name}" — ${label}`, 'success');
    };
    const handleRemovePromo = () => { setActivePromo(null); setPromoInput(''); showToast('Promoción removida', 'warn'); };

    const subtotal = getSubtotal();
    const lineDiscountTotal = getDiscountTotal();
    const total = Math.max(0, getTotal() - promoDiscount);
    const changeAmount = paymentMethod === 'cash' ? (parseFloat(amountPaid) || 0) - total : 0;

    const handleHold = () => {
        if (items.length === 0) return;
        const emptyIdx = holds.findIndex(h => h === null);
        if (emptyIdx === -1) { showToast('Máximo 3 órdenes en espera', 'error'); return; }
        setHolds(prev => { const next = [...prev]; next[emptyIdx] = { items: items.map(i => ({ ...i })), customerName, orderNotes, serviceType, orderNo: orderSeq }; return next; });
        clear(); setCustomerName(''); setOrderNotes(''); setServiceType('direct'); setActivePromo(null); setPromoInput('');
        setOrderSeq(prev => prev + 1); showToast(`Orden #${orderSeq} en espera`, 'success');
    };

    const handleRestoreHold = (slotIdx: number) => {
        const slot = holds[slotIdx]; if (!slot) return;
        const newHolds = [...holds];
        if (items.length > 0) {
            const emptyIdx = holds.findIndex((h, i) => h === null && i !== slotIdx);
            if (emptyIdx === -1) { showToast('Libera un espacio antes de cambiar de orden', 'error'); return; }
            newHolds[emptyIdx] = { items: items.map(i => ({ ...i })), customerName, orderNotes, serviceType, orderNo: orderSeq };
        }
        newHolds[slotIdx] = null; setHolds(newHolds);
        restoreItems(slot.items); setCustomerName(slot.customerName); setOrderNotes(slot.orderNotes);
        setServiceType(slot.serviceType); setOrderSeq(slot.orderNo); setActivePromo(null); setPromoInput('');
        showToast(`Orden #${slot.orderNo} restaurada`, 'success');
    };

    const handleCompleteSale = async () => {
        if (items.length === 0 || !user) return;
        if (!cashRegisterId) { showToast('Abre la caja antes de cobrar', 'error'); setShowPayment(false); return; }
        const paid = paymentMethod === 'cash' ? parseFloat(amountPaid) || 0 : total;
        if (paymentMethod === 'cash' && paid < total) return;
        setProcessing(true);
        const saleItems = items.map(i => ({ ...i }));
        const saleSubtotal = subtotal, saleLineDiscount = lineDiscountTotal;
        try {
            const sale = await api.createSale(user.id, cashRegisterId, {
                items: items.map(item => ({ product_id: item.product.id, quantity: item.quantity, unit_price: item.product.sale_price, discount: item.discount })),
                payment_method: paymentMethod, amount_paid: paid,
                discount_total: saleLineDiscount + promoDiscount,
                notes: [SERVICE_LABELS[serviceType], customerName ? `Cliente: ${customerName}` : '', activePromo ? `Promo: ${activePromo.name}` : '', orderNotes.trim()].filter(Boolean).join(' | ') || null,
            });
            setLastSale({ folio: sale.folio, total: sale.total, change: sale.change_amount, items: saleItems, subtotal: saleSubtotal, lineDiscountTotal: saleLineDiscount, promoDiscount, paymentMethod, amountPaid: paid, serviceType, customerName, orderNotes });
            clear(); setShowPayment(false); setAmountPaid(''); setCustomerName(''); setOrderNotes(''); setActivePromo(null); setPromoInput('');
            setOrderSeq(prev => prev + 1);
        } catch (err) { showToast(String(err), 'error'); }
        finally { setProcessing(false); }
    };

    const handlePrint = () => {
        if (!lastSale) return;
        const html = buildReceiptHTML(lastSale);
        const win = window.open('', '_blank', 'width=420,height=640');
        if (win) { win.document.write(html); win.document.close(); win.focus(); win.print(); }
    };

    const isSearching = searchQuery.length >= 2;

    // ── Payment methods config ─────────────────────────────────────────────────
    const payMethods = [
        { k: 'cash', label: 'Efectivo', icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="1" y="4" width="22" height="16" rx="2"/><line x1="1" y1="10" x2="23" y2="10"/></svg> },
        { k: 'card', label: 'Tarjeta', icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg> },
        { k: 'transfer', label: 'Transfer.', icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 014-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 01-4 4H3"/></svg> },
    ];

    return (
        <>
            {/* ── Toast notifications ── */}
            <div style={{ position: 'fixed', bottom: 24, right: 24, zIndex: 200, display: 'flex', flexDirection: 'column', gap: 8 }}>
                {toasts.map(t => (
                    <div key={t.id} className="scale-in" style={{
                        padding: '10px 16px', borderRadius: 12, fontSize: 13, fontWeight: 600,
                        background: t.type === 'success' ? 'rgba(34,211,160,0.15)' : t.type === 'error' ? 'rgba(244,82,112,0.15)' : 'rgba(255,255,255,0.10)',
                        border: `1px solid ${t.type === 'success' ? 'rgba(34,211,160,0.3)' : t.type === 'error' ? 'rgba(244,82,112,0.3)' : 'rgba(255,255,255,0.15)'}`,
                        color: t.type === 'success' ? T.success : t.type === 'error' ? T.danger : T.t1,
                        backdropFilter: 'blur(16px)', maxWidth: 320,
                    }}>{t.msg}</div>
                ))}
            </div>

            <div className="pos-layout">
                {/* ── Left: catalog ── */}
                <div className="pos-catalog">
                    {/* Search */}
                    <div className="pos-search-bar">
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke={T.t3} strokeWidth="2.2" strokeLinecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                        <input
                            ref={searchRef} value={searchQuery}
                            onChange={e => handleSearch(e.target.value)}
                            placeholder="Buscar producto... (F2)"
                            style={{ flex: 1, background: 'none', border: 'none', color: T.t1, fontSize: 14, outline: 'none', fontFamily: 'inherit' }}
                            autoFocus
                        />
                        {searchQuery && (
                            <button onClick={() => { setSearchQuery(''); setSearchResults([]); }} style={{ color: T.t3, display: 'grid', placeItems: 'center' }}>
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                            </button>
                        )}
                    </div>

                    {/* Category tabs */}
                    <div className="pos-cat-tabs" style={{ marginBottom: 16 }}>
                        <button className={`pos-cat-btn${activeCategory === 'all' && !isSearching ? ' active' : ''}`} onClick={() => setActiveCategory('all')}>
                            Todos <span style={{ opacity: 0.6, fontSize: 10 }}>({allProducts.length})</span>
                        </button>
                        {categories.map(cat => (
                            <button key={cat.id} className={`pos-cat-btn${activeCategory === cat.id && !isSearching ? ' active' : ''}`} onClick={() => setActiveCategory(cat.id)}>
                                {cat.name}
                            </button>
                        ))}
                    </div>

                    {/* Product grid */}
                    {loadingProducts ? (
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', flex: 1, gap: 10, color: T.t3 }}>
                            <div style={{ width: 24, height: 24, border: '2px solid rgba(139,120,245,0.2)', borderTopColor: T.primary, borderRadius: '50%', animation: 'spin 0.7s linear infinite' }} />
                            Cargando catálogo...
                        </div>
                    ) : visibleProducts.length === 0 ? (
                        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', flex: 1, gap: 12, opacity: 0.5 }}>
                            <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke={T.t3} strokeWidth="1.5" strokeLinecap="round"><path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/></svg>
                            <p style={{ fontSize: 13, color: T.t3, fontWeight: 600 }}>Sin productos</p>
                        </div>
                    ) : (
                        <div className="pos-product-grid">
                            {visibleProducts.map(product => {
                                const cartItem = items.find(i => i.product.id === product.id);
                                const inCart = !!cartItem;
                                const atMax = cartItem ? cartItem.quantity >= product.stock : false;
                                return (
                                    <button
                                        key={product.id}
                                        onClick={() => handleAddItem(product)}
                                        disabled={product.stock <= 0}
                                        className={`pos-product-card${inCart ? ' in-cart' : ''}`}
                                    >
                                        {/* Product image */}
                                        <ProductThumb product={product} fullWidth height={90} radius={12} />

                                        <div style={{ width: '100%' }}>
                                            <p style={{ fontSize: 13, fontWeight: 700, color: T.t1, lineHeight: 1.3, marginBottom: 4 }}>{product.name}</p>
                                            <p style={{ fontSize: 11, color: T.t3, fontWeight: 600, marginBottom: 8 }}>{product.category_name || 'Sin categoría'}</p>
                                            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                                                <span style={{ fontSize: 15, fontWeight: 900, color: inCart ? T.primary : T.t1, fontVariantNumeric: 'tabular-nums' }}>
                                                    {formatCurrency(product.sale_price)}
                                                </span>
                                                <span style={{ fontSize: 10, fontWeight: 700, padding: '2px 8px', borderRadius: 100, background: product.stock <= 3 ? 'rgba(244,82,112,0.15)' : 'rgba(255,255,255,0.07)', color: product.stock <= 3 ? T.danger : T.t3 }}>
                                                    {product.stock <= 0 ? 'Agotado' : atMax ? '✓ Máx' : `${product.stock} disp.`}
                                                </span>
                                            </div>
                                        </div>
                                    </button>
                                );
                            })}
                        </div>
                    )}
                </div>

                {/* ── Right: cart panel ── */}
                <div className="pos-cart">
                    {/* Cart header */}
                    <div className="cart-header">
                        <div>
                            <h2 style={{ fontSize: 16, fontWeight: 800, color: T.t1 }}>Ticket</h2>
                            <p style={{ fontSize: 11, color: T.t3, fontWeight: 600, marginTop: 2 }}>
                                Orden #{orderSeq} · {getItemCount()} artículo{getItemCount() !== 1 ? 's' : ''}
                            </p>
                        </div>
                        {items.length > 0 && (
                            <button onClick={clear} style={{ fontSize: 11, fontWeight: 700, color: T.danger, padding: '5px 12px', borderRadius: 8, background: 'rgba(244,82,112,0.08)', border: '1px solid rgba(244,82,112,0.15)', cursor: 'pointer', fontFamily: 'inherit', transition: 'background 0.15s' }}
                                onMouseEnter={e => (e.currentTarget.style.background = 'rgba(244,82,112,0.18)')}
                                onMouseLeave={e => (e.currentTarget.style.background = 'rgba(244,82,112,0.08)')}>
                                Limpiar
                            </button>
                        )}
                    </div>

                    {/* Customer name */}
                    <div style={{ marginBottom: 14 }}>
                        <input
                            value={customerName} onChange={e => setCustomerName(e.target.value)}
                            placeholder="Nombre del cliente (opcional)"
                            style={{ width: '100%', padding: '9px 12px', borderRadius: 11, background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.09)', color: T.t2, fontSize: 12, fontFamily: 'inherit', outline: 'none' }}
                            onFocus={e => (e.target.style.borderColor = 'rgba(139,120,245,0.4)')}
                            onBlur={e => (e.target.style.borderColor = 'rgba(255,255,255,0.09)')}
                        />
                    </div>

                    {/* Service type tabs */}
                    <div style={{ display: 'flex', gap: 6, marginBottom: 14 }}>
                        {(Object.keys(SERVICE_LABELS) as ServiceType[]).map(t => (
                            <button key={t} onClick={() => setServiceType(t)}
                                style={{
                                    flex: 1, padding: '7px 8px', borderRadius: 9, fontSize: 11, fontWeight: 700, cursor: 'pointer', fontFamily: 'inherit',
                                    background: serviceType === t ? 'rgba(139,120,245,0.18)' : 'rgba(255,255,255,0.04)',
                                    color: serviceType === t ? T.primary : T.t3,
                                    border: `1px solid ${serviceType === t ? 'rgba(139,120,245,0.35)' : 'rgba(255,255,255,0.08)'}`,
                                    transition: 'all 0.15s',
                                }}>
                                {SERVICE_LABELS[t]}
                            </button>
                        ))}
                    </div>

                    {/* Hold slots */}
                    {holds.some(Boolean) && (
                        <div style={{ display: 'flex', gap: 6, marginBottom: 14 }}>
                            {holds.map((slot, i) => slot ? (
                                <button key={i} onClick={() => handleRestoreHold(i)}
                                    style={{ flex: 1, padding: '7px 6px', borderRadius: 9, fontSize: 10, fontWeight: 700, cursor: 'pointer', fontFamily: 'inherit', background: 'rgba(240,197,71,0.10)', color: T.accent, border: '1px solid rgba(240,197,71,0.25)', transition: 'all 0.15s', lineHeight: 1.3 }}>
                                    #{slot.orderNo}<br /><span style={{ fontWeight: 600 }}>{slot.items.reduce((s, it) => s + it.quantity, 0)} ítem(s)</span>
                                </button>
                            ) : null)}
                        </div>
                    )}

                    {/* Cart items */}
                    <div className="cart-items">
                        {items.length === 0 ? (
                            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', gap: 12, opacity: 0.5 }}>
                                <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke={T.t3} strokeWidth="1.5" strokeLinecap="round"><circle cx="9" cy="21" r="1"/><circle cx="20" cy="21" r="1"/><path d="M1 1h4l2.68 13.39a2 2 0 002 1.61h9.72a2 2 0 001.99-1.73L23 6H6"/></svg>
                                <div style={{ textAlign: 'center' }}>
                                    <p style={{ fontSize: 13, fontWeight: 700, color: T.t2 }}>Sin artículos</p>
                                    <p style={{ fontSize: 12, color: T.t3, marginTop: 4 }}>Selecciona una prenda del catálogo</p>
                                </div>
                            </div>
                        ) : items.map(item => {
                            const lineTotal = item.product.sale_price * item.quantity - item.discount;
                            return (
                                <div key={item.product.id} className="cart-item">
                                    <div className="cart-item-top">
                                        <CartThumb product={item.product} size={40} />
                                        <div style={{ flex: 1, minWidth: 0 }}>
                                            <p style={{ fontSize: 13, fontWeight: 700, color: T.t1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{item.product.name}</p>
                                            <p style={{ fontSize: 11, color: T.t3, fontWeight: 600 }}>{formatCurrency(item.product.sale_price)} c/u</p>
                                        </div>
                                        <button className="cart-item-del" onClick={() => removeItem(item.product.id)}>
                                            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a1 1 0 011-1h4a1 1 0 011 1v2"/></svg>
                                        </button>
                                    </div>
                                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 10 }}>
                                        {/* Qty */}
                                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                                            <button className="cart-qty-btn" onClick={() => updateQuantity(item.product.id, item.quantity - 1)}>−</button>
                                            <span style={{ fontSize: 14, fontWeight: 800, color: T.t1, width: 24, textAlign: 'center' }}>{item.quantity}</span>
                                            <button className="cart-qty-btn" onClick={() => updateQuantity(item.product.id, item.quantity + 1)} disabled={item.quantity >= item.product.stock}>+</button>
                                        </div>
                                        {/* Discount */}
                                        <div style={{ display: 'flex', alignItems: 'center', gap: 5, flex: 1 }}>
                                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke={T.t3} strokeWidth="2.2" strokeLinecap="round"><path d="M20.59 13.41l-7.17 7.17a2 2 0 01-2.83 0L2 12V2h10l8.59 8.59a2 2 0 010 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/></svg>
                                            <input
                                                type="number" min="0" step="0.01"
                                                value={item.discount || ''}
                                                onChange={e => applyDiscount(item.product.id, Math.max(0, Math.min(parseFloat(e.target.value) || 0, item.product.sale_price * item.quantity)))}
                                                placeholder="Desc."
                                                style={{ width: '100%', padding: '5px 8px', borderRadius: 8, background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.10)', color: T.t2, fontSize: 12, fontFamily: 'inherit', outline: 'none' }}
                                            />
                                        </div>
                                        <span style={{ fontSize: 14, fontWeight: 900, color: T.t1, fontVariantNumeric: 'tabular-nums', flexShrink: 0 }}>{formatCurrency(lineTotal)}</span>
                                    </div>
                                </div>
                            );
                        })}
                    </div>

                    {/* Promo */}
                    <div style={{ position: 'relative', marginBottom: 10 }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 12px', borderRadius: 11, background: 'rgba(255,255,255,0.04)', border: `1px solid ${activePromo ? 'rgba(139,120,245,0.35)' : 'rgba(255,255,255,0.09)'}` }}>
                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke={activePromo ? T.primary : T.t3} strokeWidth="2.2" strokeLinecap="round"><path d="M20.59 13.41l-7.17 7.17a2 2 0 01-2.83 0L2 12V2h10l8.59 8.59a2 2 0 010 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/></svg>
                            <input
                                value={promoInput}
                                onChange={e => { setPromoInput(e.target.value); setPromoDropdown(true); if (activePromo) setActivePromo(null); }}
                                onFocus={() => setPromoDropdown(true)}
                                onBlur={() => setTimeout(() => setPromoDropdown(false), 160)}
                                placeholder="Código de promoción..."
                                style={{ flex: 1, background: 'none', border: 'none', color: activePromo ? T.primary : T.t2, fontSize: 12, fontFamily: 'inherit', outline: 'none' }}
                            />
                            {activePromo && <button onClick={handleRemovePromo} style={{ color: T.t3, display: 'grid', placeItems: 'center' }}>
                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                            </button>}
                        </div>
                        {promoDropdown && filteredPromos.length > 0 && (
                            <div style={{ position: 'absolute', bottom: '100%', left: 0, right: 0, marginBottom: 4, borderRadius: 12, background: 'rgba(18,12,38,0.96)', border: '1px solid rgba(255,255,255,0.12)', overflow: 'hidden', zIndex: 20 }}>
                                {filteredPromos.map(p => (
                                    <button key={p.id} onMouseDown={() => handleApplyPromo(p)}
                                        style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', width: '100%', padding: '9px 14px', fontSize: 12, color: T.t2, fontFamily: 'inherit', cursor: 'pointer', background: 'none', border: 'none', transition: 'background 0.12s' }}
                                        onMouseEnter={e => (e.currentTarget.style.background = 'rgba(255,255,255,0.07)')}
                                        onMouseLeave={e => (e.currentTarget.style.background = 'none')}>
                                        <span style={{ fontWeight: 700 }}>{p.name}</span>
                                        <em style={{ fontSize: 11, color: T.accent, fontStyle: 'normal', fontWeight: 700 }}>{p.discount_type === 'percentage' ? `${p.discount_value}%` : formatCurrency(p.discount_value)}</em>
                                    </button>
                                ))}
                            </div>
                        )}
                    </div>

                    {/* Notes */}
                    <textarea
                        value={orderNotes} onChange={e => setOrderNotes(e.target.value)}
                        placeholder="Notas del pedido..."
                        rows={2}
                        style={{ width: '100%', padding: '8px 12px', borderRadius: 11, background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.09)', color: T.t2, fontSize: 12, fontFamily: 'inherit', outline: 'none', resize: 'none', marginBottom: 14 }}
                        onFocus={e => (e.target.style.borderColor = 'rgba(139,120,245,0.4)')}
                        onBlur={e => (e.target.style.borderColor = 'rgba(255,255,255,0.09)')}
                    />

                    {/* Totals */}
                    {items.length > 0 && (
                        <div className="cart-totals">
                            <div style={{ display: 'flex', flexDirection: 'column', gap: 8, marginBottom: 14 }}>
                                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                                    <span style={{ fontSize: 13, color: T.t3 }}>Subtotal</span>
                                    <span style={{ fontSize: 13, color: T.t2, fontWeight: 600, fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(subtotal)}</span>
                                </div>
                                {lineDiscountTotal > 0 && (
                                    <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                                        <span style={{ fontSize: 13, color: T.warning }}>Desc. línea</span>
                                        <span style={{ fontSize: 13, color: T.warning, fontWeight: 600 }}>−{formatCurrency(lineDiscountTotal)}</span>
                                    </div>
                                )}
                                {promoDiscount > 0 && (
                                    <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                                        <span style={{ fontSize: 13, color: T.primary }}>Promo</span>
                                        <span style={{ fontSize: 13, color: T.primary, fontWeight: 600 }}>−{formatCurrency(promoDiscount)}</span>
                                    </div>
                                )}
                                <div style={{ display: 'flex', justifyContent: 'space-between', paddingTop: 10, borderTop: '1px solid rgba(255,255,255,0.07)', marginTop: 4 }}>
                                    <span style={{ fontSize: 16, fontWeight: 800, color: T.t1 }}>Total</span>
                                    <span style={{ fontSize: 20, fontWeight: 900, color: T.primary, fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(total)}</span>
                                </div>
                            </div>
                            <button
                                onClick={() => setShowPayment(true)}
                                style={{
                                    width: '100%', padding: '13px', borderRadius: 14, border: 'none',
                                    background: 'linear-gradient(135deg, #F0C547, #C8A030)',
                                    color: '#1a1200', fontSize: 14, fontWeight: 800, cursor: 'pointer',
                                    fontFamily: 'inherit', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
                                    boxShadow: '0 6px 20px rgba(240,197,71,0.3)', transition: 'all 0.15s',
                                }}
                                onMouseEnter={e => { e.currentTarget.style.boxShadow = '0 10px 28px rgba(240,197,71,0.45)'; e.currentTarget.style.transform = 'translateY(-1px)'; }}
                                onMouseLeave={e => { e.currentTarget.style.boxShadow = '0 6px 20px rgba(240,197,71,0.3)'; e.currentTarget.style.transform = 'none'; }}
                            >
                                Cobrar
                                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>
                            </button>
                            <button onClick={handleHold} style={{ width: '100%', marginTop: 8, padding: '9px', borderRadius: 11, fontSize: 12, fontWeight: 700, color: T.t3, background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.09)', cursor: 'pointer', fontFamily: 'inherit', transition: 'all 0.15s' }}
                                onMouseEnter={e => { e.currentTarget.style.color = T.t1; e.currentTarget.style.background = 'rgba(255,255,255,0.08)'; }}
                                onMouseLeave={e => { e.currentTarget.style.color = T.t3; e.currentTarget.style.background = 'rgba(255,255,255,0.04)'; }}>
                                Poner en espera
                            </button>
                        </div>
                    )}
                </div>
            </div>

            {/* ── Payment modal ── */}
            {showPayment && (
                <div className="modal-overlay">
                    <div className="glass-modal scale-in" style={{ width: '100%', maxWidth: 440, padding: 32 }}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 24 }}>
                            <div>
                                <h2 style={{ fontSize: 20, fontWeight: 900, color: T.t1, letterSpacing: '-0.02em' }}>Cobrar</h2>
                                <p style={{ fontSize: 13, color: T.t3, marginTop: 3 }}>Orden #{orderSeq}</p>
                            </div>
                            <button onClick={() => setShowPayment(false)} style={{ width: 34, height: 34, borderRadius: 10, background: 'rgba(255,255,255,0.07)', border: '1px solid rgba(255,255,255,0.12)', display: 'grid', placeItems: 'center', color: T.t2, cursor: 'pointer' }}>
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                            </button>
                        </div>

                        {!cashRegisterId && (
                            <div style={{ padding: '10px 14px', marginBottom: 16, borderRadius: 11, background: 'rgba(244,82,112,0.10)', border: '1px solid rgba(244,82,112,0.25)', color: T.danger, fontSize: 13 }}>
                                ⚠ La caja no está abierta. <Link to="/cash-register" onClick={() => setShowPayment(false)} style={{ fontWeight: 700, textDecoration: 'underline' }}>Abrir caja →</Link>
                            </div>
                        )}

                        <div style={{ padding: '20px 0', textAlign: 'center', marginBottom: 24, borderTop: '1px solid rgba(255,255,255,0.07)', borderBottom: '1px solid rgba(255,255,255,0.07)' }}>
                            <p style={{ fontSize: 12, fontWeight: 600, color: T.t3, textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 8 }}>Total a cobrar</p>
                            <p style={{ fontSize: 42, fontWeight: 900, color: T.t1, fontVariantNumeric: 'tabular-nums', letterSpacing: '-0.03em' }}>{formatCurrency(total)}</p>
                        </div>

                        {/* Payment methods */}
                        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 10, marginBottom: 22 }}>
                            {payMethods.map(m => (
                                <button key={m.k} onClick={() => setPaymentMethod(m.k)} className={`pay-method-btn${paymentMethod === m.k ? ' active' : ''}`}>
                                    {m.icon}{m.label}
                                </button>
                            ))}
                        </div>

                        {/* Cash input */}
                        {paymentMethod === 'cash' && (
                            <div style={{ marginBottom: 20 }}>
                                <label className="form-label">Monto recibido</label>
                                <input
                                    type="number" step="1" value={amountPaid} onChange={e => setAmountPaid(e.target.value)}
                                    placeholder="0.00" autoFocus
                                    style={{ width: '100%', padding: '13px 16px', borderRadius: 13, fontSize: 18, fontWeight: 800, background: 'rgba(255,255,255,0.06)', border: '1px solid rgba(255,255,255,0.15)', color: T.t1, fontFamily: 'inherit', outline: 'none', fontVariantNumeric: 'tabular-nums' }}
                                    onFocus={e => (e.target.style.borderColor = T.primary)}
                                    onBlur={e => (e.target.style.borderColor = 'rgba(255,255,255,0.15)')}
                                />
                                <div style={{ display: 'flex', gap: 8, marginTop: 10, flexWrap: 'wrap' }}>
                                    {[50, 100, 200, 500, 1000].map(a => (
                                        <button key={a} onClick={() => setAmountPaid(String((parseFloat(amountPaid) || 0) + a))}
                                            style={{ padding: '6px 14px', borderRadius: 9, background: 'rgba(255,255,255,0.06)', border: '1px solid rgba(255,255,255,0.10)', color: T.t2, fontSize: 12, fontWeight: 700, cursor: 'pointer', fontFamily: 'inherit' }}>
                                            +${a}
                                        </button>
                                    ))}
                                </div>
                                {changeAmount > 0 && (
                                    <div style={{ marginTop: 14, padding: '12px 16px', borderRadius: 13, background: 'rgba(34,211,160,0.10)', border: '1px solid rgba(34,211,160,0.20)', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                                        <span style={{ fontSize: 13, fontWeight: 600, color: T.success }}>Cambio</span>
                                        <span style={{ fontSize: 18, fontWeight: 900, color: T.success, fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(changeAmount)}</span>
                                    </div>
                                )}
                            </div>
                        )}

                        <button
                            onClick={handleCompleteSale}
                            disabled={processing || !cashRegisterId || (paymentMethod === 'cash' && (parseFloat(amountPaid) || 0) < total)}
                            style={{
                                width: '100%', padding: '14px', borderRadius: 14, border: 'none',
                                background: 'linear-gradient(135deg, #22D3A0, #18A880)',
                                color: '#001a13', fontSize: 15, fontWeight: 800, cursor: processing ? 'wait' : 'pointer',
                                fontFamily: 'inherit', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
                                boxShadow: '0 8px 28px rgba(34,211,160,0.35)', opacity: (processing || !cashRegisterId || (paymentMethod === 'cash' && (parseFloat(amountPaid) || 0) < total)) ? 0.45 : 1,
                                transition: 'all 0.15s',
                            }}
                        >
                            {processing
                                ? <><div style={{ width: 16, height: 16, border: '2px solid rgba(0,26,19,0.3)', borderTopColor: '#001a13', borderRadius: '50%', animation: 'spin 0.7s linear infinite' }} />Procesando...</>
                                : <><svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polyline points="20 6 9 17 4 12"/></svg>Confirmar venta</>
                            }
                        </button>
                    </div>
                </div>
            )}

            {/* ── Success modal ── */}
            {lastSale && (
                <div className="modal-overlay">
                    <div className="glass-modal scale-in" style={{ width: '100%', maxWidth: 380, padding: 40, textAlign: 'center', borderColor: 'rgba(34,211,160,0.25)' }}>
                        <div style={{ width: 72, height: 72, borderRadius: '50%', background: 'rgba(34,211,160,0.15)', border: '2px solid rgba(34,211,160,0.4)', display: 'grid', placeItems: 'center', margin: '0 auto 20px' }}>
                            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke={T.success} strokeWidth="2.5" strokeLinecap="round"><polyline points="20 6 9 17 4 12"/></svg>
                        </div>
                        <h2 style={{ fontSize: 22, fontWeight: 900, color: T.t1, marginBottom: 8 }}>¡Venta exitosa!</h2>
                        <p style={{ fontSize: 14, color: T.t3, marginBottom: 20 }}>Folio {lastSale.folio}</p>
                        <p style={{ fontSize: 36, fontWeight: 900, color: T.success, fontVariantNumeric: 'tabular-nums', marginBottom: 6 }}>{formatCurrency(lastSale.total)}</p>
                        {lastSale.change > 0 && <p style={{ fontSize: 14, color: T.t3, marginBottom: 24 }}>Cambio: <strong style={{ color: T.t1 }}>{formatCurrency(lastSale.change)}</strong></p>}
                        <div style={{ display: 'flex', gap: 10, justifyContent: 'center', marginTop: 28 }}>
                            <button onClick={handlePrint} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '10px 20px', borderRadius: 13, background: 'rgba(255,255,255,0.05)', border: '1px solid rgba(255,255,255,0.12)', color: T.t2, fontSize: 13, fontWeight: 700, cursor: 'pointer', fontFamily: 'inherit', transition: 'all 0.15s' }}>
                                <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round"><polyline points="6 9 6 2 18 2 18 9"/><path d="M6 18H4a2 2 0 01-2-2v-5a2 2 0 012-2h16a2 2 0 012 2v5a2 2 0 01-2 2h-2"/><rect x="6" y="14" width="12" height="8"/></svg>
                                Imprimir
                            </button>
                            <button onClick={() => { setLastSale(null); searchRef.current?.focus(); }}
                                style={{ padding: '10px 20px', borderRadius: 13, background: 'linear-gradient(135deg, #8B78F5, #6B56E0)', border: 'none', color: '#fff', fontSize: 13, fontWeight: 700, cursor: 'pointer', fontFamily: 'inherit', boxShadow: '0 4px 16px rgba(139,120,245,0.3)', transition: 'all 0.15s' }}>
                                Nueva venta
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </>
    );
}
