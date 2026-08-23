import { useEffect, useState } from 'react';
import { formatCurrency, formatDateTime } from '../utils';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { CashRegister, Expense } from '../types';
import { useToast } from '../contexts/ToastContext';

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoPlus    = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>;
const IcoLock    = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>;
const IcoUnlock  = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/></svg>;
const IcoLoader  = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;

export default function CashRegisterPage() {
    const { user, setCashRegisterId } = useSessionStore();
    const [register, setRegister] = useState<CashRegister | null>(null);
    const [expenses, setExpenses] = useState<Expense[]>([]);
    const [history, setHistory] = useState<CashRegister[]>([]);
    const [loading, setLoading] = useState(true);
    const [showOpen, setShowOpen] = useState(false);
    const [showClose, setShowClose] = useState(false);

    /// Apertura manual del cajón, para dar cambio sin cobrar nada.
    const handleOpenDrawer = async () => {
        try { await api.openCashDrawer(); }
        catch (err) { showToast(String(err), 'error'); }
    };
    const [showExpense, setShowExpense] = useState(false);
    const [openAmount, setOpenAmount] = useState('');
    const [closeAmount, setCloseAmount] = useState('');
    const [expenseForm, setExpenseForm] = useState({ category: 'Operativos', description: '', amount: '' });
    const [processing, setProcessing] = useState(false);
    const { showToast } = useToast();

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [reg, hist] = await Promise.all([api.getOpenRegister(), api.getRegisterHistory()]);
            setRegister(reg); setHistory(hist);
            if (reg) { setCashRegisterId(reg.id); const exp = await api.getExpenses(reg.id); setExpenses(exp); }
        } catch (err) { showToast(String(err), 'error'); } finally { setLoading(false); }
    };

    const handleOpen = async () => {
        if (!user) return;
        setProcessing(true);
        try {
            const reg = await api.openRegister({ opening_amount: parseFloat(openAmount) || 0 });
            setRegister(reg); setCashRegisterId(reg.id); setShowOpen(false); setOpenAmount(''); loadData();
        } catch (err) { showToast(String(err), 'error'); } finally { setProcessing(false); }
    };

    const handleClose = async () => {
        setProcessing(true);
        try {
            await api.closeRegister({ closing_amount: parseFloat(closeAmount) || 0 });
            setRegister(null); setCashRegisterId(null); setShowClose(false); setCloseAmount(''); loadData();
        } catch (err) { showToast(String(err), 'error'); } finally { setProcessing(false); }
    };

    const handleAddExpense = async () => {
        if (!user || !register) return;
        setProcessing(true);
        try {
            await api.createExpense(register.id, {
                category: expenseForm.category,
                description: expenseForm.description,
                amount: parseFloat(expenseForm.amount) || 0,
            });
            setShowExpense(false);
            setExpenseForm({ category: 'Operativos', description: '', amount: '' });
            loadData();
        } catch (err) { showToast(String(err), 'error'); } finally { setProcessing(false); }
    };

    if (loading) return (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
            <IcoLoader />
        </div>
    );

    const closedHistory = history.filter(h => h.status === 'closed');
    // Debe reflejar exactamente el mismo cálculo que expected_cash() en el
    // backend: todo lo que entra o sale del cajón en billetes.
    const cashLines = register ? [
        { label: 'Fondo de apertura', amount: register.opening_amount },
        { label: 'Ventas en efectivo', amount: register.total_cash_sales },
        { label: 'Abonos de apartados', amount: register.total_layaway_cash },
        { label: 'Devoluciones en efectivo', amount: -register.total_refunds_cash },
        { label: 'Gastos', amount: -register.total_expenses },
    ].filter(l => l.amount !== 0) : [];

    const expectedCash = register
        ? Math.round((register.opening_amount + register.total_cash_sales + register.total_layaway_cash
            - register.total_refunds_cash - register.total_expenses) * 100) / 100
        : 0;
    const closeDifference = register ? (parseFloat(closeAmount) || 0) - expectedCash : 0;

    return (
        <div className="page-container">
            {/* Header */}
            <div className="page-header">
                <div>
                    <h1 className="page-title">Caja Registradora</h1>
                    <p className="page-subtitle">{register ? 'Turno activo' : 'Sin turno activo'}</p>
                </div>
                <div style={{ display: 'flex', gap: 10 }}>
                    {!register ? (
                        <button onClick={() => setShowOpen(true)} className="btn btn-success" style={{ gap: 7 }}>
                            <IcoUnlock /> Abrir Caja
                        </button>
                    ) : (
                        <>
                            <button onClick={handleOpenDrawer} className="btn btn-ghost" style={{ gap: 7 }}>
                                <IcoUnlock /> Abrir Cajón
                            </button>
                            <button onClick={() => setShowExpense(true)} className="btn btn-primary" style={{ gap: 7 }}>
                                <IcoPlus /> Registrar Gasto
                            </button>
                            <button onClick={() => setShowClose(true)} className="btn btn-danger" style={{ gap: 7 }}>
                                <IcoLock /> Cerrar Caja
                            </button>
                        </>
                    )}
                </div>
            </div>

            {/* Open register KPIs */}
            {register && (
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 14 }}>
                    {[
                        { label: 'Ventas Totales', value: formatCurrency(register.total_sales), sub: `${register.sale_count} ventas`, color: 'var(--success)' },
                        { label: 'Efectivo', value: formatCurrency(register.total_cash_sales), sub: 'en efectivo', color: 'var(--primary)' },
                        { label: 'Tarjeta / Transf.', value: formatCurrency(register.total_card_sales + register.total_transfer_sales), sub: 'electrónico', color: 'var(--accent)' },
                        { label: 'Gastos', value: formatCurrency(register.total_expenses), sub: `${expenses.length} gastos`, color: 'var(--danger)' },
                    ].map(card => (
                        <div key={card.label} className="card" style={{ padding: '20px 22px' }}>
                            <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>{card.label}</p>
                            <p style={{ fontSize: 20, fontWeight: 900, color: card.color, fontVariantNumeric: 'tabular-nums', letterSpacing: '-0.02em' }}>{card.value}</p>
                            <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 4 }}>{card.sub}</p>
                        </div>
                    ))}
                </div>
            )}

            {/* No register banner */}
            {!register && (
                <div className="card" style={{ padding: '28px 32px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--primary)', marginBottom: 8 }}>Caja cerrada</p>
                    <h2 style={{ fontSize: 20, fontWeight: 900, color: 'var(--t1)', marginBottom: 10, lineHeight: 1.3 }}>Abre un turno para registrar ventas con control de efectivo.</h2>
                    <p style={{ fontSize: 13, color: 'var(--t2)', lineHeight: 1.6 }}>Las ventas pueden seguir operando, pero el corte de caja, gastos del turno y diferencias se vuelven más confiables cuando hay una caja activa.</p>
                </div>
            )}

            {/* Current shift expenses */}
            {register && expenses.length > 0 && (
                <div className="card" style={{ padding: '20px 0' }}>
                    <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', padding: '0 20px 12px', borderBottom: '1px solid var(--border)' }}>Gastos del Turno</p>
                    {expenses.map(e => (
                        <div key={e.id} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 20px', transition: 'background 0.12s' }}
                            onMouseEnter={el => (el.currentTarget as HTMLDivElement).style.background = 'rgba(255,255,255,0.03)'}
                            onMouseLeave={el => (el.currentTarget as HTMLDivElement).style.background = 'transparent'}
                        >
                            <div>
                                <p style={{ fontSize: 13, fontWeight: 500, color: 'var(--t1)' }}>{e.description}</p>
                                <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 2 }}>{e.category} · {formatDateTime(e.created_at)}</p>
                            </div>
                            <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--danger)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(e.amount)}</p>
                        </div>
                    ))}
                </div>
            )}

            {/* History */}
            <div className="card" style={{ padding: '20px 0', flex: 1 }}>
                <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', padding: '0 20px 12px', borderBottom: '1px solid var(--border)' }}>Historial de Cortes</p>
                {closedHistory.length === 0 ? (
                    <p style={{ textAlign: 'center', padding: '40px 0', fontSize: 13, color: 'var(--t3)' }}>Sin cortes registrados</p>
                ) : (
                    closedHistory.slice(0, 10).map(h => (
                        <div key={h.id} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 20px', transition: 'background 0.12s' }}
                            onMouseEnter={el => (el.currentTarget as HTMLDivElement).style.background = 'rgba(255,255,255,0.03)'}
                            onMouseLeave={el => (el.currentTarget as HTMLDivElement).style.background = 'transparent'}
                        >
                            <div>
                                <p style={{ fontSize: 12, fontFamily: 'monospace', color: 'var(--t1)' }}>
                                    {formatDateTime(h.opened_at)}{h.closed_at ? ` → ${formatDateTime(h.closed_at)}` : ''}
                                </p>
                                <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 2 }}>
                                    {h.user_name} · {h.sale_count} ventas
                                </p>
                            </div>
                            <div style={{ textAlign: 'right' }}>
                                <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(h.total_sales)}</p>
                                {h.difference !== null && (
                                    <p style={{ fontSize: 11, fontWeight: 600, color: h.difference >= 0 ? 'var(--success)' : 'var(--danger)', fontVariantNumeric: 'tabular-nums', marginTop: 2 }}>
                                        Dif: {formatCurrency(h.difference)}
                                    </p>
                                )}
                            </div>
                        </div>
                    ))
                )}
            </div>

            {/* ── Open Modal ── */}
            {showOpen && (
                <div className="modal-overlay" onClick={() => setShowOpen(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 360, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)', marginBottom: 20 }}>Abrir Caja</h3>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                            <div>
                                <label className="form-label">Monto Inicial en Caja</label>
                                <input type="number" value={openAmount} onChange={e => setOpenAmount(e.target.value)} className="input" style={{ height: 52, fontSize: 22, fontWeight: 700, textAlign: 'center', fontFamily: 'monospace' }} placeholder="0.00" autoFocus />
                            </div>
                            <div style={{ display: 'flex', gap: 10 }}>
                                <button onClick={() => setShowOpen(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handleOpen} disabled={processing} className="btn btn-success" style={{ flex: 1, justifyContent: 'center' }}>
                                    {processing && <IcoLoader />} Abrir
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {/* ── Close Modal ── */}
            {showClose && (
                <div className="modal-overlay" onClick={() => setShowClose(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 360, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)', marginBottom: 20 }}>Cerrar Caja</h3>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            {register && (
                                <div style={{ padding: '14px 18px', borderRadius: 14, background: 'rgba(139,120,245,0.08)', border: '1px solid rgba(139,120,245,0.2)' }}>
                                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 10 }}>
                                        {cashLines.map(l => (
                                            <div key={l.label} style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12, color: 'var(--t3)' }}>
                                                <span>{l.label}</span>
                                                <span style={{ fontVariantNumeric: 'tabular-nums', color: l.amount < 0 ? 'var(--danger)' : 'var(--t2)' }}>
                                                    {l.amount < 0 ? '-' : ''}{formatCurrency(Math.abs(l.amount))}
                                                </span>
                                            </div>
                                        ))}
                                    </div>
                                    <div style={{ borderTop: '1px solid rgba(139,120,245,0.2)', paddingTop: 10, textAlign: 'center' }}>
                                        <p style={{ fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--primary)', marginBottom: 6 }}>Efectivo Esperado</p>
                                        <p style={{ fontSize: 26, fontWeight: 900, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums', letterSpacing: '-0.02em' }}>{formatCurrency(expectedCash)}</p>
                                    </div>
                                </div>
                            )}
                            <div>
                                <label className="form-label">Conteo Físico en Caja</label>
                                <input type="number" value={closeAmount} onChange={e => setCloseAmount(e.target.value)} className="input" style={{ height: 52, fontSize: 22, fontWeight: 700, textAlign: 'center', fontFamily: 'monospace' }} placeholder="0.00" autoFocus />
                            </div>
                            {register && closeAmount && (
                                <div style={{
                                    padding: '14px 18px', borderRadius: 14, textAlign: 'center',
                                    background: closeDifference === 0 ? 'rgba(34,211,160,0.08)' : 'rgba(245,168,66,0.08)',
                                    border: `1px solid ${closeDifference === 0 ? 'rgba(34,211,160,0.25)' : 'rgba(245,168,66,0.25)'}`,
                                }}>
                                    <p style={{ fontSize: 11, textTransform: 'uppercase', letterSpacing: '0.06em', color: closeDifference === 0 ? 'var(--success)' : 'var(--warning)', marginBottom: 6 }}>Diferencia estimada</p>
                                    <p style={{ fontSize: 26, fontWeight: 900, color: closeDifference === 0 ? 'var(--success)' : 'var(--warning)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(closeDifference)}</p>
                                </div>
                            )}
                            <div style={{ display: 'flex', gap: 10 }}>
                                <button onClick={() => setShowClose(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handleClose} disabled={processing} className="btn btn-danger" style={{ flex: 1, justifyContent: 'center' }}>
                                    {processing && <IcoLoader />} Cerrar Caja
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {/* ── Expense Modal ── */}
            {showExpense && (
                <div className="modal-overlay" onClick={() => setShowExpense(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 360, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)', marginBottom: 20 }}>Registrar Gasto</h3>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div>
                                <label className="form-label">Categoría</label>
                                <select value={expenseForm.category} onChange={e => setExpenseForm({ ...expenseForm, category: e.target.value })} className="input">
                                    {['Operativos','Servicios','Suministros','Transporte','Comida','Mantenimiento','Otros'].map(c => <option key={c} value={c}>{c}</option>)}
                                </select>
                            </div>
                            <div>
                                <label className="form-label">Descripción</label>
                                <input type="text" value={expenseForm.description} onChange={e => setExpenseForm({ ...expenseForm, description: e.target.value })} className="input" placeholder="Ej. Pago de luz" />
                            </div>
                            <div>
                                <label className="form-label">Monto</label>
                                <input type="number" step="0.01" value={expenseForm.amount} onChange={e => setExpenseForm({ ...expenseForm, amount: e.target.value })} className="input" style={{ fontFamily: 'monospace' }} placeholder="0.00" />
                            </div>
                            <div style={{ display: 'flex', gap: 10 }}>
                                <button onClick={() => setShowExpense(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handleAddExpense} disabled={processing} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>
                                    {processing && <IcoLoader />} Guardar
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
