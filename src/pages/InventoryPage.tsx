import { useEffect, useState, useMemo } from 'react';
import { formatDateTime, MOVEMENT_TYPE_LABELS } from '../utils';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { InventoryMovement, Product } from '../types';
import { useToast } from '../contexts/ToastContext';

// ─── Gradient avatar helper ──────────────────────────────────────────────────
const GRAD_PAIRS: [string, string][] = [
    ['#8B78F5','#F0C547'],['#F45270','#F0C547'],['#22D3A0','#8B78F5'],
    ['#F5A842','#F45270'],['#8B78F5','#22D3A0'],['#F0C547','#22D3A0'],
];
function getGrad(name: string): [string, string] {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0;
    return GRAD_PAIRS[h % GRAD_PAIRS.length];
}
function GAvatar({ name, size = 32 }: { name: string; size?: number }) {
    const [a, b] = getGrad(name);
    const init = name.trim().split(/\s+/).slice(0, 2).map(w => w[0]).join('').toUpperCase();
    return (
        <div style={{
            width: size, height: size, borderRadius: size * 0.28, flexShrink: 0,
            background: `linear-gradient(135deg,${a},${b})`,
            display: 'grid', placeItems: 'center',
            color: '#fff', fontWeight: 800, fontSize: size * 0.36,
        }}>{init}</div>
    );
}

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoPlus     = () => <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>;
const IcoX        = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoSearch   = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>;
const IcoLoader   = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;
const IcoAlert    = () => <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>;
const IcoCart     = () => <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="9" cy="21" r="1"/><circle cx="20" cy="21" r="1"/><path d="M1 1h4l2.68 13.39a2 2 0 0 0 2 1.61h9.72a2 2 0 0 0 2-1.61L23 6H6"/></svg>;
const IcoArrow    = ({ up }: { up: boolean }) => <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" style={{ transform: up ? 'rotate(180deg)' : 'none' }}><line x1="12" y1="5" x2="12" y2="19"/><polyline points="19 12 12 19 5 12"/></svg>;
const IcoBox      = () => <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg>;

