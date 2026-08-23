import { useEffect, useState } from 'react';
import { formatCurrency, formatDateTime, EXPENSE_CATEGORIES } from '../utils';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { Expense } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoPlus   = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>;
const IcoX      = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoEdit   = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>;
const IcoTrash  = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4h6v2"/></svg>;
const IcoSearch = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>;
const IcoFilter = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3"/></svg>;
const IcoLoader = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;
const IcoDollar = () => <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" className="opacity-40"><line x1="12" y1="1" x2="12" y2="23"/><path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/></svg>;

function IcoBtn({ onClick, children, hoverColor = 'var(--primary)', hoverBg = 'rgba(139,120,245,0.10)' }: {
    onClick: () => void; children: React.ReactNode; hoverColor?: string; hoverBg?: string;
}) {
    return (
        <button onClick={onClick} className="p-2 rounded-lg" style={{ color: 'var(--t3)' }}
            onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.color = hoverColor; (e.currentTarget as HTMLButtonElement).style.background = hoverBg; }}
            onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
        >{children}</button>
    );
}

export default function ExpensesPage() {
    const { user, cashRegisterId } = useSessionStore();
    const isAdmin = user?.role === 'admin';
    const [expenses, setExpenses] = useState<Expense[]>([]);
    const [loading, setLoading] = useState(true);
    const [showForm, setShowForm] = useState(false);
    const [editing, setEditing] = useState<Expense | null>(null);
    const [form, setForm] = useState({ category: 'Operativos', description: '', amount: '' });
    const [search, setSearch] = useState('');
    const [categoryFilter, setCategoryFilter] = useState('');
    const [dateFrom, setDateFrom] = useState('');
    const [dateTo, setDateTo] = useState('');
    const [processing, setProcessing] = useState(false);
    const { showToast } = useToast();
    const { confirm } = useConfirm();

    useEffect(() => { load(); }, []);

    const load = async () => {
        try { setExpenses(await api.getExpenses()); } catch (e) { showToast(String(e), 'error'); } finally { setLoading(false); }
    };

    const openCreate = () => {
        setEditing(null); setForm({ category: 'Operativos', description: '', amount: '' }); setShowForm(true);
    };
    const openEdit = (exp: Expense) => {
        setEditing(exp); setForm({ category: exp.category, description: exp.description, amount: exp.amount.toString() }); setShowForm(true);
    };

    const handleSave = async () => {
        if (!user || !form.description || !form.amount) return;
        setProcessing(true);
        try {
            if (editing) {
                await api.updateExpense({ ...editing, category: form.category, description: form.description, amount: parseFloat(form.amount) || 0 });
            } else {
                await api.createExpense(cashRegisterId, { category: form.category, description: form.description, amount: parseFloat(form.amount) || 0 });
            }
            setShowForm(false); setForm({ category: 'Operativos', description: '', amount: '' }); load();
        } catch (e) { showToast(String(e), 'error'); } finally { setProcessing(false); }
    };

    const handleDelete = async (id: number) => {
        const ok = await confirm({ title: 'Eliminar gasto', message: '¿Eliminar este gasto? Esta acción no se puede deshacer.', variant: 'danger', confirmLabel: 'Eliminar' });
        if (!ok) return;
        try { await api.deleteExpense(id); load(); } catch (e) { showToast(String(e), 'error'); }
    };

    const filtered = expenses.filter(e => {
        const q = search.toLowerCase().trim();
        if (q && ![e.description, e.category, e.user_name || ''].some(v => v.toLowerCase().includes(q))) return false;
        if (categoryFilter && e.category !== categoryFilter) return false;
        if (dateFrom && e.created_at.slice(0, 10) < dateFrom) return false;
        if (dateTo && e.created_at.slice(0, 10) > dateTo) return false;
        return true;
    });

    const total = filtered.reduce((s, e) => s + e.amount, 0);
    const avgExpense = filtered.length > 0 ? total / filtered.length : 0;
    const categories = Array.from(new Set(expenses.map(e => e.category)));
    const hasFilters = !!(search || categoryFilter || dateFrom || dateTo);

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
                    <h1 className="page-title">Gastos</h1>
                    <p className="page-subtitle">Total filtrado: <span style={{ color: 'var(--danger)', fontWeight: 700 }}>{formatCurrency(total)}</span></p>
                </div>
                <button onClick={openCreate} className="btn btn-primary" style={{ gap: 7 }}>
                    <IcoPlus /> Registrar Gasto
                </button>
            </div>

            {/* KPIs */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 14 }}>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Total</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--danger)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(total)}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Registros</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{filtered.length}</p>
                </div>
                <div className="card" style={{ padding: '20px 24px' }}>
                    <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>Promedio</p>
                    <p style={{ fontSize: 22, fontWeight: 900, color: 'var(--accent)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(avgExpense)}</p>
                </div>
            </div>

            {/* Filters */}
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
                <div style={{ flex: 1, minWidth: 240, position: 'relative' }}>
                    <span style={{ position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--t3)', pointerEvents: 'none' }}><IcoSearch /></span>
                    <input value={search} onChange={e => setSearch(e.target.value)} className="input" style={{ paddingLeft: 36 }} placeholder="Buscar gasto, categoría o cajero…" />
                </div>
                <select value={categoryFilter} onChange={e => setCategoryFilter(e.target.value)} className="input" style={{ width: 'auto', minWidth: 160 }}>
                    <option value="">Todas las categorías</option>
                    {categories.map(c => <option key={c} value={c}>{c}</option>)}
                </select>
                <input type="date" value={dateFrom} onChange={e => setDateFrom(e.target.value)} className="input" style={{ width: 'auto' }} />
                <input type="date" value={dateTo} onChange={e => setDateTo(e.target.value)} className="input" style={{ width: 'auto' }} />
                {hasFilters && (
                    <button onClick={() => { setSearch(''); setCategoryFilter(''); setDateFrom(''); setDateTo(''); }} className="btn btn-ghost btn-sm" style={{ gap: 6 }}>
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
                                <th>Categoría</th>
                                <th>Descripción</th>
                                <th>Cajero</th>
                                <th>Fecha</th>
                                <th style={{ textAlign: 'right' }}>Monto</th>
                                <th style={{ textAlign: 'center', width: 80 }}>Acciones</th>
                            </tr>
                        </thead>
                        <tbody>
                            {filtered.map(e => (
                                <tr key={e.id}>
                                    <td><span className="badge badge-muted">{e.category}</span></td>
                                    <td style={{ fontSize: 13, fontWeight: 500, color: 'var(--t1)' }}>{e.description}</td>
                                    <td style={{ fontSize: 12, color: 'var(--t2)' }}>{e.user_name || '—'}</td>
                                    <td style={{ fontFamily: 'monospace', fontSize: 11, color: 'var(--t3)' }}>{formatDateTime(e.created_at)}</td>
                                    <td style={{ textAlign: 'right' }}>
                                        <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--danger)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(e.amount)}</span>
                                    </td>
                                    <td>
                                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 2 }}>
                                            {isAdmin ? (
                                                <>
                                                    <IcoBtn onClick={() => openEdit(e)}><IcoEdit /></IcoBtn>
                                                    <IcoBtn onClick={() => handleDelete(e.id)} hoverColor="var(--danger)" hoverBg="rgba(244,82,112,0.10)"><IcoTrash /></IcoBtn>
                                                </>
                                            ) : (
                                                <span style={{ fontSize: 12, color: 'var(--t3)' }}>—</span>
                                            )}
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {filtered.length === 0 && (
                        <div style={{ padding: '64px 0', textAlign: 'center', color: 'var(--t3)' }}>
                            <IcoDollar />
                            <p style={{ marginTop: 12, fontSize: 13 }}>Sin gastos registrados</p>
                        </div>
                    )}
                </div>
            </div>

            {/* Form Modal */}
            {showForm && (
                <div className="modal-overlay" onClick={() => setShowForm(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 360, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>{editing ? 'Editar Gasto' : 'Registrar Gasto'}</h3>
                            <button onClick={() => setShowForm(false)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t1)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; }}
                            ><IcoX /></button>
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div>
                                <label className="form-label">Categoría</label>
                                <select value={form.category} onChange={e => setForm({ ...form, category: e.target.value })} className="input">
                                    {(EXPENSE_CATEGORIES || ['Operativos','Servicios','Suministros','Transporte','Comida','Mantenimiento','Otros']).map((c: string) => (
                                        <option key={c} value={c}>{c}</option>
                                    ))}
                                </select>
                            </div>
                            <div>
                                <label className="form-label">Descripción</label>
                                <input type="text" value={form.description} onChange={e => setForm({ ...form, description: e.target.value })} className="input" placeholder="Ej. Pago de luz" />
                            </div>
                            <div>
                                <label className="form-label">Monto</label>
                                <input type="number" step="0.01" value={form.amount} onChange={e => setForm({ ...form, amount: e.target.value })} className="input" style={{ fontFamily: 'monospace' }} placeholder="0.00" />
                            </div>
                            <div style={{ display: 'flex', gap: 10 }}>
                                <button onClick={() => setShowForm(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handleSave} disabled={processing} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>
                                    {processing && <IcoLoader />} {editing ? 'Guardar' : 'Registrar'}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
