import { useEffect, useState } from 'react';
import { formatCurrency, formatDateTime, STATUS_LABELS, PAYMENT_METHOD_LABELS } from '../utils';
import * as api from '../api';
import type { Sale } from '../types';
import { useSessionStore } from '../stores/useSessionStore';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoSearch  = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>;
const IcoX       = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoEye     = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>;
const IcoBan     = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="12" cy="12" r="10"/><line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/></svg>;
const IcoFilter  = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/></svg>;
const IcoReceipt = () => <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" className="opacity-40"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/><polyline points="10 9 9 9 8 9"/></svg>;

function IcoBtn({ onClick, title, children, hoverColor = 'var(--primary)', hoverBg = 'rgba(139,120,245,0.10)' }: {
    onClick: () => void; title: string; children: React.ReactNode;
    hoverColor?: string; hoverBg?: string;
}) {
    return (
        <button onClick={onClick} title={title} className="p-2 rounded-lg transition-colors" style={{ color: 'var(--t3)' }}
            onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.color = hoverColor; (e.currentTarget as HTMLButtonElement).style.background = hoverBg; }}
            onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
        >{children}</button>
    );
}

export default function SalesPage() {
    const [sales, setSales] = useState<Sale[]>([]);
    const [loading, setLoading] = useState(true);
    const [detail, setDetail] = useState<Sale | null>(null);
    const [search, setSearch] = useState('');
    const [statusFilter, setStatusFilter] = useState('');
    const [paymentFilter, setPaymentFilter] = useState('');
    const [dateFrom, setDateFrom] = useState('');
    const [dateTo, setDateTo] = useState('');
    const { user } = useSessionStore();
    const { showToast } = useToast();
    const { confirm } = useConfirm();

    useEffect(() => { loadSales(); }, []);

    const loadSales = async () => {
        try { const s = await api.getSales(); setSales(s); }
        catch (err) { console.error(err); }
        finally { setLoading(false); }
    };

    const handleViewDetail = async (saleId: number) => {
        try { const d = await api.getSaleDetail(saleId); setDetail(d); }
        catch (err) { showToast(String(err), 'error'); }
    };

    const handleCancel = async (saleId: number) => {
        if (!user) return;
        const ok = await confirm({ title: 'Cancelar venta', message: '¿Cancelar esta venta? Se restaurará el inventario.', variant: 'danger', confirmLabel: 'Cancelar venta' });
        if (!ok) return;
        try { await api.cancelSale(saleId, user.id); loadSales(); setDetail(null); }
        catch (err) { showToast(String(err), 'error'); }
    };

    const filtered = sales.filter(s => {
        const q = search.toLowerCase().trim();
        if (q && ![s.folio, s.user_name || '', s.payment_method, s.status].some(v => v.toLowerCase().includes(q))) return false;
        if (statusFilter && s.status !== statusFilter) return false;
        if (paymentFilter && s.payment_method !== paymentFilter) return false;
        if (dateFrom && s.created_at.slice(0, 10) < dateFrom) return false;
        if (dateTo && s.created_at.slice(0, 10) > dateTo) return false;
        return true;
    });

    const totalSales = filtered.filter(s => s.status === 'completed').reduce((sum, s) => sum + s.total, 0);
    const cancelledCount = filtered.filter(s => s.status !== 'completed').length;
    const hasFilters = !!(search || statusFilter || paymentFilter || dateFrom || dateTo);

    if (loading) return (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--primary)" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
        </div>
    );

    return (
        <div className="page-container">
            {/* Header */}
            <div className="page-header">
                <div>
                    <h1 className="page-title">Ventas</h1>
                    <p className="page-subtitle">{filtered.length} de {sales.length} ventas · {formatCurrency(totalSales)} completadas</p>
                </div>
            </div>

            {/* KPIs */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 14 }}>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Total filtrado</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(totalSales)}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Tickets</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{filtered.length}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Canceladas</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: cancelledCount > 0 ? 'var(--danger)' : 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{cancelledCount}</p>
                </div>
            </div>

            {/* Filters */}
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
                <div style={{ flex: 1, minWidth: 240, position: 'relative' }}>
                    <span style={{ position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--t3)', pointerEvents: 'none' }}><IcoSearch /></span>
                    <input type="text" value={search} onChange={e => setSearch(e.target.value)} placeholder="Buscar folio, cajero, método de pago…" className="input" style={{ paddingLeft: 36 }} />
                </div>
                <input type="date" value={dateFrom} onChange={e => setDateFrom(e.target.value)} className="input" style={{ width: 'auto' }} />
                <input type="date" value={dateTo} onChange={e => setDateTo(e.target.value)} className="input" style={{ width: 'auto' }} />
                <select value={paymentFilter} onChange={e => setPaymentFilter(e.target.value)} className="input" style={{ width: 'auto', minWidth: 150 }}>
                    <option value="">Todos los pagos</option>
                    {Object.entries(PAYMENT_METHOD_LABELS).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                </select>
                <select value={statusFilter} onChange={e => setStatusFilter(e.target.value)} className="input" style={{ width: 'auto', minWidth: 150 }}>
                    <option value="">Todos los estados</option>
                    {Object.entries(STATUS_LABELS).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                </select>
                {hasFilters && (
                    <button onClick={() => { setSearch(''); setStatusFilter(''); setPaymentFilter(''); setDateFrom(''); setDateTo(''); }} className="btn btn-ghost btn-sm" style={{ gap: 6 }}>
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
                                <th>Folio</th>
                                <th>Fecha</th>
                                <th>Cajero</th>
                                <th>Método</th>
                                <th style={{ textAlign: 'right' }}>Total</th>
                                <th style={{ textAlign: 'center' }}>Estado</th>
                                <th style={{ textAlign: 'center', width: 90 }}>Acciones</th>
                            </tr>
                        </thead>
                        <tbody>
                            {filtered.map(s => (
                                <tr key={s.id}>
                                    <td>
                                        <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--primary)', fontFamily: 'monospace' }}>{s.folio}</span>
                                    </td>
                                    <td style={{ fontFamily: 'monospace', fontSize: 12, color: 'var(--t2)' }}>{formatDateTime(s.created_at)}</td>
                                    <td style={{ color: 'var(--t2)', fontSize: 13 }}>{s.user_name}</td>
                                    <td style={{ color: 'var(--t2)', fontSize: 13 }}>{PAYMENT_METHOD_LABELS[s.payment_method] || s.payment_method}</td>
                                    <td style={{ textAlign: 'right' }}>
                                        <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(s.total)}</span>
                                    </td>
                                    <td style={{ textAlign: 'center' }}>
                                        <span className={`badge ${s.status === 'completed' ? 'badge-success' : 'badge-danger'}`}>
                                            {STATUS_LABELS[s.status] || s.status}
                                        </span>
                                    </td>
                                    <td>
                                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 2 }}>
                                            <IcoBtn onClick={() => handleViewDetail(s.id)} title="Ver detalle"><IcoEye /></IcoBtn>
                                            {s.status === 'completed' && (
                                                <IcoBtn onClick={() => handleCancel(s.id)} title="Cancelar" hoverColor="var(--danger)" hoverBg="rgba(244,82,112,0.10)"><IcoBan /></IcoBtn>
                                            )}
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {filtered.length === 0 && (
                        <div style={{ padding: '64px 0', textAlign: 'center', color: 'var(--t3)' }}>
                            <IcoReceipt />
                            <p style={{ marginTop: 12, fontSize: 13 }}>No se encontraron ventas</p>
                        </div>
                    )}
                </div>
            </div>

            {/* ── Detail Modal ── */}
            {detail && (
                <div className="modal-overlay" onClick={() => setDetail(null)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 480, maxHeight: '90vh', overflowY: 'auto' }} onClick={e => e.stopPropagation()}>
                        {/* Modal header */}
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '20px 24px', borderBottom: '1px solid rgba(255,255,255,0.07)' }}>
                            <div>
                                <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>Detalle de Venta</h3>
                                <p style={{ fontSize: 12, fontFamily: 'monospace', color: 'var(--primary)', marginTop: 2 }}>{detail.folio}</p>
                            </div>
                            <button onClick={() => setDetail(null)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)', transition: 'all 0.15s' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t1)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; }}
                            ><IcoX /></button>
                        </div>

                        <div style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 16 }}>
                            {/* Meta */}
                            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10 }}>
                                {[
                                    { label: 'Fecha', value: formatDateTime(detail.created_at), mono: true },
                                    { label: 'Método de Pago', value: PAYMENT_METHOD_LABELS[detail.payment_method] || detail.payment_method },
                                    { label: 'Cajero', value: detail.user_name || '—' },
                                    { label: 'Estado', value: STATUS_LABELS[detail.status] || detail.status },
                                ].map(m => (
                                    <div key={m.label} style={{ padding: '12px 14px', borderRadius: 12, background: 'rgba(255,255,255,0.03)', border: '1px solid var(--border)' }}>
                                        <p style={{ fontSize: 10, color: 'var(--t3)', textTransform: 'uppercase', letterSpacing: '0.06em', marginBottom: 4 }}>{m.label}</p>
                                        <p style={{ fontSize: 13, fontWeight: 600, color: 'var(--t1)', fontFamily: m.mono ? 'monospace' : 'inherit' }}>{m.value}</p>
                                    </div>
                                ))}
                            </div>

                            {/* Items */}
                            <div style={{ borderRadius: 14, overflow: 'hidden', border: '1px solid var(--border)' }}>
                                {detail.items?.map((item, i) => (
                                    <div key={item.id} style={{
                                        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                                        padding: '11px 16px',
                                        borderTop: i > 0 ? '1px solid var(--border)' : 'none',
                                    }}>
                                        <div>
                                            <span style={{ fontSize: 13, color: 'var(--t1)', fontWeight: 500 }}>{item.product_name}</span>
                                            <span style={{ fontSize: 11, color: 'var(--t3)', marginLeft: 8 }}>×{item.quantity}</span>
                                            {item.discount > 0 && <span className="badge badge-accent" style={{ marginLeft: 6 }}>-{item.discount}%</span>}
                                        </div>
                                        <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(item.subtotal)}</span>
                                    </div>
                                ))}
                            </div>

                            {/* Totals */}
                            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                                {detail.discount_total > 0 && (
                                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 13, color: 'var(--t3)' }}>
                                        <span>Descuento</span>
                                        <span style={{ color: 'var(--danger)' }}>-{formatCurrency(detail.discount_total)}</span>
                                    </div>
                                )}
                                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '14px 18px', borderRadius: 14, background: 'rgba(139,120,245,0.08)', border: '1px solid rgba(139,120,245,0.2)', marginTop: 4 }}>
                                    <span style={{ fontSize: 14, fontWeight: 700, color: 'var(--t1)' }}>Total</span>
                                    <span style={{ fontSize: 26, fontWeight: 900, color: 'var(--primary)', fontVariantNumeric: 'tabular-nums', letterSpacing: '-0.02em' }}>{formatCurrency(detail.total)}</span>
                                </div>
                                {detail.amount_paid > 0 && (
                                    <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, color: 'var(--t3)', padding: '0 4px' }}>
                                        <span>Pagado: {formatCurrency(detail.amount_paid)}</span>
                                        <span>Cambio: {formatCurrency(detail.change_amount)}</span>
                                    </div>
                                )}
                            </div>

                            {detail.status === 'completed' && (
                                <button onClick={() => handleCancel(detail.id)} className="btn btn-danger btn-full" style={{ justifyContent: 'center' }}>
                                    <IcoBan /> Cancelar Venta
                                </button>
                            )}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
