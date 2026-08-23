import { useEffect, useState } from 'react';
import { formatCurrency, formatDateTime, PAYMENT_METHOD_LABELS } from '../utils';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { Layaway } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';

const IcoX = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoEye = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>;
const IcoLoader = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;

const STATUS_LABELS: Record<string, string> = { active: 'Activo', completed: 'Entregado', cancelled: 'Cancelado' };
const STATUS_BADGE: Record<string, string> = { active: 'badge-warning', completed: 'badge-success', cancelled: 'badge-danger' };

export default function LayawaysPage() {
    const { user } = useSessionStore();
    const [layaways, setLayaways] = useState<Layaway[]>([]);
    const [loading, setLoading] = useState(true);
    const [statusFilter, setStatusFilter] = useState<'active' | 'completed' | 'cancelled' | 'all'>('active');
    const [detail, setDetail] = useState<Layaway | null>(null);
    const [payAmount, setPayAmount] = useState('');
    const [payMethod, setPayMethod] = useState('cash');
    const [processing, setProcessing] = useState(false);
    const { showToast } = useToast();
    const { confirm } = useConfirm();

    useEffect(() => { load(); }, [statusFilter]); // eslint-disable-line react-hooks/exhaustive-deps

    const load = async () => {
        setLoading(true);
        try { setLayaways(await api.getLayaways(statusFilter === 'all' ? undefined : statusFilter)); }
        catch (e) { showToast(String(e), 'error'); } finally { setLoading(false); }
    };

    const openDetail = async (id: number) => {
        try { setDetail(await api.getLayawayDetail(id)); setPayAmount(''); setPayMethod('cash'); }
        catch (e) { showToast(String(e), 'error'); }
    };

    const handleAddPayment = async () => {
        if (!detail || !user) return;
        const amount = parseFloat(payAmount) || 0;
        if (amount <= 0) { showToast('Ingresa un abono válido', 'error'); return; }
        setProcessing(true);
        try {
            const updated = await api.addLayawayPayment(detail.id, amount, payMethod);
            setDetail(updated); setPayAmount(''); load();
            showToast('Abono registrado', 'success');
        } catch (e) { showToast(String(e), 'error'); } finally { setProcessing(false); }
    };

    const handleComplete = async () => {
        if (!detail) return;
        setProcessing(true);
        try {
            const updated = await api.completeLayaway(detail.id);
            setDetail(updated); load();
            showToast('Apartado entregado', 'success');
        } catch (e) { showToast(String(e), 'error'); } finally { setProcessing(false); }
    };

    const handleCancel = async () => {
        if (!detail || !user) return;
        const ok = await confirm({ title: 'Cancelar apartado', message: 'Se regresará el stock reservado al inventario. ¿Continuar?', variant: 'danger', confirmLabel: 'Cancelar apartado' });
        if (!ok) return;
        setProcessing(true);
        try {
            await api.cancelLayaway(detail.id);
            setDetail(null); load();
            showToast('Apartado cancelado', 'success');
        } catch (e) { showToast(String(e), 'error'); } finally { setProcessing(false); }
    };

    const activeBalance = layaways.filter(l => l.status === 'active').reduce((s, l) => s + (l.total - l.paid), 0);

    if (loading) return <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}><IcoLoader /></div>;

    const balance = detail ? detail.total - detail.paid : 0;

    return (
        <div className="page-container">
            <div className="page-header">
                <div>
                    <h1 className="page-title">Apartados</h1>
                    <p className="page-subtitle">{layaways.length} apartados · saldo por cobrar {formatCurrency(activeBalance)}</p>
                </div>
                <select value={statusFilter} onChange={e => setStatusFilter(e.target.value as typeof statusFilter)} className="input" style={{ width: 'auto', minWidth: 150 }}>
                    <option value="active">Activos</option>
                    <option value="completed">Entregados</option>
                    <option value="cancelled">Cancelados</option>
                    <option value="all">Todos</option>
                </select>
            </div>

            <div className="card" style={{ flex: 1, padding: 0, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
                <div style={{ overflowX: 'auto', overflowY: 'auto', flex: 1 }}>
                    <table className="table-base">
                        <thead>
                            <tr>
                                <th>Folio</th>
                                <th>Cliente</th>
                                <th>Fecha</th>
                                <th style={{ textAlign: 'right' }}>Total</th>
                                <th style={{ textAlign: 'right' }}>Pagado</th>
                                <th style={{ textAlign: 'right' }}>Saldo</th>
                                <th style={{ textAlign: 'center' }}>Estado</th>
                                <th style={{ textAlign: 'center', width: 70 }}></th>
                            </tr>
                        </thead>
                        <tbody>
                            {layaways.map(l => (
                                <tr key={l.id}>
                                    <td><span style={{ fontSize: 13, fontWeight: 700, color: 'var(--primary)', fontFamily: 'monospace' }}>{l.folio}</span></td>
                                    <td style={{ fontSize: 13, color: 'var(--t2)' }}>{l.customer_name || 'Público general'}</td>
                                    <td style={{ fontFamily: 'monospace', fontSize: 11, color: 'var(--t3)' }}>{formatDateTime(l.created_at)}</td>
                                    <td style={{ textAlign: 'right', fontSize: 13, fontWeight: 700, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(l.total)}</td>
                                    <td style={{ textAlign: 'right', fontSize: 13, color: 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(l.paid)}</td>
                                    <td style={{ textAlign: 'right', fontSize: 13, fontWeight: 700, color: l.total - l.paid > 0 ? 'var(--warning)' : 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(l.total - l.paid)}</td>
                                    <td style={{ textAlign: 'center' }}><span className={`badge ${STATUS_BADGE[l.status]}`}>{STATUS_LABELS[l.status] || l.status}</span></td>
                                    <td style={{ textAlign: 'center' }}>
                                        <button onClick={() => openDetail(l.id)} className="p-2 rounded-lg" style={{ color: 'var(--t3)' }} title="Ver"><IcoEye /></button>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {layaways.length === 0 && <div style={{ padding: '64px 0', textAlign: 'center', color: 'var(--t3)', fontSize: 13 }}>Sin apartados · créalos desde el Punto de Venta</div>}
                </div>
            </div>

            {detail && (
                <div className="modal-overlay" onClick={() => setDetail(null)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 480, maxHeight: '90vh', overflowY: 'auto' }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '20px 24px', borderBottom: '1px solid rgba(255,255,255,0.07)' }}>
                            <div>
                                <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>Apartado {detail.folio}</h3>
                                <p style={{ fontSize: 12, color: 'var(--t3)', marginTop: 2 }}>{detail.customer_name || 'Público general'} · {STATUS_LABELS[detail.status]}</p>
                            </div>
                            <button onClick={() => setDetail(null)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}><IcoX /></button>
                        </div>
                        <div style={{ padding: 24, display: 'flex', flexDirection: 'column', gap: 16 }}>
                            {/* Items */}
                            <div style={{ borderRadius: 14, overflow: 'hidden', border: '1px solid var(--border)' }}>
                                {detail.items?.map((it, i) => (
                                    <div key={it.id} style={{ display: 'flex', justifyContent: 'space-between', padding: '11px 16px', borderTop: i > 0 ? '1px solid var(--border)' : 'none' }}>
                                        <span style={{ fontSize: 13, color: 'var(--t1)' }}>{it.product_name}{it.variant_label ? <span style={{ color: 'var(--primary)', fontWeight: 700 }}> · {it.variant_label}</span> : ''} <span style={{ color: 'var(--t3)' }}>×{it.quantity}</span></span>
                                        <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(it.subtotal)}</span>
                                    </div>
                                ))}
                            </div>

                            {/* Totals */}
                            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 10 }}>
                                <div style={{ padding: '12px', borderRadius: 12, background: 'rgba(255,255,255,0.03)', textAlign: 'center' }}>
                                    <p style={{ fontSize: 10, color: 'var(--t3)', textTransform: 'uppercase' }}>Total</p>
                                    <p style={{ fontSize: 15, fontWeight: 800, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(detail.total)}</p>
                                </div>
                                <div style={{ padding: '12px', borderRadius: 12, background: 'rgba(34,211,160,0.08)', textAlign: 'center' }}>
                                    <p style={{ fontSize: 10, color: 'var(--success)', textTransform: 'uppercase' }}>Pagado</p>
                                    <p style={{ fontSize: 15, fontWeight: 800, color: 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(detail.paid)}</p>
                                </div>
                                <div style={{ padding: '12px', borderRadius: 12, background: 'rgba(245,168,66,0.08)', textAlign: 'center' }}>
                                    <p style={{ fontSize: 10, color: 'var(--warning)', textTransform: 'uppercase' }}>Saldo</p>
                                    <p style={{ fontSize: 15, fontWeight: 800, color: 'var(--warning)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(balance)}</p>
                                </div>
                            </div>

                            {/* Payments */}
                            {detail.payments && detail.payments.length > 0 && (
                                <div>
                                    <p style={{ fontSize: 12, fontWeight: 700, color: 'var(--t2)', marginBottom: 6 }}>Abonos</p>
                                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                                        {detail.payments.map(p => (
                                            <div key={p.id} style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, color: 'var(--t3)', padding: '4px 2px' }}>
                                                <span>{formatDateTime(p.created_at)} · {PAYMENT_METHOD_LABELS[p.payment_method] || p.payment_method}</span>
                                                <span style={{ color: 'var(--success)', fontWeight: 700 }}>{formatCurrency(p.amount)}</span>
                                            </div>
                                        ))}
                                    </div>
                                </div>
                            )}

                            {/* Actions */}
                            {detail.status === 'active' && (
                                <>
                                    {balance > 0 && (
                                        <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end' }}>
                                            <div style={{ flex: 1 }}>
                                                <label className="form-label">Abono</label>
                                                <input type="number" step="0.01" value={payAmount} onChange={e => setPayAmount(e.target.value)} className="input" placeholder="0.00" style={{ fontFamily: 'monospace' }} />
                                            </div>
                                            <select value={payMethod} onChange={e => setPayMethod(e.target.value)} className="input" style={{ width: 'auto' }}>
                                                {Object.entries(PAYMENT_METHOD_LABELS).map(([k, v]) => <option key={k} value={k}>{v}</option>)}
                                            </select>
                                            <button onClick={handleAddPayment} disabled={processing} className="btn btn-success" style={{ justifyContent: 'center' }}>Abonar</button>
                                        </div>
                                    )}
                                    <div style={{ display: 'flex', gap: 10 }}>
                                        <button onClick={handleCancel} disabled={processing} className="btn btn-danger" style={{ flex: 1, justifyContent: 'center' }}>Cancelar apartado</button>
                                        <button onClick={handleComplete} disabled={processing || balance > 0.001} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>
                                            {processing && <IcoLoader />} Entregar
                                        </button>
                                    </div>
                                    {balance > 0.001 && <p style={{ fontSize: 11, color: 'var(--t3)', textAlign: 'center' }}>Liquida el saldo para poder entregar</p>}
                                </>
                            )}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
