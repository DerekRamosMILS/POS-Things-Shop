import { useEffect, useState } from 'react';
import { formatCurrency, formatDateTime, EXPENSE_CATEGORIES } from '../utils';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { Expense } from '../types';
import { DollarSign, Plus, Edit2, Trash2, Loader2 } from 'lucide-react';

export default function ExpensesPage() {
    const { user, cashRegisterId } = useSessionStore();
    const isAdmin = user?.role === 'admin';
    const [expenses, setExpenses] = useState<Expense[]>([]);
    const [loading, setLoading] = useState(true);
    const [showForm, setShowForm] = useState(false);
    const [editing, setEditing] = useState<Expense | null>(null);
    const [form, setForm] = useState({ category: 'Operativos', description: '', amount: '' });
    const [processing, setProcessing] = useState(false);

    useEffect(() => { load(); }, []);
    const load = async () => { try { setExpenses(await api.getExpenses()); } catch (e) { console.error(e); } finally { setLoading(false); } };

    const openCreate = () => { setEditing(null); setForm({ category: 'Operativos', description: '', amount: '' }); setShowForm(true); };
    const openEdit = (exp: Expense) => { setEditing(exp); setForm({ category: exp.category, description: exp.description, amount: exp.amount.toString() }); setShowForm(true); };

    const handleSave = async () => {
        if (!user || !form.description || !form.amount) return;
        setProcessing(true);
        try {
            if (editing) {
                await api.updateExpense({ ...editing, category: form.category, description: form.description, amount: parseFloat(form.amount) || 0 });
            } else {
                await api.createExpense(user.id, cashRegisterId, { category: form.category, description: form.description, amount: parseFloat(form.amount) || 0 });
            }
            setShowForm(false); setForm({ category: 'Operativos', description: '', amount: '' }); load();
        } catch (e) { alert(String(e)); } finally { setProcessing(false); }
    };

    const handleDelete = async (id: number) => {
        if (!confirm('¿Eliminar este gasto?')) return;
        try { await api.deleteExpense(id); load(); } catch (e) { alert(String(e)); }
    };

    const total = expenses.reduce((s, e) => s + e.amount, 0);

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 h-full flex flex-col animate-fade-in">
            <div className="flex items-center justify-between mb-8">
                <div>
                    <h1 className="text-4xl font-bold text-text-primary">Gastos</h1>
                    <p className="text-text-secondary text-lg mt-2">Total acumulado: <span className="font-semibold text-danger">{formatCurrency(total)}</span></p>
                </div>
                <button onClick={openCreate} className="flex items-center gap-3 px-8 py-4 bg-primary hover:bg-primary-hover text-white rounded-2xl font-bold text-xl transition-colors shadow-[0_4px_20px_rgba(0,224,90,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,224,90,0.4)]">
                    <Plus size={24} strokeWidth={2.5} /> Registrar Gasto
                </button>
            </div>

            <div className="glass rounded-[32px] border border-border flex-1 flex flex-col overflow-hidden shadow-[0_10px_50px_rgba(0,0,0,0.5)]">
                <div className="overflow-auto flex-1">
                    <table className="w-full">
                        <thead>
                            <tr className="border-b border-border/60">
                                <th className="text-left text-base font-bold text-text-muted uppercase px-10 py-6 tracking-wide">Descripción</th>
                                <th className="text-left text-base font-bold text-text-muted uppercase px-10 py-6 tracking-wide">Categoría</th>
                                <th className="text-left text-base font-bold text-text-muted uppercase px-10 py-6 tracking-wide">Usuario</th>
                                <th className="text-right text-base font-bold text-text-muted uppercase px-10 py-6 tracking-wide">Monto</th>
                                <th className="text-left text-base font-bold text-text-muted uppercase px-10 py-6 tracking-wide">Fecha</th>
                                {isAdmin && <th className="text-center text-base font-bold text-text-muted uppercase px-10 py-6 tracking-wide">Acciones</th>}
                            </tr>
                        </thead>
                        <tbody>
                            {expenses.map((e) => (
                                <tr key={e.id} className="border-b border-border/50 hover:bg-white/5 transition-colors">
                                    <td className="px-10 py-6 text-xl font-bold text-text-primary">{e.description}</td>
                                    <td className="px-10 py-6"><span className="text-lg font-bold px-4 py-2 rounded-xl bg-primary/10 text-primary border border-primary/20">{e.category}</span></td>
                                    <td className="px-10 py-6 text-xl text-text-secondary">{e.user_name}</td>
                                    <td className="px-10 py-6 text-right text-2xl font-black text-danger drop-shadow-[0_0_8px_rgba(244,63,94,0.3)]">{formatCurrency(e.amount)}</td>
                                    <td className="px-10 py-6 text-xl text-text-muted font-mono tracking-wider">{formatDateTime(e.created_at)}</td>
                                    {isAdmin && (
                                        <td className="px-10 py-6 text-center">
                                            <div className="flex items-center justify-center gap-4">
                                                <button onClick={() => openEdit(e)} className="p-4 text-text-muted hover:text-primary rounded-xl hover:bg-primary/10 transition-all border border-transparent hover:border-primary/30 hover:scale-110 shadow-[0_4px_10px_rgba(0,0,0,0.1)]"><Edit2 size={24} /></button>
                                                <button onClick={() => handleDelete(e.id)} className="p-4 text-text-muted hover:text-danger rounded-xl hover:bg-danger/10 transition-all border border-transparent hover:border-danger/30 hover:scale-110 shadow-[0_4px_10px_rgba(0,0,0,0.1)]"><Trash2 size={24} /></button>
                                            </div>
                                        </td>
                                    )}
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {expenses.length === 0 && (
                        <div className="py-20 text-center text-text-muted">
                            <DollarSign size={56} className="mx-auto mb-3 opacity-30" />
                            <p className="text-base">Sin gastos registrados</p>
                        </div>
                    )}
                </div>
            </div>

            {/* Create/Edit Modal */}
            {showForm && (
                <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 animate-fade-in">
                    <div className="bg-bg-secondary border border-white/10 rounded-[32px] w-full max-w-2xl p-12 lg:p-14 animate-fade-in shadow-[0_20px_80px_rgba(0,0,0,0.9)]">
                        <div className="flex justify-between mb-8">
                            <h3 className="text-3xl font-black text-text-primary">{editing ? 'Editar Gasto' : 'Nuevo Gasto'}</h3>
                            <button onClick={() => setShowForm(false)} className="text-text-muted hover:text-text-primary text-3xl font-bold">✕</button>
                        </div>
                        <div className="space-y-8">
                            <div>
                                <label className="text-lg font-bold text-text-secondary block mb-3 uppercase tracking-wider">Categoría</label>
                                <select value={form.category} onChange={(e) => setForm({ ...form, category: e.target.value })} className="w-full px-8 py-5 bg-bg-primary border border-border rounded-[20px] text-text-primary text-xl">
                                    {EXPENSE_CATEGORIES.map(c => <option key={c}>{c}</option>)}
                                </select>
                            </div>
                            <div>
                                <label className="text-lg font-bold text-text-secondary block mb-3 uppercase tracking-wider">Descripción</label>
                                <input type="text" value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} className="w-full px-8 py-5 bg-bg-primary border border-border rounded-[20px] text-text-primary text-xl" placeholder="Describe el gasto..." />
                            </div>
                            <div>
                                <label className="text-lg font-bold text-text-secondary block mb-3 uppercase tracking-wider">Monto</label>
                                <input type="number" step="0.01" value={form.amount} onChange={(e) => setForm({ ...form, amount: e.target.value })} className="w-full px-8 py-5 bg-bg-primary border border-border rounded-[20px] text-text-primary text-2xl font-mono tracking-wider" placeholder="0.00" />
                            </div>
                        </div>
                        <div className="flex gap-6 mt-10">
                            <button onClick={() => setShowForm(false)} className="flex-1 py-5 bg-bg-primary border-2 border-border rounded-[20px] text-text-secondary text-xl font-bold hover:bg-surface-hover hover:border-border-light transition-all">Cancelar</button>
                            <button onClick={handleSave} disabled={processing} className="flex-1 py-5 bg-primary hover:bg-primary-hover text-[#0B0B0F] rounded-[20px] text-xl font-black flex items-center justify-center gap-3 transition-all shadow-[0_4px_20px_rgba(0,224,90,0.25)] hover:-translate-y-1 hover:shadow-[0_8px_30px_rgba(0,224,90,0.4)] disabled:opacity-50 disabled:shadow-none disabled:translate-y-0">{processing && <Loader2 size={24} className="animate-spin" />}Guardar</button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