export default function InventoryPage() {
    const { user } = useSessionStore();
    const [movements, setMovements] = useState<InventoryMovement[]>([]);
    const [lowStock, setLowStock] = useState<Product[]>([]);
    const [loading, setLoading] = useState(true);
    const [showAdjust, setShowAdjust] = useState(false);
    const [showPurchase, setShowPurchase] = useState(false);
    const [products, setProducts] = useState<Product[]>([]);
    const [search, setSearch] = useState('');
    const [typeFilter, setTypeFilter] = useState('');
    const [adjustForm, setAdjustForm] = useState({ product_id: 0, quantity: 0, reason: '' });
    const [purchaseForm, setPurchaseForm] = useState({ product_id: 0, quantity: 0, purchase_price: '' });
    const [processing, setProcessing] = useState(false);
    const { showToast } = useToast();

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [m, ls, prods] = await Promise.all([
                api.getInventoryMovements(),
                api.getLowStockProducts(),
                api.getProducts({ is_active: true }),
            ]);
            setMovements(m); setLowStock(ls); setProducts(prods);
        } catch (err) { showToast(String(err), 'error'); } finally { setLoading(false); }
    };

    const handleAdjust = async () => {
        if (!user || !adjustForm.product_id || adjustForm.quantity === 0 || !adjustForm.reason) return;
        setProcessing(true);
        try {
            await api.adjustStock(adjustForm);
            setShowAdjust(false); loadData();
            showToast('Ajuste aplicado correctamente', 'success');
        } catch (err) { showToast(String(err), 'error'); } finally { setProcessing(false); }
    };

    const handlePurchase = async () => {
        if (!user || !purchaseForm.product_id || purchaseForm.quantity <= 0) return;
        setProcessing(true);
        try {
            await api.registerPurchase({
                product_id: purchaseForm.product_id,
                quantity: purchaseForm.quantity,
                purchase_price: purchaseForm.purchase_price ? parseFloat(purchaseForm.purchase_price) : undefined,
            });
            setShowPurchase(false); loadData();
            showToast('Compra registrada correctamente', 'success');
        } catch (err) { showToast(String(err), 'error'); } finally { setProcessing(false); }
    };

    const filteredMovements = useMemo(() => {
        return movements.filter(m => {
            const q = search.toLowerCase().trim();
            if (q && ![m.product_name || '', m.reason || '', m.user_name || '', m.movement_type].some(v => v.toLowerCase().includes(q))) return false;
            if (typeFilter && m.movement_type !== typeFilter) return false;
            return true;
        });
    }, [movements, search, typeFilter]);

    const movementTypes = Array.from(new Set(movements.map(m => m.movement_type)));
    const incomingUnits = filteredMovements.filter(m => m.quantity > 0).reduce((sum, m) => sum + m.quantity, 0);
    const outgoingUnits = Math.abs(filteredMovements.filter(m => m.quantity < 0).reduce((sum, m) => sum + m.quantity, 0));

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
                    <h1 className="page-title">Inventario</h1>
                    <p className="page-subtitle">Entradas, salidas y control de stock</p>
                </div>
                <div style={{ display: 'flex', gap: 10 }}>
                    <button onClick={() => { setPurchaseForm({ product_id: 0, quantity: 0, purchase_price: '' }); setShowPurchase(true); }} className="btn btn-ghost">
                        <IcoCart /> Recibir Compra
                    </button>
                    <button onClick={() => { setAdjustForm({ product_id: 0, quantity: 0, reason: '' }); setShowAdjust(true); }} className="btn btn-primary">
                        <IcoPlus /> Ajuste Manual
                    </button>
                </div>
            </div>

            {/* Layout: movements table + stock critico sidebar */}
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 280px', gap: 20, flex: 1, minHeight: 0 }}>

                {/* Left: KPIs + Table */}
                <div style={{ display: 'flex', flexDirection: 'column', gap: 16, minHeight: 0 }}>

                    {/* KPI row */}
                    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 14 }}>
                        {[
                            { label: 'Movimientos', value: filteredMovements.length, color: 'var(--primary)', suffix: '' },
                            { label: 'Entradas', value: incomingUnits, color: 'var(--success)', suffix: '+' },
                            { label: 'Salidas', value: outgoingUnits, color: 'var(--danger)', suffix: '-' },
                        ].map(k => (
                            <div key={k.label} className="card" style={{ padding: '18px 22px' }}>
                                <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 6 }}>{k.label}</p>
                                <p style={{ fontSize: 28, fontWeight: 900, color: k.color, fontVariantNumeric: 'tabular-nums' }}>{k.suffix}{k.value}</p>
                            </div>
                        ))}
                    </div>

                    {/* Movements table */}
                    <div className="card" style={{ padding: 0, flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', minHeight: 0 }}>
                        {/* Table toolbar */}
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '14px 20px', borderBottom: '1px solid var(--border)' }}>
                            <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)' }}>Historial de Movimientos</span>
                            <div style={{ display: 'flex', gap: 8 }}>
                                <div style={{ position: 'relative' }}>
                                    <span style={{ position: 'absolute', left: 10, top: '50%', transform: 'translateY(-50%)', color: 'var(--t3)', pointerEvents: 'none' }}><IcoSearch /></span>
                                    <input value={search} onChange={e => setSearch(e.target.value)} className="input" style={{ paddingLeft: 30, height: 34, fontSize: 12, width: 160, borderRadius: 9 }} placeholder="Buscar…" />
                                </div>
                                <select value={typeFilter} onChange={e => setTypeFilter(e.target.value)} className="input" style={{ height: 34, fontSize: 12, width: 140, borderRadius: 9 }}>
                                    <option value="">Cualquier tipo</option>
                                    {movementTypes.map(t => <option key={t} value={t}>{MOVEMENT_TYPE_LABELS[t] || t}</option>)}
                                </select>
                            </div>
                        </div>
                        <div style={{ overflowY: 'auto', flex: 1 }}>
                            <table className="table-base">
                                <thead>
                                    <tr>
                                        <th>Producto</th>
                                        <th>Tipo / Motivo</th>
                                        <th style={{ textAlign: 'center' }}>Variación</th>
                                        <th style={{ textAlign: 'center' }}>Stock Final</th>
                                        <th style={{ textAlign: 'right' }}>Fecha</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {filteredMovements.slice(0, 100).map(m => {
                                        const isIn = m.quantity > 0;
                                        return (
                                            <tr key={m.id}>
                                                <td>
                                                    <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                                                        <GAvatar name={m.product_name || 'X'} size={28} />
                                                        <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--t1)' }}>{m.product_name}</span>
                                                    </div>
                                                </td>
                                                <td>
                                                    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                                                        <span className={`badge ${isIn ? 'badge-success' : 'badge-danger'}`}>
                                                            {MOVEMENT_TYPE_LABELS[m.movement_type] || m.movement_type}
                                                        </span>
                                                        <span style={{ fontSize: 11, color: 'var(--t3)', maxWidth: 120, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                                                            {m.reason || (m.movement_type === 'sale' ? 'Ticket POS' : '—')}
                                                        </span>
                                                    </div>
                                                </td>
                                                <td>
                                                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 6 }}>
                                                        <span style={{ fontSize: 11, color: 'var(--t3)' }}>{m.previous_stock}</span>
                                                        <IcoArrow up={isIn} />
                                                        <span style={{ fontSize: 13, fontWeight: 800, color: isIn ? 'var(--success)' : 'var(--danger)', fontVariantNumeric: 'tabular-nums' }}>
                                                            {isIn ? '+' : ''}{m.quantity}
                                                        </span>
                                                    </div>
                                                </td>
                                                <td style={{ textAlign: 'center' }}>
                                                    <span style={{ fontSize: 13, fontWeight: 800, color: 'var(--t1)', background: 'rgba(255,255,255,0.06)', borderRadius: 8, padding: '2px 10px', fontVariantNumeric: 'tabular-nums' }}>
                                                        {m.new_stock}
                                                    </span>
                                                </td>
                                                <td style={{ textAlign: 'right', fontSize: 11, color: 'var(--t3)', fontFamily: 'monospace' }}>
                                                    {formatDateTime(m.created_at)}
                                                </td>
                                            </tr>
                                        );
                                    })}
                                </tbody>
                            </table>
                            {filteredMovements.length === 0 && (
                                <div style={{ padding: '60px 0', textAlign: 'center', color: 'var(--t3)' }}>
                                    <IcoBox />
                                    <p style={{ marginTop: 10, fontSize: 13 }}>No hay movimientos</p>
                                </div>
                            )}
                        </div>
                    </div>
                </div>

                {/* Right: Stock crítico */}
                <div className="card" style={{ padding: 0, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                    <div style={{ padding: '16px 20px', borderBottom: '1px solid var(--border)', display: 'flex', alignItems: 'center', gap: 10 }}>
                        <div style={{ width: 36, height: 36, borderRadius: 11, background: 'rgba(245,168,66,0.12)', border: '1px solid rgba(245,168,66,0.25)', display: 'grid', placeItems: 'center', color: 'var(--warning)', flexShrink: 0 }}>
                            <IcoAlert />
                        </div>
                        <div>
                            <p style={{ fontSize: 13, fontWeight: 800, color: 'var(--t1)' }}>Stock Crítico</p>
                            <p style={{ fontSize: 11, color: lowStock.length > 0 ? 'var(--warning)' : 'var(--success)' }}>{lowStock.length} artículos</p>
                        </div>
                    </div>
                    <div style={{ overflowY: 'auto', flex: 1, padding: '12px 16px', display: 'flex', flexDirection: 'column', gap: 8 }}>
                        {lowStock.length === 0 ? (
                            <div style={{ padding: '40px 0', textAlign: 'center', color: 'var(--t3)' }}>
                                <p style={{ fontSize: 22, marginBottom: 8 }}>✓</p>
                                <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--success)' }}>¡Todo en orden!</p>
                                <p style={{ fontSize: 11, marginTop: 4 }}>Ningún producto bajo el mínimo.</p>
                            </div>
                        ) : (
                            lowStock.slice(0, 12).map(p => {
                                const outOfStock = p.stock <= 0;
                                return (
                                    <div key={p.id} style={{
                                        padding: '10px 12px', borderRadius: 12,
                                        background: 'rgba(255,255,255,0.03)',
                                        border: `1px solid ${outOfStock ? 'rgba(244,82,112,0.25)' : 'rgba(245,168,66,0.2)'}`,
                                        display: 'flex', alignItems: 'center', gap: 10,
                                    }}>
                                        <GAvatar name={p.name} size={30} />
                                        <div style={{ flex: 1, minWidth: 0 }}>
                                            <p style={{ fontSize: 12, fontWeight: 600, color: 'var(--t1)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{p.name}</p>
                                            <p style={{ fontSize: 10, color: 'var(--t3)', marginTop: 1 }}>Mín: {p.min_stock} uds</p>
                                        </div>
                                        <span style={{ fontSize: 18, fontWeight: 900, color: outOfStock ? 'var(--danger)' : 'var(--warning)', fontVariantNumeric: 'tabular-nums', flexShrink: 0 }}>
                                            {p.stock}
                                        </span>
                                    </div>
                                );
                            })
                        )}
                    </div>
                    {/* Hint */}
                    <div style={{ padding: '12px 16px', borderTop: '1px solid var(--border)', background: 'rgba(139,120,245,0.05)' }}>
                        <p style={{ fontSize: 11, color: 'var(--t3)', lineHeight: 1.5 }}>
                            Mantén actualizados los mínimos de stock en el catálogo para alertas precisas.
                        </p>
                    </div>
                </div>
            </div>

            {/* ── Adjust Modal ── */}
            {showAdjust && (
                <div className="modal-overlay" onClick={() => setShowAdjust(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 380, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>Ajuste Manual</h3>
                            <button onClick={() => setShowAdjust(false)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
                            ><IcoX /></button>
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div>
                                <label className="form-label">Producto</label>
                                <select value={adjustForm.product_id} onChange={e => setAdjustForm({ ...adjustForm, product_id: Number(e.target.value) })} className="input">
                                    <option value={0}>Seleccionar producto…</option>
                                    {products.map(p => <option key={p.id} value={p.id}>{p.name} (Disp: {p.stock})</option>)}
                                </select>
                            </div>
                            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                                <div>
                                    <label className="form-label">Variación</label>
                                    <input type="number" placeholder="+5 o -2" value={adjustForm.quantity || ''} onChange={e => setAdjustForm({ ...adjustForm, quantity: parseInt(e.target.value) || 0 })} className="input" style={{ fontFamily: 'monospace' }} />
                                </div>
                                <div>
                                    <label className="form-label">Motivo</label>
                                    <select value={adjustForm.reason} onChange={e => setAdjustForm({ ...adjustForm, reason: e.target.value })} className="input" style={{ fontSize: 12 }}>
                                        <option value="">Selecciona…</option>
                                        <option value="Inventario inicial">Inventario Inicial</option>
                                        <option value="Merma/Daño">Merma o Daño</option>
                                        <option value="Robo">Robo/Extravío</option>
                                        <option value="Muestra">Muestra/Donación</option>
                                        <option value="Ajuste contable">Ajuste Contable</option>
                                    </select>
                                </div>
                            </div>
                            <button onClick={handleAdjust} disabled={processing} className="btn btn-primary btn-full" style={{ justifyContent: 'center', marginTop: 4 }}>
                                {processing && <IcoLoader />} Confirmar Ajuste
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {/* ── Purchase Modal ── */}
            {showPurchase && (
                <div className="modal-overlay" onClick={() => setShowPurchase(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 380, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>Recibir Compra</h3>
                            <button onClick={() => setShowPurchase(false)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
                            ><IcoX /></button>
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div>
                                <label className="form-label">Producto a abastecer</label>
                                <select value={purchaseForm.product_id} onChange={e => setPurchaseForm({ ...purchaseForm, product_id: Number(e.target.value) })} className="input">
                                    <option value={0}>Buscar producto…</option>
                                    {products.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                                </select>
                            </div>
                            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                                <div>
                                    <label className="form-label">Cantidad</label>
                                    <input type="number" min="1" placeholder="0" value={purchaseForm.quantity || ''} onChange={e => setPurchaseForm({ ...purchaseForm, quantity: parseInt(e.target.value) || 0 })} className="input" style={{ fontFamily: 'monospace' }} />
                                </div>
                                <div>
                                    <label className="form-label">Costo Unitario</label>
                                    <input type="number" step="0.01" placeholder="Opcional" value={purchaseForm.purchase_price} onChange={e => setPurchaseForm({ ...purchaseForm, purchase_price: e.target.value })} className="input" style={{ fontFamily: 'monospace' }} />
                                </div>
                            </div>
                            <button onClick={handlePurchase} disabled={processing} className="btn btn-success btn-full" style={{ justifyContent: 'center', marginTop: 4 }}>
                                {processing && <IcoLoader />} Registrar Entrada
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
