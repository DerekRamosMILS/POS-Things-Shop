import { useEffect, useState } from 'react';
import { formatDateTime, MOVEMENT_TYPE_LABELS } from '../utils';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { InventoryMovement, Product } from '../types';
import { Boxes, ArrowDownCircle, AlertTriangle, Plus, Loader2, X } from 'lucide-react';

export default function InventoryPage() {
    const { user } = useSessionStore();
    const [movements, setMovements] = useState<InventoryMovement[]>([]);
    const [lowStock, setLowStock] = useState<Product[]>([]);
    const [loading, setLoading] = useState(true);
    const [showAdjust, setShowAdjust] = useState(false);
    const [showPurchase, setShowPurchase] = useState(false);
    const [products, setProducts] = useState<Product[]>([]);
    const [adjustForm, setAdjustForm] = useState({ product_id: 0, quantity: 0, reason: '' });
    const [purchaseForm, setPurchaseForm] = useState({ product_id: 0, quantity: 0, purchase_price: '' });
    const [processing, setProcessing] = useState(false);

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [m, ls, prods] = await Promise.all([api.getInventoryMovements(), api.getLowStockProducts(), api.getProducts({ is_active: true })]);
            setMovements(m); setLowStock(ls); setProducts(prods);
        } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    const handleAdjust = async () => {
        if (!user || !adjustForm.product_id || adjustForm.quantity === 0 || !adjustForm.reason) return;
        setProcessing(true);
        try { await api.adjustStock(user.id, adjustForm); setShowAdjust(false); loadData(); } catch (err) { alert(String(err)); } finally { setProcessing(false); }
    };

    const handlePurchase = async () => {
        if (!user || !purchaseForm.product_id || purchaseForm.quantity <= 0) return;
        setProcessing(true);
        try { await api.registerPurchase(user.id, { product_id: purchaseForm.product_id, quantity: purchaseForm.quantity, purchase_price: purchaseForm.purchase_price ? parseFloat(purchaseForm.purchase_price) : undefined }); setShowPurchase(false); loadData(); } catch (err) { alert(String(err)); } finally { setProcessing(false); }
    };

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 lg:p-14 xl:p-16 h-full flex flex-col animate-fade-in">
            <div className="flex items-center justify-between mb-10">
                <div>
                    <h1 className="text-4xl font-bold text-text-primary">Inventario</h1>
                    <p className="text-text-secondary text-lg mt-2">{lowStock.length} productos con stock bajo</p>
                </div>
                <div className="flex gap-4">
                    <button onClick={() => { setPurchaseForm({ product_id: 0, quantity: 0, purchase_price: '' }); setShowPurchase(true); }} className="flex items-center gap-3 px-8 py-4 bg-success hover:bg-success-hover text-[#0B0B0F] rounded-2xl font-bold text-base transition-colors shadow-[0_4px_20px_rgba(16,185,129,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(16,185,129,0.4)]">
                        <ArrowDownCircle size={22} strokeWidth={2.5} /> Registrar Compra
                    </button>
                    <button onClick={() => { setAdjustForm({ product_id: 0, quantity: 0, reason: '' }); setShowAdjust(true); }} className="flex items-center gap-3 px-8 py-4 bg-primary hover:bg-primary-hover text-[#0B0B0F] rounded-2xl font-bold text-base transition-colors shadow-[0_4px_20px_rgba(0,224,90,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,224,90,0.4)]">
                        <Plus size={22} strokeWidth={2.5} /> Ajuste Manual
                    </button>
                </div>
            </div>

            {lowStock.length > 0 && (
                <div className="glass rounded-[32px] border border-warning/30 p-12 mb-12 bg-warning/10 shadow-[0_10px_40px_rgba(245,158,11,0.15)]">
                    <h2 className="text-3xl font-bold text-warning mb-8 flex items-center gap-4"><AlertTriangle size={36} strokeWidth={2.5} /> Productos con Stock Crítico</h2>
                    <div className="grid grid-cols-2 lg:grid-cols-4 gap-8">
                        {lowStock.slice(0, 8).map((p) => (
                            <div key={p.id} className="bg-bg-primary rounded-3xl p-8 lg:p-10 border border-warning/20 shadow-[0_12px_40px_rgba(245,158,11,0.1)]">
                                <p className="text-2xl text-text-primary truncate font-bold mb-4">{p.name}</p>
                                <div className="flex items-end justify-between mt-6">
                                    <span className="text-xl font-medium text-text-muted">Min: {p.min_stock}</span>
                                    <span className={`text-5xl font-black tracking-tighter drop-shadow-[0_0_10px_rgba(245,158,11,0.3)] ${p.stock === 0 ? 'text-danger drop-shadow-[0_0_15px_rgba(244,63,94,0.4)]' : 'text-warning'}`}>{p.stock}</span>
                                </div>
                            </div>
                        ))}
                    </div>
                </div>
            )}

            <div className="glass rounded-[32px] border border-border flex-1 flex flex-col overflow-hidden shadow-[0_10px_50px_rgba(0,0,0,0.5)]">
                <div className="p-8 lg:p-10 pb-0 flex flex-col sm:flex-row sm:items-center justify-between gap-6">
                    <h2 className="text-3xl font-bold text-text-primary mb-6 flex items-center gap-4">
                        <Boxes size={32} className="text-primary" /> Historial de Movimientos
                    </h2>
                </div>
                <div className="overflow-auto flex-1 p-2">
                    <table className="w-full border-collapse">
                        <thead>
                            <tr className="border-b border-border/60">
                                <th className="text-left text-base font-bold tracking-wide text-text-muted uppercase px-10 py-6">Producto</th>
                                <th className="text-left text-base font-bold tracking-wide text-text-muted uppercase px-10 py-6">Tipo</th>
                                <th className="text-center text-base font-bold tracking-wide text-text-muted uppercase px-10 py-6">Cantidad</th>
                                <th className="text-center text-base font-bold tracking-wide text-text-muted uppercase px-10 py-6">Stock Anterior</th>
                                <th className="text-center text-base font-bold tracking-wide text-text-muted uppercase px-10 py-6">Stock Nuevo</th>
                                <th className="text-left text-base font-bold tracking-wide text-text-muted uppercase px-10 py-6">Motivo</th>
                                <th className="text-left text-base font-bold tracking-wide text-text-muted uppercase px-10 py-6">Fecha</th>
                            </tr>
                        </thead>
                        <tbody>
                            {movements.slice(0, 50).map((m) => (
                                <tr key={m.id} className="border-b border-border/30 hover:bg-white/5 transition-colors">
                                    <td className="px-10 py-6 text-2xl font-bold text-text-primary">{m.product_name}</td>
                                    <td className="px-10 py-6"><span className={`text-xl font-bold px-4 py-2 rounded-xl border border-transparent ${m.quantity > 0 ? 'bg-success/10 text-success border-success/30' : 'bg-danger/10 text-danger border-danger/30'}`}>{MOVEMENT_TYPE_LABELS[m.movement_type] || m.movement_type}</span></td>
                                    <td className={`px-10 py-6 text-center text-3xl font-black font-mono tracking-tight drop-shadow-[0_0_8px_rgba(255,255,255,0.1)] ${m.quantity > 0 ? 'text-success' : 'text-danger'}`}>{m.quantity > 0 ? '+' : ''}{m.quantity}</td>
                                    <td className="px-10 py-6 text-center text-xl text-text-secondary font-mono tracking-wider">{m.previous_stock}</td>
                                    <td className="px-10 py-6 text-center text-3xl text-text-primary font-bold font-mono tracking-tight drop-shadow-[0_0_8px_rgba(255,255,255,0.2)]">{m.new_stock}</td>
                                    <td className="px-10 py-6 text-xl text-text-muted">{m.reason || '—'}</td>
                                    <td className="px-10 py-6 text-lg text-text-muted font-mono tracking-wide">{formatDateTime(m.created_at)}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {movements.length === 0 && <div className="py-24 text-center text-text-muted"><Boxes size={72} strokeWidth={1.5} className="mx-auto mb-5 opacity-30" /><p className="text-xl font-medium tracking-wide">Sin movimientos registrados</p></div>}
                </div>
            </div>

            {/* Adjust Modal */}
            {showAdjust && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
                    <div className="bg-bg-secondary border border-border rounded-2xl w-full max-w-xl p-10 animate-fade-in">
                        <div className="flex justify-between mb-6">
                            <h3 className="text-xl font-bold text-text-primary">Ajuste de Stock</h3>
                            <button onClick={() => setShowAdjust(false)} className="text-text-muted hover:text-text-primary"><X size={22} /></button>
                        </div>
                        <div className="space-y-6">
                            <div>
                                <label className="text-sm font-medium text-text-secondary block mb-2">Producto</label>
                                <select value={adjustForm.product_id} onChange={(e) => setAdjustForm({ ...adjustForm, product_id: Number(e.target.value) })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm">
                                    <option value={0}>Seleccionar producto...</option>
                                    {products.map(p => <option key={p.id} value={p.id}>{p.name} (Stock: {p.stock})</option>)}
                                </select>
                            </div>
                            <div>
                                <label className="text-sm font-medium text-text-secondary block mb-2">Cantidad (+/-)</label>
                                <input type="number" value={adjustForm.quantity} onChange={(e) => setAdjustForm({ ...adjustForm, quantity: parseInt(e.target.value) || 0 })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" />
                            </div>
                            <div>
                                <label className="text-sm font-medium text-text-secondary block mb-2">Motivo *</label>
                                <input type="text" value={adjustForm.reason} onChange={(e) => setAdjustForm({ ...adjustForm, reason: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" placeholder="Ej: Merma, inventario físico..." />
                            </div>
                        </div>
                        <div className="flex gap-4 mt-8">
                            <button onClick={() => setShowAdjust(false)} className="flex-1 py-4 bg-bg-primary border border-border rounded-2xl text-text-secondary text-sm font-medium hover:bg-surface-hover transition-colors">Cancelar</button>
                            <button onClick={handleAdjust} disabled={processing} className="flex-1 py-4 bg-primary text-white rounded-2xl text-sm font-medium flex items-center justify-center gap-2">{processing && <Loader2 size={18} className="animate-spin" />}Aplicar</button>
                        </div>
                    </div>
                </div>
            )}

            {/* Purchase Modal */}
            {showPurchase && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
                    <div className="bg-bg-secondary border border-border rounded-2xl w-full max-w-xl p-10 animate-fade-in">
                        <div className="flex justify-between mb-6">
                            <h3 className="text-xl font-bold text-text-primary">Registrar Compra</h3>
                            <button onClick={() => setShowPurchase(false)} className="text-text-muted hover:text-text-primary"><X size={22} /></button>
                        </div>
                        <div className="space-y-6">
                            <div>
                                <label className="text-sm font-medium text-text-secondary block mb-2">Producto</label>
                                <select value={purchaseForm.product_id} onChange={(e) => setPurchaseForm({ ...purchaseForm, product_id: Number(e.target.value) })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm">
                                    <option value={0}>Seleccionar producto...</option>
                                    {products.map(p => <option key={p.id} value={p.id}>{p.name} (Stock: {p.stock})</option>)}
                                </select>
                            </div>
                            <div>
                                <label className="text-sm font-medium text-text-secondary block mb-2">Cantidad</label>
                                <input type="number" min="1" value={purchaseForm.quantity} onChange={(e) => setPurchaseForm({ ...purchaseForm, quantity: parseInt(e.target.value) || 0 })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" />
                            </div>
                            <div>
                                <label className="text-sm font-medium text-text-secondary block mb-2">Precio de Compra (opcional)</label>
                                <input type="number" step="0.01" value={purchaseForm.purchase_price} onChange={(e) => setPurchaseForm({ ...purchaseForm, purchase_price: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" />
                            </div>
                        </div>
                        <div className="flex gap-4 mt-8">
                            <button onClick={() => setShowPurchase(false)} className="flex-1 py-4 bg-bg-primary border border-border rounded-2xl text-text-secondary text-sm font-medium hover:bg-surface-hover transition-colors">Cancelar</button>
                            <button onClick={handlePurchase} disabled={processing} className="flex-1 py-4 bg-success text-white rounded-2xl text-sm font-medium flex items-center justify-center gap-2">{processing && <Loader2 size={18} className="animate-spin" />}Registrar</button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
