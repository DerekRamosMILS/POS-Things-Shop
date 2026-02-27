import { useEffect, useRef, useState, useCallback } from 'react';
import { useCartStore } from '../stores/useCartStore';
import { useSessionStore } from '../stores/useSessionStore';
import { formatCurrency } from '../utils';
import * as api from '../api';
import type { Product } from '../types';
import {
    Search,
    Plus,
    Minus,
    Trash2,
    ShoppingCart,
    CreditCard,
    Banknote,
    ArrowRightLeft,
    X,
    Check,
    Loader2,
} from 'lucide-react';

export default function POSPage() {
    const { items, addItem, removeItem, updateQuantity, clear, getSubtotal, getDiscountTotal, getTotal, getItemCount } = useCartStore();
    const { user, cashRegisterId } = useSessionStore();
    const [searchQuery, setSearchQuery] = useState('');
    const [searchResults, setSearchResults] = useState<Product[]>([]);
    const [showPayment, setShowPayment] = useState(false);
    const [paymentMethod, setPaymentMethod] = useState('cash');
    const [amountPaid, setAmountPaid] = useState('');
    const [processing, setProcessing] = useState(false);
    const [lastSale, setLastSale] = useState<{ folio: string; total: number; change: number } | null>(null);
    const searchRef = useRef<HTMLInputElement>(null);
    const barcodeBuffer = useRef('');
    const barcodeTimeout = useRef<number | null>(null);

    // Barcode scanner listener
    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            const target = e.target as HTMLElement;
            if (target.tagName === 'INPUT' && target !== searchRef.current) return;

            if (e.key === 'Enter' && barcodeBuffer.current.length >= 4) {
                e.preventDefault();
                handleBarcodeScan(barcodeBuffer.current);
                barcodeBuffer.current = '';
                return;
            }

            if (e.key.length === 1) {
                barcodeBuffer.current += e.key;
                if (barcodeTimeout.current) clearTimeout(barcodeTimeout.current);
                barcodeTimeout.current = window.setTimeout(() => {
                    barcodeBuffer.current = '';
                }, 100);
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, []);

    const handleBarcodeScan = async (code: string) => {
        try {
            const product = await api.getProductByBarcode(code);
            if (product) {
                addItem(product);
                setSearchQuery('');
                setSearchResults([]);
            }
        } catch (err) {
            console.error('Barcode scan error:', err);
        }
    };

    const handleSearch = useCallback(async (query: string) => {
        setSearchQuery(query);
        if (query.length < 2) {
            setSearchResults([]);
            return;
        }
        try {
            const products = await api.getProducts({ search: query, is_active: true });
            setSearchResults(products.slice(0, 10));
        } catch (err) {
            console.error(err);
        }
    }, []);

    const handleCompleteSale = async () => {
        if (items.length === 0 || !user) return;

        const total = getTotal();
        const paid = paymentMethod === 'cash' ? parseFloat(amountPaid) || 0 : total;

        if (paymentMethod === 'cash' && paid < total) return;

        setProcessing(true);
        try {
            const sale = await api.createSale(user.id, cashRegisterId, {
                items: items.map((item) => ({
                    product_id: item.product.id,
                    quantity: item.quantity,
                    unit_price: item.product.sale_price,
                    discount: item.discount,
                })),
                payment_method: paymentMethod,
                amount_paid: paid,
                discount_total: getDiscountTotal(),
            });

            setLastSale({
                folio: sale.folio,
                total: sale.total,
                change: sale.change_amount,
            });
            clear();
            setShowPayment(false);
            setAmountPaid('');
        } catch (err) {
            alert(String(err));
        } finally {
            setProcessing(false);
        }
    };

    // Quick amount button — ADDITIVE behavior
    const handleQuickAmount = (amount: number) => {
        const current = parseFloat(amountPaid) || 0;
        setAmountPaid((current + amount).toString());
    };

    const total = getTotal();
    const changeAmount = paymentMethod === 'cash' ? (parseFloat(amountPaid) || 0) - total : 0;

    return (
        <div className="h-full flex animate-fade-in">
            {/* Left: Product Search */}
            <div className="flex-1 flex flex-col border-r border-border">
                {/* Search Bar */}
                <div className="p-8 lg:p-10 border-b border-border">
                    <div className="relative">
                        <Search className="absolute left-8 top-1/2 -translate-y-1/2 text-text-muted" size={32} />
                        <input
                            ref={searchRef}
                            type="text"
                            value={searchQuery}
                            onChange={(e) => handleSearch(e.target.value)}
                            placeholder="Buscar producto o escanear código de barras..."
                            className="w-full pl-[88px] pr-8 py-6 lg:py-8 bg-bg-secondary border border-border rounded-2xl text-text-primary text-2xl placeholder-text-muted focus:border-primary transition-colors shadow-[inset_0_2px_10px_rgba(0,0,0,0.2)]"
                            data-barcode="true"
                            autoFocus
                        />
                    </div>
                </div>

                {/* Search Results */}
                <div className="flex-1 overflow-y-auto p-8 lg:p-10">
                    {searchResults.length > 0 ? (
                        <div className="grid grid-cols-2 xl:grid-cols-3 gap-6">
                            {searchResults.map((product) => (
                                <button
                                    key={product.id}
                                    onClick={() => {
                                        addItem(product);
                                        setSearchQuery('');
                                        setSearchResults([]);
                                        searchRef.current?.focus();
                                    }}
                                    disabled={product.stock <= 0}
                                    className="text-left p-8 bg-bg-secondary border border-border rounded-2xl hover:border-primary/40 hover:bg-surface-hover transition-all duration-150 disabled:opacity-40 disabled:cursor-not-allowed hover:-translate-y-1 hover:shadow-[0_10px_30px_rgba(0,224,90,0.05)]"
                                >
                                    <p className="text-2xl font-bold text-text-primary truncate">{product.name}</p>
                                    <p className="text-lg font-mono tracking-wider text-text-muted mt-2">{product.sku}</p>
                                    <div className="flex items-end justify-between mt-6">
                                        <p className="text-4xl font-black text-primary tracking-tight drop-shadow-[0_0_8px_rgba(0,224,90,0.2)]">{formatCurrency(product.sale_price)}</p>
                                        <span className={`text-xl font-bold px-5 py-2.5 rounded-xl ${product.stock <= product.min_stock ? 'bg-danger/10 text-danger border border-danger/30 shadow-[0_0_15px_rgba(244,63,94,0.15)]' : 'bg-success/10 text-success border border-success/30'}`}>
                                            Stock: {product.stock}
                                        </span>
                                    </div>
                                </button>
                            ))}
                        </div>
                    ) : searchQuery.length >= 2 ? (
                        <div className="flex flex-col items-center justify-center h-full text-text-muted">
                            <PackageIcon size={56} className="mb-3 opacity-30" />
                            <p className="text-base">No se encontraron productos</p>
                        </div>
                    ) : (
                        <div className="flex flex-col items-center justify-center h-full text-text-muted">
                            <ShoppingCart size={56} className="mb-4 opacity-20" />
                            <p className="text-base">Busca un producto o escanea un código de barras</p>
                            <p className="text-sm mt-1 text-text-muted">El escáner USB funciona automáticamente</p>
                        </div>
                    )}
                </div>
            </div>

            {/* Right: Cart */}
            <div className="w-[580px] xl:w-[680px] flex flex-col bg-bg-secondary shrink-0">
                {/* Cart Header */}
                <div className="px-10 py-8 border-b border-border flex items-center justify-between">
                    <h2 className="text-3xl font-black tracking-tight text-text-primary flex items-center gap-4">
                        <ShoppingCart size={36} className="text-primary" />
                        Carrito ({getItemCount()})
                    </h2>
                    {items.length > 0 && (
                        <button
                            onClick={clear}
                            className="text-xl font-bold text-danger hover:text-danger-hover transition-colors px-6 py-3 rounded-xl hover:bg-danger/10"
                        >
                            Vaciar
                        </button>
                    )}
                </div>

                {/* Cart Items */}
                <div className="flex-1 overflow-y-auto p-6 space-y-4">
                    {items.length === 0 ? (
                        <div className="flex flex-col items-center justify-center h-full text-text-muted">
                            <ShoppingCart size={64} strokeWidth={1.5} className="mb-4 opacity-20" />
                            <p className="text-xl font-medium tracking-wide">Carrito vacío</p>
                        </div>
                    ) : (
                        items.map((item) => (
                            <div key={item.product.id} className="bg-bg-primary rounded-2xl p-8 border border-border animate-fade-in shadow-[0_4px_15px_rgba(0,0,0,0.2)]">
                                <div className="flex items-start justify-between mb-6">
                                    <div className="flex-1 min-w-0 pr-4">
                                        <p className="text-2xl font-bold text-text-primary truncate">{item.product.name}</p>
                                        <p className="text-xl text-text-muted mt-2 font-mono">{formatCurrency(item.product.sale_price)} c/u</p>
                                    </div>
                                    <button
                                        onClick={() => removeItem(item.product.id)}
                                        className="text-text-muted hover:text-danger hover:bg-danger/10 transition-all p-4 rounded-xl border border-transparent hover:border-danger/30 hover:scale-110"
                                    >
                                        <Trash2 size={28} />
                                    </button>
                                </div>
                                <div className="flex items-center justify-between">
                                    <div className="flex items-center gap-4">
                                        <button
                                            onClick={() => updateQuantity(item.product.id, item.quantity - 1)}
                                            className="w-16 h-16 rounded-2xl bg-bg-secondary border border-border flex items-center justify-center hover:bg-surface-hover hover:border-primary/40 transition-colors"
                                        >
                                            <Minus size={24} />
                                        </button>
                                        <span className="w-16 text-center text-3xl font-black font-mono tracking-wider text-text-primary">{item.quantity}</span>
                                        <button
                                            onClick={() => updateQuantity(item.product.id, item.quantity + 1)}
                                            disabled={item.quantity >= item.product.stock}
                                            className="w-16 h-16 rounded-2xl bg-bg-secondary border border-border flex items-center justify-center hover:bg-surface-hover hover:border-primary/40 transition-colors disabled:opacity-30 disabled:hover:border-border"
                                        >
                                            <Plus size={24} />
                                        </button>
                                    </div>
                                    <p className="text-4xl font-black tracking-tight text-primary drop-shadow-[0_0_8px_rgba(0,224,90,0.2)]">
                                        {formatCurrency(item.product.sale_price * item.quantity - item.discount)}
                                    </p>
                                </div>
                            </div>
                        ))
                    )}
                </div>

                {/* Cart Totals & Pay */}
                {items.length > 0 && (
                    <div className="border-t border-border p-10 space-y-8 bg-bg-primary">
                        <div className="space-y-4">
                            <div className="flex justify-between text-2xl font-bold text-text-secondary">
                                <span>Subtotal</span>
                                <span className="font-mono">{formatCurrency(getSubtotal())}</span>
                            </div>
                            {getDiscountTotal() > 0 && (
                                <div className="flex justify-between text-2xl font-bold text-warning">
                                    <span>Descuento</span>
                                    <span className="font-mono">-{formatCurrency(getDiscountTotal())}</span>
                                </div>
                            )}
                            <div className="flex justify-between text-5xl font-black tracking-tighter text-text-primary pt-6 border-t border-border/50">
                                <span>Total</span>
                                <span className="text-primary drop-shadow-[0_0_15px_rgba(0,224,90,0.3)]">{formatCurrency(total)}</span>
                            </div>
                        </div>

                        <button
                            onClick={() => setShowPayment(true)}
                            className="w-full py-8 bg-primary hover:bg-primary-hover text-[#0B0B0F] text-3xl font-black tracking-wide rounded-[24px] transition-all duration-200 flex items-center justify-center gap-4 shadow-[0_4px_20px_rgba(0,224,90,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,224,90,0.4)]"
                        >
                            <CreditCard size={36} strokeWidth={3} />
                            Cobrar
                        </button>
                    </div>
                )}
            </div>

            {/* Payment Modal */}
            {showPayment && (
                <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 animate-fade-in p-6">
                    <div className="bg-bg-secondary border border-white/10 rounded-[32px] w-full max-w-4xl p-12 lg:p-16 animate-fade-in shadow-[0_20px_80px_rgba(0,0,0,0.9)] max-h-[90vh] overflow-y-auto">
                        <div className="flex items-center justify-between mb-12">
                            <h3 className="text-4xl font-black tracking-tight text-text-primary">Procesar Pago</h3>
                            <button onClick={() => setShowPayment(false)} className="text-text-muted hover:text-text-primary p-3 rounded-2xl hover:bg-white/5 transition-colors">
                                <X size={36} />
                            </button>
                        </div>

                        <div className="text-center mb-12 p-10 bg-bg-primary rounded-3xl border border-border">
                            <p className="text-text-muted text-2xl font-bold tracking-wider uppercase mb-2">Total a cobrar</p>
                            <p className="text-8xl font-black tracking-tighter text-primary mt-4 drop-shadow-[0_0_25px_rgba(0,224,90,0.4)]">{formatCurrency(total)}</p>
                        </div>

                        {/* Payment Methods */}
                        <div className="grid grid-cols-3 gap-8 mb-12">
                            {[
                                { key: 'cash', label: 'Efectivo', icon: Banknote },
                                { key: 'card', label: 'Tarjeta', icon: CreditCard },
                                { key: 'transfer', label: 'Transferencia', icon: ArrowRightLeft },
                            ].map((pm) => (
                                <button
                                    key={pm.key}
                                    onClick={() => setPaymentMethod(pm.key)}
                                    className={`flex flex-col items-center gap-4 p-8 rounded-3xl border-2 transition-all ${paymentMethod === pm.key
                                        ? 'bg-primary/10 border-primary text-primary shadow-[0_4px_30px_rgba(0,224,90,0.2)] scale-105'
                                        : 'border-border text-text-secondary hover:border-border-light hover:bg-surface-hover'
                                        }`}
                                >
                                    <pm.icon size={48} strokeWidth={paymentMethod === pm.key ? 2.5 : 2} />
                                    <span className="text-2xl font-bold">{pm.label}</span>
                                </button>
                            ))}
                        </div>

                        {/* Cash amount */}
                        {paymentMethod === 'cash' && (
                            <div className="mb-12">
                                <label className="text-2xl font-bold text-text-secondary mb-4 block">Monto recibido</label>
                                <input
                                    type="number"
                                    value={amountPaid}
                                    onChange={(e) => setAmountPaid(e.target.value)}
                                    className="w-full px-8 py-8 bg-bg-primary border-2 border-border rounded-3xl text-text-primary text-5xl font-mono tracking-wider text-center focus:border-primary transition-colors focus:shadow-[0_0_20px_rgba(0,224,90,0.2)]"
                                    placeholder="0.00"
                                    autoFocus
                                    step="0.01"
                                />
                                {changeAmount > 0 && (
                                    <div className="mt-8 p-8 bg-success/10 border-2 border-success/30 rounded-3xl text-center shadow-[inset_0_2px_20px_rgba(16,185,129,0.15)]">
                                        <p className="text-2xl font-black text-success uppercase tracking-widest mb-2">Cambio a entregar</p>
                                        <p className="text-7xl font-black tracking-tight text-success drop-shadow-[0_0_15px_rgba(16,185,129,0.4)]">{formatCurrency(changeAmount)}</p>
                                    </div>
                                )}
                                {/* Quick amounts */}
                                <div className="grid grid-cols-4 gap-6 mt-8">
                                    {[50, 100, 200, 500].map((amount) => (
                                        <button
                                            key={amount}
                                            onClick={() => handleQuickAmount(amount)}
                                            className="py-6 bg-bg-primary border-2 border-border rounded-2xl text-text-secondary text-3xl font-bold font-mono tracking-wider hover:bg-surface-hover hover:border-primary/50 hover:text-primary transition-colors"
                                        >
                                            +${amount}
                                        </button>
                                    ))}
                                </div>
                            </div>
                        )}

                        <button
                            onClick={handleCompleteSale}
                            disabled={processing || (paymentMethod === 'cash' && (parseFloat(amountPaid) || 0) < total)}
                            className="w-full py-8 bg-success hover:bg-success-hover text-[#0B0B0F] text-4xl font-black tracking-wide rounded-3xl transition-all duration-200 flex items-center justify-center gap-4 disabled:opacity-40 disabled:cursor-not-allowed shadow-[0_8px_30px_rgba(16,185,129,0.3)] hover:shadow-[0_12px_40px_rgba(16,185,129,0.5)] disabled:shadow-none hover:-translate-y-2 disabled:hover:translate-y-0"
                        >
                            {processing ? (
                                <Loader2 className="w-12 h-12 animate-spin" />
                            ) : (
                                <>
                                    <Check size={40} strokeWidth={4} />
                                    Completar Venta
                                </>
                            )}
                        </button>
                    </div>
                </div>
            )}

            {/* Success Modal */}
            {lastSale && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
                    <div className="bg-bg-secondary border border-border rounded-2xl w-full max-w-md p-10 text-center animate-fade-in">
                        <div className="w-24 h-24 bg-success/10 border border-success/20 rounded-full flex items-center justify-center mx-auto mb-6">
                            <Check size={44} className="text-success" />
                        </div>
                        <h3 className="text-2xl font-bold text-text-primary mb-2">¡Venta Completada!</h3>
                        <p className="text-text-muted text-base mb-5">Folio: {lastSale.folio}</p>
                        <p className="text-4xl font-bold text-success mb-2">{formatCurrency(lastSale.total)}</p>
                        {lastSale.change > 0 && (
                            <p className="text-xl text-warning">Cambio: {formatCurrency(lastSale.change)}</p>
                        )}
                        <button
                            onClick={() => {
                                setLastSale(null);
                                searchRef.current?.focus();
                            }}
                            className="w-full mt-8 py-4 bg-primary hover:bg-primary-hover text-white text-lg font-semibold rounded-2xl transition-all"
                        >
                            Nueva Venta
                        </button>
                    </div>
                </div>
            )}
        </div>
    );
}

function PackageIcon(props: { size: number; className?: string }) {
    return (
        <svg width={props.size} height={props.size} className={props.className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="m7.5 4.27 9 5.15" /><path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" /><path d="m3.3 7 8.7 5 8.7-5" /><path d="M12 22V12" />
        </svg>
    );
}
