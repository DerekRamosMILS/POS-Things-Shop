import { useEffect, useState } from 'react';
import { formatCurrency, formatDateTime } from '../utils';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { CashRegister, Expense } from '../types';
import { DollarSign, Lock, Unlock, Plus, Loader2 } from 'lucide-react';

export default function CashRegisterPage() {
    const { user, setCashRegisterId } = useSessionStore();
    const [register, setRegister] = useState<CashRegister | null>(null);
    const [expenses, setExpenses] = useState<Expense[]>([]);
    const [history, setHistory] = useState<CashRegister[]>([]);
    const [loading, setLoading] = useState(true);
    const [showOpen, setShowOpen] = useState(false);
    const [showClose, setShowClose] = useState(false);
    const [showExpense, setShowExpense] = useState(false);
    const [openAmount, setOpenAmount] = useState('');
    const [closeAmount, setCloseAmount] = useState('');
    const [expenseForm, setExpenseForm] = useState({ category: 'Operativos', description: '', amount: '' });
    const [processing, setProcessing] = useState(false);

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [reg, hist] = await Promise.all([api.getOpenRegister(), api.getRegisterHistory()]);
            setRegister(reg); setHistory(hist);
            if (reg) { setCashRegisterId(reg.id); const exp = await api.getExpenses(reg.id); setExpenses(exp); }
        } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    const handleOpen = async () => {
        if (!user) return;
        setProcessing(true);
        try {
            const reg = await api.openRegister(user.id, { opening_amount: parseFloat(openAmount) || 0 });
            setRegister(reg); setCashRegisterId(reg.id); setShowOpen(false); setOpenAmount(''); loadData();
        } catch (err) { alert(String(err)); } finally { setProcessing(false); }
    };

    const handleClose = async () => {
        setProcessing(true);
        try {
            await api.closeRegister({ closing_amount: parseFloat(closeAmount) || 0 });
            setRegister(null); setCashRegisterId(null); setShowClose(false); setCloseAmount(''); loadData();
        } catch (err) { alert(String(err)); } finally { setProcessing(false); }
    };

    const handleAddExpense = async () => {
        if (!user || !register) return;
        setProcessing(true);
        try {
            await api.createExpense(user.id, register.id, { category: expenseForm.category, description: expenseForm.description, amount: parseFloat(expenseForm.amount) || 0 });
            setShowExpense(false); setExpenseForm({ category: 'Operativos', description: '', amount: '' }); loadData();
        } catch (err) { alert(String(err)); } finally { setProcessing(false); }
    };

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 space-y-10 lg:space-y-12 animate-fade-in">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-6">
                <div><h1 className="text-4xl font-black tracking-tight text-text-primary">Caja Registradora</h1></div>
                {!register ? (
                    <button onClick={() => setShowOpen(true)} className="flex items-center justify-center gap-3 px-8 py-5 bg-success hover:bg-success-hover text-[#0B0B0F] rounded-[20px] font-black text-xl transition-all shadow-[0_4px_20px_rgba(16,185,129,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(16,185,129,0.4)]"><Unlock size={24} /> Abrir Caja</button>
                ) : (
                    <div className="flex flex-col sm:flex-row gap-4">
                        <button onClick={() => setShowExpense(true)} className="flex items-center justify-center gap-3 px-8 py-5 bg-primary hover:bg-primary-hover text-[#0B0B0F] rounded-[20px] font-black text-xl transition-all shadow-[0_4px_20px_rgba(0,224,90,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,224,90,0.4)]"><Plus size={24} /> Registrar Gasto</button>
                        <button onClick={() => setShowClose(true)} className="flex items-center justify-center gap-3 px-8 py-5 bg-danger hover:bg-danger-hover text-[#0B0B0F] rounded-[20px] font-black text-xl transition-all shadow-[0_4px_20px_rgba(244,63,94,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(244,63,94,0.4)]"><Lock size={24} /> Cerrar Caja</button>
                    </div>
                )}
            </div>

            {register && (
                <div className="grid grid-cols-2 lg:grid-cols-4 gap-6">
                    {[
                        { label: 'Ventas Totales', value: formatCurrency(register.total_sales), color: 'text-success', sub: `${register.sale_count} ventas` },
                        { label: 'Efectivo', value: formatCurrency(register.total_cash_sales), color: 'text-primary', sub: 'en efectivo' },
                        { label: 'Tarjeta/Transf', value: formatCurrency(register.total_card_sales + register.total_transfer_sales), color: 'text-accent', sub: 'electrónico' },
                        { label: 'Gastos', value: formatCurrency(register.total_expenses), color: 'text-danger', sub: `${expenses.length} gastos` },
                    ].map((card) => (
                        <div key={card.label} className="glass rounded-[32px] p-8 lg:p-10 border border-border shadow-[0_10px_40px_rgba(0,0,0,0.3)] hover:-translate-y-1 transition-transform">
                            <p className="text-lg font-bold text-text-muted uppercase tracking-wider">{card.label}</p>
                            <p className={`text-5xl font-black ${card.color} mt-4 tracking-tighter drop-shadow-[0_0_15px_rgba(255,255,255,0.1)]`}>{card.value}</p>
                            <p className="text-base font-medium text-text-muted mt-3">{card.sub}</p>
                        </div>
                    ))}
                </div>
            )}

            {register && expenses.length > 0 && (
                <div className="glass rounded-[32px] border border-border p-10 lg:p-12 shadow-[0_10px_50px_rgba(0,0,0,0.4)]">
                    <h2 className="text-3xl font-bold text-text-primary mb-8 flex items-center gap-4"><DollarSign size={32} strokeWidth={2.5} className="text-danger" /> Gastos del Turno</h2>
                    <div className="space-y-4">
                        {expenses.map((e) => (
                            <div key={e.id} className="flex items-center justify-between px-8 py-6 bg-bg-primary rounded-[20px] border border-white/5 hover:bg-white/5 transition-colors">
                                <div><p className="text-xl font-bold text-text-primary">{e.description}</p><p className="text-base text-text-muted mt-2 font-mono tracking-wide">{e.category} • {formatDateTime(e.created_at)}</p></div>
                                <p className="text-2xl font-black tracking-tight text-danger drop-shadow-[0_0_10px_rgba(244,63,94,0.3)]">{formatCurrency(e.amount)}</p>
                            </div>
                        ))}
                    </div>
                </div>
            )}

            {/* History */}
            <div className="glass rounded-[32px] border border-border p-10 lg:p-12 shadow-[0_10px_50px_rgba(0,0,0,0.4)]">
                <h2 className="text-3xl font-bold text-text-primary mb-8">Historial de Cortes</h2>
                <div className="space-y-4">
                    {history.filter(h => h.status === 'closed').slice(0, 10).map((h) => (
                        <div key={h.id} className="flex items-center justify-between px-8 py-6 bg-bg-primary rounded-[20px] border border-white/5 hover:bg-white/5 transition-colors">
                            <div>
                                <p className="text-xl font-bold text-text-primary font-mono tracking-wide">{formatDateTime(h.opened_at)} — {h.closed_at ? formatDateTime(h.closed_at) : ''}</p>
                                <p className="text-base text-text-muted mt-2">{h.user_name} • <span className="font-bold text-text-secondary">{h.sale_count}</span> ventas</p>
                            </div>
                            <div className="text-right">
                                <p className="text-2xl font-black text-success tracking-tight drop-shadow-[0_0_10px_rgba(16,185,129,0.3)]">{formatCurrency(h.total_sales)}</p>
                                {h.difference !== null && <p className={`text-base font-black uppercase tracking-wider mt-2 ${h.difference >= 0 ? 'text-success' : 'text-danger'}`}>Dif: {formatCurrency(h.difference)}</p>}
                            </div>
                        </div>
                    ))}
                    {history.filter(h => h.status === 'closed').length === 0 && <p className="text-text-muted text-xl text-center py-12">Sin cortes registrados</p>}
                </div>
            </div>

            {/* Open Modal */}
            {showOpen && (
                <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 animate-fade-in p-6">
                    <div className="bg-bg-secondary border border-white/10 rounded-[32px] w-full max-w-2xl p-12 lg:p-14 animate-fade-in shadow-[0_20px_80px_rgba(0,0,0,0.9)]">
                        <h3 className="text-4xl font-black text-text-primary mb-10 tracking-tight">Abrir Caja</h3>
                        <label className="text-lg font-bold tracking-wider uppercase text-text-secondary block mb-4">Monto Inicial</label>
                        <input type="number" value={openAmount} onChange={(e) => setOpenAmount(e.target.value)} className="w-full px-8 py-8 bg-bg-primary border-2 border-border rounded-[24px] text-text-primary text-5xl font-mono tracking-wider text-center mb-10 focus:border-success transition-colors focus:shadow-[0_0_20px_rgba(16,185,129,0.15)]" placeholder="0.00" autoFocus />
                        <div className="flex gap-6">
                            <button onClick={() => setShowOpen(false)} className="flex-1 py-6 bg-bg-primary border-2 border-border rounded-[24px] text-text-secondary font-bold text-xl hover:bg-surface-hover hover:border-border-light transition-all">Cancelar</button>
                            <button onClick={handleOpen} disabled={processing} className="flex-1 py-6 bg-success hover:bg-success-hover text-[#0B0B0F] rounded-[24px] font-black text-xl flex items-center justify-center gap-3 transition-all shadow-[0_4px_20px_rgba(16,185,129,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(16,185,129,0.4)] disabled:opacity-50 disabled:shadow-none disabled:translate-y-0">{processing && <Loader2 size={24} className="animate-spin" />}Abrir</button>
                        </div>
                    </div>
                </div>
            )}

            {/* Close Modal */}
            {showClose && (
                <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 animate-fade-in p-6">
                    <div className="bg-bg-secondary border border-white/10 rounded-[32px] w-full max-w-2xl p-12 lg:p-14 animate-fade-in shadow-[0_20px_80px_rgba(0,0,0,0.9)]">
                        <h3 className="text-4xl font-black text-text-primary mb-10 tracking-tight">Cerrar Caja</h3>
                        {register && (
                            <div className="mb-10 p-8 bg-bg-primary rounded-[24px] border border-border flex flex-col items-center justify-center text-center">
                                <p className="text-base font-bold text-text-muted mb-2 tracking-widest uppercase">Monto esperado en efectivo</p>
                                <p className="text-5xl font-black text-primary tracking-tighter drop-shadow-[0_0_15px_rgba(0,224,90,0.3)]">{formatCurrency(register.opening_amount + register.total_cash_sales - register.total_expenses)}</p>
                            </div>
                        )}
                        <label className="text-lg font-bold tracking-wider uppercase text-text-secondary block mb-4">Monto en Caja (Conteo Físico)</label>
                        <input type="number" value={closeAmount} onChange={(e) => setCloseAmount(e.target.value)} className="w-full px-8 py-8 bg-bg-primary border-2 border-border rounded-[24px] text-text-primary text-5xl font-mono tracking-wider text-center mb-10 focus:border-danger transition-colors focus:shadow-[0_0_20px_rgba(244,63,94,0.15)]" placeholder="0.00" autoFocus />
                        <div className="flex gap-6">
                            <button onClick={() => setShowClose(false)} className="flex-1 py-6 bg-bg-primary border-2 border-border rounded-[24px] text-text-secondary font-bold text-xl hover:bg-surface-hover hover:border-border-light transition-all">Cancelar</button>
                            <button onClick={handleClose} disabled={processing} className="flex-1 py-6 bg-danger hover:bg-danger-hover text-[#0B0B0F] rounded-[24px] font-black text-xl flex items-center justify-center gap-3 transition-all shadow-[0_4px_20px_rgba(244,63,94,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(244,63,94,0.4)] disabled:opacity-50 disabled:shadow-none disabled:translate-y-0">{processing && <Loader2 size={24} className="animate-spin" />}Cerrar Caja</button>
                        </div>
                    </div>
                </div>
            )}

            {/* Expense Modal */}
            {showExpense && (
                <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 animate-fade-in p-6">
                    <div className="bg-bg-secondary border border-white/10 rounded-[32px] w-full max-w-2xl p-12 lg:p-14 animate-fade-in shadow-[0_20px_80px_rgba(0,0,0,0.9)]">
                        <h3 className="text-4xl font-black text-text-primary mb-10 tracking-tight">Registrar Gasto</h3>
                        <div className="space-y-8">
                            <div>
                                <label className="text-lg font-bold tracking-wider uppercase text-text-secondary block mb-4">Categoría</label>
                                <select value={expenseForm.category} onChange={(e) => setExpenseForm({ ...expenseForm, category: e.target.value })} className="w-full px-8 py-6 bg-bg-primary border-2 border-border rounded-[24px] text-text-primary text-xl">
                                    {['Operativos', 'Servicios', 'Suministros', 'Transporte', 'Comida', 'Mantenimiento', 'Otros'].map(c => <option key={c} value={c}>{c}</option>)}
                                </select>
                            </div>
                            <div>
                                <label className="text-lg font-bold tracking-wider uppercase text-text-secondary block mb-4">Descripción</label>
                                <input type="text" value={expenseForm.description} onChange={(e) => setExpenseForm({ ...expenseForm, description: e.target.value })} className="w-full px-8 py-6 bg-bg-primary border-2 border-border rounded-[24px] text-text-primary text-xl" />
                            </div>
                            <div>
                                <label className="text-lg font-bold tracking-wider uppercase text-text-secondary block mb-4">Monto</label>
                                <input type="number" value={expenseForm.amount} onChange={(e) => setExpenseForm({ ...expenseForm, amount: e.target.value })} className="w-full px-8 py-6 bg-bg-primary border-2 border-border rounded-[24px] text-text-primary text-2xl font-mono tracking-wider" />
                            </div>
                        </div>
                        <div className="flex gap-6 mt-10">
                            <button onClick={() => setShowExpense(false)} className="flex-1 py-6 bg-bg-primary border-2 border-border rounded-[24px] text-text-secondary font-bold text-xl hover:bg-surface-hover hover:border-border-light transition-all">Cancelar</button>
                            <button onClick={handleAddExpense} disabled={processing} className="flex-1 py-6 bg-primary hover:bg-primary-hover text-[#0B0B0F] rounded-[24px] font-black text-xl flex items-center justify-center gap-3 transition-all shadow-[0_4px_20px_rgba(0,224,90,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,224,90,0.4)] disabled:opacity-50 disabled:shadow-none disabled:translate-y-0">{processing && <Loader2 size={24} className="animate-spin" />}Guardar</button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
