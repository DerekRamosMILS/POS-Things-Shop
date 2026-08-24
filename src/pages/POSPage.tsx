import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { useCartStore, cartLineId, lineIdOf, stockOf } from '../stores/useCartStore';
import { useSessionStore } from '../stores/useSessionStore';
import { useHoldsStore, type ServiceType } from '../stores/useHoldsStore';
import { formatCurrency } from '../utils';
import { configFromSettings, createScannerHandler } from '../utils/scanner';
import { useProductImage } from '../hooks/useProductImages';
import { evaluateMixedTender, round2 } from '../utils/cash';
import KeyboardHelp from '../components/KeyboardHelp';
import * as api from '../api';


import type { CartItem, CartVariant, Category, Customer, PaymentSplit, Product, ProductVariant, Promotion } from '../types';

const variantLabel = (v: CartVariant | null): string =>
    v ? ([v.size, v.color].filter(x => x && x.trim()).join(' / ') || 'Único') : '';

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
    // La foto no viene en el listado; se pide solo para lo que está en pantalla.
    const photo = useProductImage(product.id, product.has_image);

    if (photo && !imgError) {
        return (
            <img
                src={photo}
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
    const photo = useProductImage(product.id, product.has_image);
    if (photo && !imgError) {
        return (
            <img
                src={photo} alt={product.name}
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
const SERVICE_LABELS: Record<ServiceType, string> = { direct: 'Venta directa', layaway: 'Apartado' };

interface StoreInfo { name: string; address: string; phone: string; footer: string; }
interface Toast { id: number; msg: string; type: 'success' | 'warn' | 'error'; }
interface CompletedSale {
    folio: string; total: number; change: number; items: CartItem[];
    subtotal: number; lineDiscountTotal: number; promoDiscount: number; tax: number;
    paymentMethod: string; amountPaid: number; serviceType: ServiceType;
    customerName: string; orderNotes: string;
}
let toastSeq = 0;

// A promotion applies only within its date window (compared in local time).
function isPromoValidToday(p: Promotion): boolean {
    const today = new Date().toLocaleDateString('en-CA'); // YYYY-MM-DD, local
    const start = (p.start_date || '').slice(0, 10);
    const end = (p.end_date || '').slice(0, 10);
    if (start && today < start) return false;
    if (end && today > end) return false;
    return true;
}

// ─── Print receipt ─────────────────────────────────────────────────────────────
const esc = (s: string) => s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c] || c));

function buildReceiptHTML(sale: CompletedSale, store: StoreInfo): string {
    const rows = sale.items.map(item =>
        `<tr><td>${esc(item.product.name)}${item.variant ? ` <small>(${esc(variantLabel(item.variant))})</small>` : ''}</td><td style="text-align:center">${item.quantity}</td>
         <td style="text-align:right">${formatCurrency(item.product.sale_price)}</td>
         <td style="text-align:right">${formatCurrency(item.product.sale_price * item.quantity - item.discount)}</td></tr>`
    ).join('');
    const now = new Date().toLocaleString('es-MX');
    const storeName = store.name || 'ThingsShop';
    return `<!DOCTYPE html><html><head><meta charset="UTF-8"><title>Ticket ${esc(sale.folio)}</title>
<style>*{margin:0;padding:0;box-sizing:border-box}body{font-family:monospace;font-size:12px;width:80mm;margin:0 auto;padding:10px}
h1{font-size:18px;text-align:center;margin-bottom:4px}.c{text-align:center}.sm{font-size:11px}.hr{border:none;border-top:1px dashed #000;margin:8px 0}
table{width:100%;border-collapse:collapse;margin:6px 0}th{text-align:left;padding:2px 0;border-bottom:1px solid #000}
td{padding:3px 0;vertical-align:top}.grand td{font-weight:bold;font-size:14px;border-top:1px dashed #000;padding-top:6px}
</style></head><body><h1>${esc(storeName)}</h1>
${store.address ? `<p class="c sm">${esc(store.address)}</p>` : ''}
${store.phone ? `<p class="c sm">Tel: ${esc(store.phone)}</p>` : ''}
<p class="c">Folio: <b>${esc(sale.folio)}</b></p><p class="c sm">${now}</p>
<hr class="hr"><p>Tipo: ${SERVICE_LABELS[sale.serviceType]}</p>${sale.customerName ? `<p>Cliente: ${esc(sale.customerName)}</p>` : ''}
<hr class="hr"><table><thead><tr><th>Artículo</th><th style="text-align:center">Cant</th><th style="text-align:right">Precio</th><th style="text-align:right">Total</th></tr></thead>
<tbody>${rows}</tbody></table><hr class="hr"><table>
<tr><td>Subtotal</td><td style="text-align:right">${formatCurrency(sale.subtotal)}</td></tr>
${sale.lineDiscountTotal > 0 ? `<tr><td>Desc. línea</td><td style="text-align:right">-${formatCurrency(sale.lineDiscountTotal)}</td></tr>` : ''}
${sale.promoDiscount > 0 ? `<tr><td>Promoción</td><td style="text-align:right">-${formatCurrency(sale.promoDiscount)}</td></tr>` : ''}
${sale.tax > 0 ? `<tr><td>Impuesto</td><td style="text-align:right">${formatCurrency(sale.tax)}</td></tr>` : ''}
<tr class="grand"><td>TOTAL</td><td style="text-align:right">${formatCurrency(sale.total)}</td></tr>
<tr><td>Pago</td><td style="text-align:right">${formatCurrency(sale.amountPaid)}</td></tr>
${sale.change > 0 ? `<tr><td>Cambio</td><td style="text-align:right">${formatCurrency(sale.change)}</td></tr>` : ''}
</table>${sale.orderNotes ? `<hr class="hr"><p>Notas: ${esc(sale.orderNotes)}</p>` : ''}
<hr class="hr"><p class="c">${esc(store.footer || '¡Gracias por su compra!')}</p></body></html>`;
}

// ─── Component ────────────────────────────────────────────────────────────────
export default function POSPage() {
    const {
        items, addItem, removeItem, updateQuantity, clear, restoreItems,
        applyDiscount, getSubtotal, getDiscountTotal, getItemCount,
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
    // Línea del ticket seleccionada con el teclado, para operarla sin mouse.
    const [selectedLine, setSelectedLine] = useState(0);
    // Facturación: solo tiene sentido con un cliente que tenga RFC capturado.
    const [requiereFactura, setRequiereFactura] = useState(false);
    const [showHelp, setShowHelp] = useState(false);
    const customerRef = useRef<HTMLInputElement>(null);
    const discountRefs = useRef<Record<string, HTMLInputElement | null>>({});
    const [customers, setCustomers] = useState<Customer[]>([]);
    const [customerId, setCustomerId] = useState<number | null>(null);
    const [orderNotes, setOrderNotes] = useState('');

    // Layaway (apartado) creation
    const [showLayaway, setShowLayaway] = useState(false);
    const [layawayInitial, setLayawayInitial] = useState('');
    const [layawayMethod, setLayawayMethod] = useState('cash');
    const [layawayDue, setLayawayDue] = useState('');

    // Variant picker
    const [variantPickerProduct, setVariantPickerProduct] = useState<Product | null>(null);
    const [variantOptions, setVariantOptions] = useState<ProductVariant[]>([]);
    const [orderSeq, setOrderSeq] = useState(() => Math.floor(Math.random() * 500) + 1000);

    const [showPayment, setShowPayment] = useState(false);
    const [paymentMethod, setPaymentMethod] = useState('cash');
    const [amountPaid, setAmountPaid] = useState('');
    // Non-cash legs of a mixed tender; cash covers whatever is left over.
    const [mixedCard, setMixedCard] = useState('');
    const [mixedTransfer, setMixedTransfer] = useState('');
    const [processing, setProcessing] = useState(false);
    // Un id por intento de cobro: si el envío se repite (doble clic, reintento
    // tras un cuelgue), el backend devuelve la venta original en vez de otra.
    const chargeRequestId = useRef<string>(crypto.randomUUID());
    // Id de la última venta, para poder reimprimir su ticket desde la base.
    const lastSaleId = useRef<number | null>(null);
    const [lastSale, setLastSale] = useState<CompletedSale | null>(null);

    const [toasts, setToasts] = useState<Toast[]>([]);

    const { holds, setHolds } = useHoldsStore();
    const [config, setConfig] = useState<Record<string, string>>({});

    const [promos, setPromos] = useState<Promotion[]>([]);
    const [promoInput, setPromoInput] = useState('');
    const [activePromo, setActivePromo] = useState<Promotion | null>(null);
    const [promoDropdown, setPromoDropdown] = useState(false);

    const searchRef = useRef<HTMLInputElement>(null);
    // Los ajustes del lector viven en un ref para que el manejador global no se
    // vuelva a montar cada vez que cambian.
    const scannerConfig = useRef(configFromSettings({}));
    // El manejador global se registra una sola vez; estos refs lo mantienen
    // apuntando al estado actual sin volver a montarlo en cada render.
    const selectedLineRef = useRef(0);
    const showHelpRef = useRef(false);
    const showLayawayRef = useRef(false);
    const variantPickerRef = useRef(false);
    const handleHoldRef = useRef<() => void>(() => {});
    const onScanRef = useRef<(code: string) => void>(() => {});
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
                const [cats, prods, promoList, cfg, custs] = await Promise.all([
                    api.getCategories(), api.getProducts({ is_active: true }), api.getPromotions(), api.getAllConfig(),
                    api.getCustomers().catch(() => []),
                ]);
                setCategories(cats.filter(c => c.is_active));
                setAllProducts(prods);
                setPromos(promoList.filter(p => p.is_active));
                setCustomers(custs.filter(c => c.is_active));
                const map: Record<string, string> = {};
                cfg.forEach(c => { map[c.key] = c.value; });
                setConfig(map);
                scannerConfig.current = configFromSettings(map);
            } catch (err) { showToast(String(err), 'error'); }
            finally { setLoadingProducts(false); }
        })();
    }, []);

    // Keyboard shortcuts + barcode scanner
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            const { showPayment: sp, items: its, lastSale: ls } = liveRef.current;
            const target = e.target as HTMLElement | null;
            const typing = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;

            if (e.key === 'F1') { e.preventDefault(); setShowHelp(h => !h); return; }
            if (e.key === 'F10') { e.preventDefault(); if (its.length > 0 && !sp && !ls) setShowPayment(true); return; }
            if (e.key === 'Escape') {
                if (sp) { e.preventDefault(); setShowPayment(false); return; }
                if (showHelpRef.current) { e.preventDefault(); setShowHelp(false); return; }
                // Salir de un campo devuelve el foco al buscador, que es donde
                // el cajero quiere estar entre una venta y otra.
                if (typing) { e.preventDefault(); (target as HTMLElement).blur(); searchRef.current?.focus(); }
                return;
            }
            if (e.key === 'F2') { e.preventDefault(); searchRef.current?.focus(); return; }
            if (e.key === 'F3') { e.preventDefault(); customerRef.current?.focus(); return; }

            // El resto opera sobre el ticket y no debe dispararse mientras se
            // escribe, salvo las teclas de función.
            const lines = liveRef.current.items;
            if (e.key === 'F4') {
                e.preventDefault();
                const line = lines[selectedLineRef.current];
                if (line) discountRefs.current[cartLineId(line)]?.focus();
                return;
            }
            if (e.key === 'F8') { e.preventDefault(); if (lines.length > 0) handleHoldRef.current(); return; }
            if (e.key === 'F9') { e.preventDefault(); if (lines.length > 0) { setServiceType('layaway'); setShowLayaway(true); } return; }

            // Con un modal abierto el escáner queda inhabilitado: escanear
            // mientras se cobra metía el producto al ticket por detrás, después
            // de que el cajero ya había visto el total.
            const modalAbierto = sp || ls || showHelpRef.current
                || showLayawayRef.current || variantPickerRef.current;
            if (modalAbierto) return;

            if (typing || lines.length === 0) { scanner(e); return; }

            const current = Math.min(selectedLineRef.current, lines.length - 1);
            const line = lines[current];

            switch (e.key) {
                case 'ArrowDown':
                    e.preventDefault();
                    setSelectedLine(Math.min(current + 1, lines.length - 1));
                    return;
                case 'ArrowUp':
                    e.preventDefault();
                    setSelectedLine(Math.max(current - 1, 0));
                    return;
                case 'ArrowRight':
                    e.preventDefault();
                    if (line && line.quantity < stockOf(line)) {
                        updateQuantity(cartLineId(line), line.quantity + 1);
                    }
                    return;
                case 'ArrowLeft':
                    e.preventDefault();
                    if (line) updateQuantity(cartLineId(line), line.quantity - 1);
                    return;
                case 'Delete':
                    e.preventDefault();
                    if (line) {
                        removeItem(cartLineId(line));
                        setSelectedLine(Math.max(0, current - 1));
                    }
                    return;
            }

            scanner(e);
        };
        // El escáner se captura aunque el foco esté en un campo: en el mostrador
        // el cursor casi siempre está en el buscador, y ahí es donde más se lee.
        const scanner = createScannerHandler({
            getConfig: () => scannerConfig.current,
            onScan: (code) => onScanRef.current(code),
        });
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, []); // eslint-disable-line react-hooks/exhaustive-deps

    const openVariantPicker = async (product: Product) => {
        setVariantPickerProduct(product);
        setVariantOptions([]);
        try { setVariantOptions(await api.getVariants(product.id)); } catch { setVariantOptions([]); }
    };

    const addVariantToCart = useCallback((product: Product, v: ProductVariant) => {
        const cv: CartVariant = { id: v.id, size: v.size, color: v.color, stock: v.stock };
        if (v.stock <= 0) { showToast(`Sin stock: ${product.name} ${variantLabel(cv)}`, 'error'); return; }
        const existing = liveRef.current.items.find(i => cartLineId(i) === lineIdOf(product.id, v.id));
        if (existing && existing.quantity >= v.stock) { showToast(`"${product.name} ${variantLabel(cv)}" alcanzó el máximo (${v.stock})`, 'warn'); return; }
        addItem(product, cv);
    }, [addItem, showToast]);

    const handleAddItem = useCallback((product: Product) => {
        if (product.stock <= 0) { showToast(`Sin stock: ${product.name}`, 'error'); return; }
        if (product.has_variants) { openVariantPicker(product); return; }
        const existing = liveRef.current.items.find(i => cartLineId(i) === lineIdOf(product.id, null));
        if (existing && existing.quantity >= product.stock) { showToast(`"${product.name}" ya alcanzó el máximo (${product.stock})`, 'warn'); return; }
        addItem(product);
    }, [addItem, showToast]);

    // El manejador global es estable; esto mantiene apuntando a la versión
    // actual sin volver a registrar el listener en cada render.
    onScanRef.current = (code: string) => { handleBarcodeScan(code); };
    selectedLineRef.current = selectedLine;
    showHelpRef.current = showHelp;
    showLayawayRef.current = showLayaway;
    variantPickerRef.current = variantPickerProduct !== null;
    handleHoldRef.current = () => handleHold();

    const handleBarcodeScan = async (code: string) => {
        try {
            const variant = await api.getVariantByBarcode(code);
            if (variant) {
                const product = allProducts.find(p => p.id === variant.product_id);
                if (product) { addVariantToCart(product, variant); setSearchQuery(''); setSearchResults([]); return; }
            }
            const product = await api.getProductByBarcode(code);
            if (product) { handleAddItem(product); setSearchQuery(''); setSearchResults([]); }
            else showToast('Producto no encontrado', 'error');
        } catch (err) { showToast(String(err), 'error'); }
    };

    const handleSearch = useCallback(async (query: string) => {
        setSearchQuery(query);
        if (query.length < 2) { setSearchResults([]); return; }
        try {
            const products = await api.getProducts({ search: query, is_active: true });
            setSearchResults(products.slice(0, 24));
        } catch (err) { showToast(String(err), 'error'); }
    }, []);

    const visibleProducts = useMemo(() => {
        if (searchQuery.length >= 2) return searchResults;
        if (activeCategory === 'all') return allProducts;
        return allProducts.filter(p => p.category_id === activeCategory);
    }, [activeCategory, allProducts, searchQuery.length, searchResults]);

    const filteredPromos = useMemo(() => {
        const valid = promos.filter(isPromoValidToday);
        if (!promoInput) return valid.slice(0, 6);
        const q = promoInput.toLowerCase();
        return valid.filter(p => p.name.toLowerCase().includes(q)).slice(0, 6);
    }, [promos, promoInput]);

    const promoDiscount = useMemo(() => {
        if (!activePromo || !isPromoValidToday(activePromo)) return 0;
        // Only the items the promo applies to contribute to its base.
        let base = 0;
        for (const it of items) {
            const lineNet = it.product.sale_price * it.quantity - it.discount;
            if (activePromo.applies_to === 'all') base += lineNet;
            else if (activePromo.applies_to === 'category' && it.product.category_id === activePromo.target_id) base += lineNet;
            else if (activePromo.applies_to === 'product' && it.product.id === activePromo.target_id) base += lineNet;
        }
        base = Math.max(0, base);
        if (base <= 0) return 0;
        if (activePromo.discount_type === 'percentage') return Math.round(base * (activePromo.discount_value / 100) * 100) / 100;
        return Math.min(activePromo.discount_value, base);
    }, [activePromo, items]);

    const handleApplyPromo = (promo: Promotion) => {
        setActivePromo(promo); setPromoInput(promo.name); setPromoDropdown(false);
        const label = promo.discount_type === 'percentage' ? `${promo.discount_value}% off` : `${formatCurrency(promo.discount_value)} off`;
        showToast(`Promo "${promo.name}" — ${label}`, 'success');
    };
    const handleRemovePromo = () => { setActivePromo(null); setPromoInput(''); showToast('Promoción removida', 'warn'); };

    const subtotal = getSubtotal();
    const lineDiscountTotal = getDiscountTotal();
    const taxableBase = Math.max(0, subtotal - lineDiscountTotal - promoDiscount);
    const taxRate = parseFloat(config.tax_rate || '0') || 0;
    const tax = Math.round(taxableBase * taxRate / 100 * 100) / 100;
    const total = Math.round((taxableBase + tax) * 100) / 100;
    const cashGiven = parseFloat(amountPaid) || 0;
    // El reparto del cobro mixto se calcula igual que en el backend; vive en
    // utils/cash para que ambas versiones estén sujetas a las mismas pruebas.
    const mixed = evaluateMixedTender(
        total, parseFloat(mixedCard) || 0, parseFloat(mixedTransfer) || 0, cashGiven,
    );
    const mixedNonCash = mixed.nonCash;
    const mixedCashDue = mixed.cashDue;
    const mixedInvalid = paymentMethod === 'mixed' && mixed.problem !== null;

    const changeAmount = paymentMethod === 'cash'
        ? round2(cashGiven - total)
        : paymentMethod === 'mixed'
            ? mixed.change
            : 0;

    const cannotCharge = processing || !cashRegisterId || mixedInvalid
        || (paymentMethod === 'cash' && cashGiven < total);

    const selectedCustomer = customerId ? customers.find(c => c.id === customerId) ?? null : null;

    // Cambiar de cliente no debe arrastrar la intención de facturar del anterior.
    useEffect(() => {
        if (!selectedCustomer?.rfc) setRequiereFactura(false);
    }, [selectedCustomer?.id, selectedCustomer?.rfc]);

    const storeInfo: StoreInfo = {
        name: config.store_name || '', address: config.store_address || '',
        phone: config.store_phone || '', footer: config.ticket_footer || '',
    };

    const handleHold = () => {
        if (items.length === 0) return;
        const emptyIdx = holds.findIndex(h => h === null);
        if (emptyIdx === -1) { showToast('Máximo 3 órdenes en espera', 'error'); return; }
        const next = [...holds];
        next[emptyIdx] = { items: items.map(i => ({ ...i })), customerName, orderNotes, serviceType, orderNo: orderSeq, promo: activePromo };
        setHolds(next);
        clear(); setCustomerName(''); setOrderNotes(''); setServiceType('direct'); setActivePromo(null); setPromoInput('');
        setOrderSeq(prev => prev + 1); showToast(`Orden #${orderSeq} en espera`, 'success');
    };

    const handleRestoreHold = (slotIdx: number) => {
        const slot = holds[slotIdx]; if (!slot) return;
        const newHolds = [...holds];
        if (items.length > 0) {
            const emptyIdx = holds.findIndex((h, i) => h === null && i !== slotIdx);
            if (emptyIdx === -1) { showToast('Libera un espacio antes de cambiar de orden', 'error'); return; }
            newHolds[emptyIdx] = { items: items.map(i => ({ ...i })), customerName, orderNotes, serviceType, orderNo: orderSeq, promo: activePromo };
        }
        newHolds[slotIdx] = null; setHolds(newHolds);
        restoreItems(slot.items); setCustomerName(slot.customerName); setOrderNotes(slot.orderNotes);
        setServiceType(slot.serviceType); setOrderSeq(slot.orderNo);
        setActivePromo(slot.promo); setPromoInput(slot.promo?.name || '');
        showToast(`Orden #${slot.orderNo} restaurada`, 'success');
    };

    const handleCompleteSale = async () => {
        if (items.length === 0 || !user) return;
        if (!cashRegisterId) { showToast('Abre la caja antes de cobrar', 'error'); setShowPayment(false); return; }
        if (cannotCharge) return;

        // Solo el pago mixto necesita desglose. Con un solo método se manda
        // vacío a propósito: el backend usa su propio total, y así una
        // diferencia de centavos entre lo que calculó la pantalla y lo que
        // calculó el servidor no convierte el cobro en un error incomprensible.
        const payments: PaymentSplit[] = paymentMethod === 'mixed'
            ? [
                ...(parseFloat(mixedCard) > 0 ? [{ method: 'card', amount: parseFloat(mixedCard) }] : []),
                ...(parseFloat(mixedTransfer) > 0 ? [{ method: 'transfer', amount: parseFloat(mixedTransfer) }] : []),
                ...(cashGiven > 0 ? [{ method: 'cash', amount: cashGiven }] : []),
              ]
            : [];

        const paid = paymentMethod === 'cash' ? cashGiven : total;
        setProcessing(true);
        const saleItems = items.map(i => ({ ...i }));
        const saleSubtotal = subtotal, saleLineDiscount = lineDiscountTotal, saleTax = tax;
        try {
            const sale = await api.createSale(cashRegisterId, {
                items: items.map(item => ({ product_id: item.product.id, quantity: item.quantity, unit_price: item.product.sale_price, discount: item.discount, variant_id: item.variant?.id ?? null })),
                payment_method: paymentMethod, amount_paid: paid, payments,
                discount_total: saleLineDiscount + promoDiscount,
                promotion_id: activePromo?.id ?? null,
                requiere_factura: requiereFactura,
                customer_id: customerId,
                client_request_id: chargeRequestId.current,
                notes: [SERVICE_LABELS[serviceType], customerName ? `Cliente: ${customerName}` : '', activePromo ? `Promo: ${activePromo.name}` : '', orderNotes.trim()].filter(Boolean).join(' | ') || null,
            });
            setLastSale({ folio: sale.folio, total: sale.total, change: sale.change_amount, items: saleItems, subtotal: saleSubtotal, lineDiscountTotal: saleLineDiscount, promoDiscount, tax: saleTax, paymentMethod, amountPaid: paid, serviceType, customerName, orderNotes });
            // Ticket térmico y apertura del cajón. Si no hay impresora
            // configurada queda el botón de imprimir por diálogo del sistema.
            lastSaleId.current = sale.id;
            // El backend recalcula descuentos e impuesto. Si su total no coincide
            // con el que vio el cajero, hay que decirlo, no cobrar en silencio.
            if (Math.abs(sale.total - total) > 0.01) {
                showToast(
                    `El total cobrado fue ${formatCurrency(sale.total)}, no ${formatCurrency(total)}. Revisa la promoción aplicada.`,
                    'warn',
                );
            }
            if (config.printer_auto_print !== '0') {
                api.printSaleReceipt(sale.id).catch((err) => {
                    if (String(err).includes('SIN_IMPRESORA')) return;
                    showToast(`No se pudo imprimir el ticket: ${err}`, 'error');
                });
            }
            // Reflect the sold units in the on-screen catalog immediately.
            const soldMap = new Map<number, number>();
            saleItems.forEach(i => soldMap.set(i.product.id, (soldMap.get(i.product.id) || 0) + i.quantity));
            const applySold = (list: Product[]) => list.map(p => soldMap.has(p.id) ? { ...p, stock: Math.max(0, p.stock - (soldMap.get(p.id) || 0)) } : p);
            setAllProducts(applySold);
            setSearchResults(applySold);
            chargeRequestId.current = crypto.randomUUID();
            clear(); setShowPayment(false); setAmountPaid(''); setMixedCard(''); setMixedTransfer(''); setCustomerName(''); setRequiereFactura(false); setCustomerId(null); setOrderNotes(''); setActivePromo(null); setPromoInput('');
            setOrderSeq(prev => prev + 1);
        } catch (err) { showToast(String(err), 'error'); }
        finally { setProcessing(false); }
    };

    // Apartado total = gross (no line/promo discounts apply to layaways).
    const layawayTotal = subtotal;

    const handleCreateLayaway = async () => {
        if (items.length === 0 || !user) return;
        const initial = parseFloat(layawayInitial) || 0;
        if (initial > layawayTotal) { showToast('El anticipo no puede superar el total', 'error'); return; }
        setProcessing(true);
        const soldMap = new Map<number, number>();
        items.forEach(i => soldMap.set(i.product.id, (soldMap.get(i.product.id) || 0) + i.quantity));
        try {
            await api.createLayaway({
                customer_id: customerId,
                notes: [customerName ? `Cliente: ${customerName}` : '', orderNotes.trim()].filter(Boolean).join(' | ') || null,
                due_date: layawayDue || null,
                initial_payment: initial,
                payment_method: layawayMethod,
                items: items.map(i => ({ product_id: i.product.id, quantity: i.quantity, unit_price: i.product.sale_price, variant_id: i.variant?.id ?? null })),
            });
            const applySold = (list: Product[]) => list.map(p => soldMap.has(p.id) ? { ...p, stock: Math.max(0, p.stock - (soldMap.get(p.id) || 0)) } : p);
            setAllProducts(applySold); setSearchResults(applySold);
            showToast('Apartado creado', 'success');
            clear(); setShowLayaway(false); setLayawayInitial(''); setLayawayDue('');
            setCustomerName(''); setCustomerId(null); setOrderNotes(''); setServiceType('direct'); setActivePromo(null); setPromoInput('');
            setOrderSeq(prev => prev + 1);
        } catch (err) { showToast(String(err), 'error'); }
        finally { setProcessing(false); }
    };

    /// Reimpresión manual: intenta el ticket térmico y, si no hay impresora
    /// configurada, cae al diálogo del sistema con el ticket en HTML.
    const handlePrint = async () => {
        if (!lastSale) return;
        if (lastSaleId.current !== null && config.printer_name) {
            try {
                await api.printSaleReceipt(lastSaleId.current, false);
                showToast('Ticket enviado a la impresora');
                return;
            } catch (err) {
                if (!String(err).includes('SIN_IMPRESORA')) {
                    showToast(`Impresora: ${err}`, 'error');
                    return;
                }
            }
        }
        printReceiptDialog();
    };

    const printReceiptDialog = () => {
        if (!lastSale) return;
        const html = buildReceiptHTML(lastSale, storeInfo);
        // Hidden iframe printing works reliably inside the Tauri webview,
        // where window.open can be blocked or return null.
        const iframe = document.createElement('iframe');
        iframe.style.position = 'fixed';
        iframe.style.right = '0';
        iframe.style.bottom = '0';
        iframe.style.width = '0';
        iframe.style.height = '0';
        iframe.style.border = '0';
        document.body.appendChild(iframe);
        const doc = iframe.contentWindow?.document;
        if (!doc) {
            const win = window.open('', '_blank', 'width=420,height=640');
            if (win) { win.document.write(html); win.document.close(); win.focus(); win.print(); }
            document.body.removeChild(iframe);
            return;
        }
        doc.open(); doc.write(html); doc.close();
        const run = () => {
            iframe.contentWindow?.focus();
            iframe.contentWindow?.print();
            setTimeout(() => { if (iframe.parentNode) document.body.removeChild(iframe); }, 1000);
        };
        // Give the browser a tick to lay out before printing.
        setTimeout(run, 200);
    };

    const isSearching = searchQuery.length >= 2;

    // ── Payment methods config ─────────────────────────────────────────────────
    const payMethods = [
        { k: 'cash', label: 'Efectivo', icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="1" y="4" width="22" height="16" rx="2"/><line x1="1" y1="10" x2="23" y2="10"/></svg> },
        { k: 'card', label: 'Tarjeta', icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg> },
        { k: 'transfer', label: 'Transfer.', icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><polyline points="17 1 21 5 17 9"/><path d="M3 11V9a4 4 0 014-4h14"/><polyline points="7 23 3 19 7 15"/><path d="M21 13v2a4 4 0 01-4 4H3"/></svg> },
        { k: 'mixed', label: 'Mixto', icon: <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><rect x="2" y="7" width="14" height="10" rx="2"/><path d="M8 17v2a2 2 0 002 2h10a2 2 0 002-2v-6a2 2 0 00-2-2h-2"/></svg> },
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
                            placeholder="Buscar producto o escanear... (F2)"
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

                    {/* Customer selector + free-text name */}
                    <div style={{ marginBottom: 14, display: 'flex', flexDirection: 'column', gap: 8 }}>
                        {customers.length > 0 && (
                            <select
                                value={customerId ?? ''}
                                onChange={e => {
                                    const id = e.target.value ? Number(e.target.value) : null;
                                    setCustomerId(id);
                                    const c = customers.find(cu => cu.id === id);
                                    if (c) setCustomerName(c.name);
                                }}
                                style={{ width: '100%', padding: '9px 12px', borderRadius: 11, background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.09)', color: T.t2, fontSize: 12, fontFamily: 'inherit', outline: 'none' }}
                            >
                                <option value="">Cliente registrado (opcional)…</option>
                                {customers.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
                            </select>
                        )}
                        {selectedCustomer?.rfc && (
                            <label style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 8, fontSize: 12, color: T.t2, cursor: 'pointer' }}>
                                <input
                                    type="checkbox"
                                    checked={requiereFactura}
                                    onChange={e => setRequiereFactura(e.target.checked)}
                                    style={{ width: 15, height: 15, accentColor: T.primary, cursor: 'pointer' }}
                                />
                                Requiere factura <span style={{ color: T.t3, fontFamily: 'monospace', fontSize: 11 }}>{selectedCustomer.rfc}</span>
                            </label>
                        )}
                        <input
                            ref={customerRef}
                            value={customerName} onChange={e => { setCustomerName(e.target.value); setCustomerId(null); }}
                            placeholder="Nombre del cliente (F3)"
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
                        ) : items.map((item, index) => {
                            const lineTotal = item.product.sale_price * item.quantity - item.discount;
                            const lineId = cartLineId(item);
                            const maxStock = stockOf(item);
                            const selected = index === Math.min(selectedLine, items.length - 1);
                            return (
                                <div
                                    key={lineId}
                                    className="cart-item"
                                    onClick={() => setSelectedLine(index)}
                                    style={selected ? { outline: `1px solid ${T.primary}`, outlineOffset: -1 } : undefined}
                                >
                                    <div className="cart-item-top">
                                        <CartThumb product={item.product} size={40} />
                                        <div style={{ flex: 1, minWidth: 0 }}>
                                            <p style={{ fontSize: 13, fontWeight: 700, color: T.t1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{item.product.name}</p>
                                            {item.variant
                                                ? <p style={{ fontSize: 11, color: T.primary, fontWeight: 700 }}>{variantLabel(item.variant)} · {formatCurrency(item.product.sale_price)} c/u</p>
                                                : <p style={{ fontSize: 11, color: T.t3, fontWeight: 600 }}>{formatCurrency(item.product.sale_price)} c/u</p>}
                                        </div>
                                        <button className="cart-item-del" onClick={() => removeItem(lineId)}>
                                            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a1 1 0 011-1h4a1 1 0 011 1v2"/></svg>
                                        </button>
                                    </div>
                                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 10 }}>
                                        {/* Qty */}
                                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                                            <button className="cart-qty-btn" onClick={() => updateQuantity(lineId, item.quantity - 1)}>−</button>
                                            <span style={{ fontSize: 14, fontWeight: 800, color: T.t1, width: 24, textAlign: 'center' }}>{item.quantity}</span>
                                            <button className="cart-qty-btn" onClick={() => updateQuantity(lineId, item.quantity + 1)} disabled={item.quantity >= maxStock}>+</button>
                                        </div>
                                        {/* Discount */}
                                        <div style={{ display: 'flex', alignItems: 'center', gap: 5, flex: 1 }}>
                                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke={T.t3} strokeWidth="2.2" strokeLinecap="round"><path d="M20.59 13.41l-7.17 7.17a2 2 0 01-2.83 0L2 12V2h10l8.59 8.59a2 2 0 010 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/></svg>
                                            <input
                                                ref={el => { discountRefs.current[lineId] = el; }}
                                                type="number" min="0" step="0.01"
                                                value={item.discount || ''}
                                                onChange={e => applyDiscount(lineId, Math.max(0, Math.min(parseFloat(e.target.value) || 0, item.product.sale_price * item.quantity)))}
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
                                {tax > 0 && (
                                    <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                                        <span style={{ fontSize: 13, color: T.t3 }}>IVA ({taxRate}%)</span>
                                        <span style={{ fontSize: 13, color: T.t2, fontWeight: 600 }}>{formatCurrency(tax)}</span>
                                    </div>
                                )}
                                <div style={{ display: 'flex', justifyContent: 'space-between', paddingTop: 10, borderTop: '1px solid rgba(255,255,255,0.07)', marginTop: 4 }}>
                                    <span style={{ fontSize: 16, fontWeight: 800, color: T.t1 }}>Total</span>
                                    <span style={{ fontSize: 20, fontWeight: 900, color: T.primary, fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(total)}</span>
                                </div>
                            </div>
                            <button
                                onClick={() => serviceType === 'layaway' ? setShowLayaway(true) : setShowPayment(true)}
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
                                {serviceType === 'layaway' ? 'Crear apartado' : 'Cobrar'}
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
                        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 10, marginBottom: 22 }}>
                            {payMethods.map(m => (
                                <button key={m.k} onClick={() => setPaymentMethod(m.k)} className={`pay-method-btn${paymentMethod === m.k ? ' active' : ''}`}>
                                    {m.icon}{m.label}
                                </button>
                            ))}
                        </div>

                        {/* Mixed tender: card/transfer legs, cash covers the rest */}
                        {paymentMethod === 'mixed' && (
                            <div style={{ marginBottom: 20 }}>
                                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 12 }}>
                                    <div>
                                        <label className="form-label">Tarjeta</label>
                                        <input type="number" step="1" min="0" value={mixedCard} autoFocus
                                            onChange={e => setMixedCard(e.target.value)} placeholder="0.00"
                                            style={{ width: '100%', padding: '11px 14px', borderRadius: 12, fontSize: 15, fontWeight: 700, background: 'rgba(255,255,255,0.06)', border: '1px solid rgba(255,255,255,0.15)', color: T.t1, fontFamily: 'inherit', outline: 'none', fontVariantNumeric: 'tabular-nums' }} />
                                    </div>
                                    <div>
                                        <label className="form-label">Transferencia</label>
                                        <input type="number" step="1" min="0" value={mixedTransfer}
                                            onChange={e => setMixedTransfer(e.target.value)} placeholder="0.00"
                                            style={{ width: '100%', padding: '11px 14px', borderRadius: 12, fontSize: 15, fontWeight: 700, background: 'rgba(255,255,255,0.06)', border: '1px solid rgba(255,255,255,0.15)', color: T.t1, fontFamily: 'inherit', outline: 'none', fontVariantNumeric: 'tabular-nums' }} />
                                    </div>
                                </div>

                                <label className="form-label">Efectivo recibido</label>
                                <input type="number" step="1" min="0" value={amountPaid}
                                    onChange={e => setAmountPaid(e.target.value)} placeholder="0.00"
                                    style={{ width: '100%', padding: '13px 16px', borderRadius: 13, fontSize: 18, fontWeight: 800, background: 'rgba(255,255,255,0.06)', border: '1px solid rgba(255,255,255,0.15)', color: T.t1, fontFamily: 'inherit', outline: 'none', fontVariantNumeric: 'tabular-nums' }} />

                                <div style={{ marginTop: 12, padding: '12px 16px', borderRadius: 13, background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.08)', display: 'flex', flexDirection: 'column', gap: 6 }}>
                                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, color: T.t3 }}>
                                        <span>Cubierto sin efectivo</span>
                                        <span style={{ fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(mixedNonCash)}</span>
                                    </div>
                                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 13, fontWeight: 700, color: T.t1 }}>
                                        <span>Falta en efectivo</span>
                                        <span style={{ fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(mixedCashDue)}</span>
                                    </div>
                                </div>

                                {mixed.problem && mixedNonCash > 0 && (
                                    <p style={{ marginTop: 10, fontSize: 12, color: T.danger }}>
                                        {mixed.problem}.
                                    </p>
                                )}

                                {changeAmount > 0 && !mixedInvalid && (
                                    <div style={{ marginTop: 12, padding: '12px 16px', borderRadius: 13, background: 'rgba(34,211,160,0.10)', border: '1px solid rgba(34,211,160,0.20)', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                                        <span style={{ fontSize: 13, fontWeight: 600, color: T.success }}>Cambio</span>
                                        <span style={{ fontSize: 18, fontWeight: 900, color: T.success, fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(changeAmount)}</span>
                                    </div>
                                )}
                            </div>
                        )}

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
                            disabled={cannotCharge}
                            style={{
                                width: '100%', padding: '14px', borderRadius: 14, border: 'none',
                                background: 'linear-gradient(135deg, #22D3A0, #18A880)',
                                color: '#001a13', fontSize: 15, fontWeight: 800, cursor: processing ? 'wait' : 'pointer',
                                fontFamily: 'inherit', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8,
                                boxShadow: '0 8px 28px rgba(34,211,160,0.35)', opacity: cannotCharge ? 0.45 : 1,
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

            {showHelp && <KeyboardHelp onClose={() => setShowHelp(false)} />}

            {/* ── Variant picker ── */}
            {variantPickerProduct && (
                <div className="modal-overlay" onClick={() => setVariantPickerProduct(null)}>
                    <div className="glass-modal scale-in" style={{ width: '100%', maxWidth: 420, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 6 }}>
                            <h2 style={{ fontSize: 18, fontWeight: 900, color: T.t1 }}>{variantPickerProduct.name}</h2>
                            <button onClick={() => setVariantPickerProduct(null)} style={{ width: 32, height: 32, borderRadius: 10, background: 'rgba(255,255,255,0.07)', border: '1px solid rgba(255,255,255,0.12)', display: 'grid', placeItems: 'center', color: T.t2, cursor: 'pointer' }}>
                                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                            </button>
                        </div>
                        <p style={{ fontSize: 12, color: T.t3, marginBottom: 16 }}>Elige talla / color</p>
                        {variantOptions.length === 0 ? (
                            <p style={{ textAlign: 'center', padding: '24px 0', fontSize: 13, color: T.t3 }}>Cargando variantes…</p>
                        ) : (
                            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 8 }}>
                                {variantOptions.map(v => {
                                    const label = variantLabel({ id: v.id, size: v.size, color: v.color, stock: v.stock });
                                    const out = v.stock <= 0;
                                    return (
                                        <button key={v.id} disabled={out} onClick={() => addVariantToCart(variantPickerProduct, v)}
                                            style={{
                                                display: 'flex', flexDirection: 'column', alignItems: 'flex-start', gap: 4, padding: '12px 14px', borderRadius: 12, cursor: out ? 'not-allowed' : 'pointer',
                                                background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.10)', fontFamily: 'inherit', opacity: out ? 0.5 : 1,
                                            }}>
                                            <span style={{ fontSize: 14, fontWeight: 800, color: T.t1 }}>{label}</span>
                                            <span style={{ fontSize: 11, fontWeight: 700, color: v.stock <= 3 ? T.danger : T.t3 }}>{out ? 'Agotado' : `${v.stock} disp.`}</span>
                                        </button>
                                    );
                                })}
                            </div>
                        )}
                        <button onClick={() => setVariantPickerProduct(null)} style={{ width: '100%', marginTop: 16, padding: '11px', borderRadius: 12, fontSize: 13, fontWeight: 700, color: T.t1, background: 'rgba(139,120,245,0.15)', border: '1px solid rgba(139,120,245,0.3)', cursor: 'pointer', fontFamily: 'inherit' }}>
                            Listo
                        </button>
                    </div>
                </div>
            )}

            {/* ── Layaway (apartado) modal ── */}
            {showLayaway && (
                <div className="modal-overlay">
                    <div className="glass-modal scale-in" style={{ width: '100%', maxWidth: 420, padding: 28 }}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                            <h2 style={{ fontSize: 20, fontWeight: 900, color: T.t1 }}>Nuevo apartado</h2>
                            <button onClick={() => setShowLayaway(false)} style={{ width: 34, height: 34, borderRadius: 10, background: 'rgba(255,255,255,0.07)', border: '1px solid rgba(255,255,255,0.12)', display: 'grid', placeItems: 'center', color: T.t2, cursor: 'pointer' }}>
                                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
                            </button>
                        </div>

                        <div style={{ padding: '16px 0', textAlign: 'center', marginBottom: 18, borderTop: '1px solid rgba(255,255,255,0.07)', borderBottom: '1px solid rgba(255,255,255,0.07)' }}>
                            <p style={{ fontSize: 12, fontWeight: 600, color: T.t3, textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 6 }}>Total del apartado</p>
                            <p style={{ fontSize: 34, fontWeight: 900, color: T.t1, fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(layawayTotal)}</p>
                        </div>

                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div>
                                <label className="form-label">Anticipo</label>
                                <input type="number" step="0.01" value={layawayInitial} onChange={e => setLayawayInitial(e.target.value)} placeholder="0.00"
                                    style={{ width: '100%', padding: '12px 14px', borderRadius: 12, fontSize: 16, fontWeight: 700, background: 'rgba(255,255,255,0.06)', border: '1px solid rgba(255,255,255,0.15)', color: T.t1, fontFamily: 'inherit', outline: 'none', fontVariantNumeric: 'tabular-nums' }} autoFocus />
                            </div>
                            <div style={{ display: 'flex', gap: 10 }}>
                                <div style={{ flex: 1 }}>
                                    <label className="form-label">Método</label>
                                    <select value={layawayMethod} onChange={e => setLayawayMethod(e.target.value)} className="input">
                                        <option value="cash">Efectivo</option>
                                        <option value="card">Tarjeta</option>
                                        <option value="transfer">Transferencia</option>
                                    </select>
                                </div>
                                <div style={{ flex: 1 }}>
                                    <label className="form-label">Fecha límite</label>
                                    <input type="date" value={layawayDue} onChange={e => setLayawayDue(e.target.value)} className="input" />
                                </div>
                            </div>
                            <div style={{ display: 'flex', justifyContent: 'space-between', padding: '10px 14px', borderRadius: 11, background: 'rgba(245,168,66,0.08)', border: '1px solid rgba(245,168,66,0.2)' }}>
                                <span style={{ fontSize: 13, fontWeight: 600, color: T.warning }}>Saldo pendiente</span>
                                <span style={{ fontSize: 15, fontWeight: 900, color: T.warning, fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(Math.max(0, layawayTotal - (parseFloat(layawayInitial) || 0)))}</span>
                            </div>
                            <button onClick={handleCreateLayaway} disabled={processing}
                                style={{ width: '100%', padding: '14px', borderRadius: 14, border: 'none', background: 'linear-gradient(135deg, #8B78F5, #6B56E0)', color: '#fff', fontSize: 15, fontWeight: 800, cursor: processing ? 'wait' : 'pointer', fontFamily: 'inherit', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8, opacity: processing ? 0.6 : 1 }}>
                                {processing ? 'Procesando...' : 'Confirmar apartado'}
                            </button>
                        </div>
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
