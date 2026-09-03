import { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { formatCurrency } from '../utils';
import * as api from '../api';
import type { CatalogoFiscal, Customer } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useConfirm } from '../contexts/ConfirmContext';

const IcoPlus = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>;
const IcoX = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoEdit = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>;
const IcoTrash = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/></svg>;
const IcoSearch = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>;
const IcoLoader = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;

function IcoBtn({ onClick, children, hoverColor = 'var(--primary)', hoverBg = 'rgba(139,120,245,0.10)' }: { onClick: () => void; children: React.ReactNode; hoverColor?: string; hoverBg?: string; }) {
    return (
        <button onClick={onClick} className="p-2 rounded-lg" style={{ color: 'var(--t3)' }}
            onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.color = hoverColor; (e.currentTarget as HTMLButtonElement).style.background = hoverBg; }}
            onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; }}
        >{children}</button>
    );
}

export default function CustomersPage() {
    const [customers, setCustomers] = useState<Customer[]>([]);
    const [loading, setLoading] = useState(true);
    const [showForm, setShowForm] = useState(false);
    const [editing, setEditing] = useState<Customer | null>(null);
    const [form, setForm] = useState({ name: '', phone: '', email: '', notes: '', rfc: '', razon_social: '', regimen_fiscal: '', cp_fiscal: '', uso_cfdi: '' });
    const [catalogos, setCatalogos] = useState<CatalogoFiscal>({ regimenes: [], usos_cfdi: [] });
    const [showFiscal, setShowFiscal] = useState(false);
    const [searchParams] = useSearchParams();
    const [search, setSearch] = useState(searchParams.get('search') || '');
    const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'inactive'>('active');
    const [processing, setProcessing] = useState(false);
    const { showToast } = useToast();
    const { confirm } = useConfirm();

    useEffect(() => { load(); api.getCatalogosFiscales().then(setCatalogos).catch(() => {}); }, []);
    useEffect(() => { const s = searchParams.get('search'); if (s) setSearch(s); }, [searchParams]);

    const load = async () => {
        try { setCustomers(await api.getCustomers()); } catch (e) { showToast(String(e), 'error'); } finally { setLoading(false); }
    };

    const openCreate = () => { setEditing(null); setForm({ name: '', phone: '', email: '', notes: '', rfc: '', razon_social: '', regimen_fiscal: '', cp_fiscal: '', uso_cfdi: '' }); setShowFiscal(false); setShowForm(true); };
    const openEdit = (c: Customer) => {
        setEditing(c);
        setForm({
            name: c.name, phone: c.phone || '', email: c.email || '', notes: c.notes || '',
            rfc: c.rfc || '', razon_social: c.razon_social || '',
            regimen_fiscal: c.regimen_fiscal || '', cp_fiscal: c.cp_fiscal || '',
            uso_cfdi: c.uso_cfdi || '',
        });
        // Si el cliente ya factura, la sección se abre sola.
        setShowFiscal(Boolean(c.rfc));
        setShowForm(true);
    };

    /// Los campos fiscales vacíos se mandan como null: el backend distingue
    /// "sin capturar" de "capturado vacío" al validar.
    const fiscalPayload = () => ({
        rfc: form.rfc.trim() || null,
        razon_social: form.razon_social.trim() || null,
        regimen_fiscal: form.regimen_fiscal || null,
        cp_fiscal: form.cp_fiscal.trim() || null,
        uso_cfdi: form.uso_cfdi || null,
    });

    const handleSave = async () => {
        if (!form.name.trim()) { showToast('El nombre es requerido', 'error'); return; }
        setProcessing(true);
        try {
            if (editing) {
                await api.updateCustomer({ id: editing.id, name: form.name, phone: form.phone || null, email: form.email || null, notes: form.notes || null, ...fiscalPayload(), is_active: editing.is_active });
            } else {
                await api.createCustomer({ name: form.name, phone: form.phone || null, email: form.email || null, notes: form.notes || null, ...fiscalPayload() });
            }
            setShowForm(false); load();
            showToast(editing ? 'Cliente actualizado' : 'Cliente creado', 'success');
        } catch (e) { showToast(String(e), 'error'); } finally { setProcessing(false); }
    };

    const handleDelete = async (c: Customer) => {
        const ok = await confirm({ title: 'Desactivar cliente', message: `¿Desactivar a "${c.name}"?`, variant: 'warning', confirmLabel: 'Desactivar' });
        if (!ok) return;
        try { await api.deleteCustomer(c.id); load(); } catch (e) { showToast(String(e), 'error'); }
    };

    const filtered = customers.filter(c => {
        const q = search.toLowerCase().trim();
        if (q && ![c.name, c.phone || '', c.email || ''].some(v => v.toLowerCase().includes(q))) return false;
        if (statusFilter === 'active' && !c.is_active) return false;
        if (statusFilter === 'inactive' && c.is_active) return false;
        return true;
    });

    if (loading) return <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}><IcoLoader /></div>;

    return (
        <div className="page-container">
            <div className="page-header">
                <div>
                    <h1 className="page-title">Clientes</h1>
                    <p className="page-subtitle">{filtered.length} de {customers.length} clientes</p>
                </div>
                <button onClick={openCreate} className="btn btn-primary" style={{ gap: 7 }}><IcoPlus /> Nuevo Cliente</button>
            </div>

            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
                <div style={{ flex: 1, minWidth: 240, position: 'relative' }}>
                    <span style={{ position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--t3)', pointerEvents: 'none' }}><IcoSearch /></span>
                    <input value={search} onChange={e => setSearch(e.target.value)} className="input" style={{ paddingLeft: 36 }} placeholder="Buscar nombre, teléfono o email…" />
                </div>
                <select value={statusFilter} onChange={e => setStatusFilter(e.target.value as 'all' | 'active' | 'inactive')} className="input" style={{ width: 'auto', minWidth: 130 }}>
                    <option value="active">Activos</option>
                    <option value="inactive">Inactivos</option>
                    <option value="all">Todos</option>
                </select>
            </div>

            <div className="card" style={{ flex: 1, padding: 0, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
                <div style={{ overflowX: 'auto', overflowY: 'auto', flex: 1 }}>
                    <table className="table-base">
                        <thead>
                            <tr>
                                <th>Nombre</th>
                                <th>Teléfono</th>
                                <th>Email</th>
                                <th style={{ textAlign: 'right' }}>Compras</th>
                                <th style={{ textAlign: 'right' }}>Total</th>
                                <th style={{ textAlign: 'center' }}>Estado</th>
                                <th style={{ textAlign: 'center', width: 90 }}>Acciones</th>
                            </tr>
                        </thead>
                        <tbody>
                            {filtered.map(c => (
                                <tr key={c.id}>
                                    <td style={{ fontWeight: 600, color: 'var(--t1)', fontSize: 13 }}>{c.name}</td>
                                    <td style={{ fontSize: 13, color: 'var(--t2)' }}>{c.phone || '—'}</td>
                                    <td style={{ fontSize: 13, color: 'var(--t2)' }}>{c.email || '—'}</td>
                                    <td style={{ textAlign: 'right', fontSize: 13, color: 'var(--t2)', fontVariantNumeric: 'tabular-nums' }}>{c.purchase_count ?? 0}</td>
                                    <td style={{ textAlign: 'right', fontSize: 13, fontWeight: 700, color: 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(c.total_purchases ?? 0)}</td>
                                    <td style={{ textAlign: 'center' }}>
                                        <span className={`badge ${c.is_active ? 'badge-success' : 'badge-muted'}`}>{c.is_active ? 'Activo' : 'Inactivo'}</span>
                                    </td>
                                    <td>
                                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 2 }}>
                                            <IcoBtn onClick={() => openEdit(c)}><IcoEdit /></IcoBtn>
                                            {c.is_active && <IcoBtn onClick={() => handleDelete(c)} hoverColor="var(--danger)" hoverBg="rgba(244,82,112,0.10)"><IcoTrash /></IcoBtn>}
                                        </div>
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {filtered.length === 0 && <div style={{ padding: '64px 0', textAlign: 'center', color: 'var(--t3)', fontSize: 13 }}>Sin clientes</div>}
                </div>
            </div>

            {showForm && (
                <div className="modal-overlay" onClick={() => setShowForm(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 400, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>{editing ? 'Editar Cliente' : 'Nuevo Cliente'}</h3>
                            <button onClick={() => setShowForm(false)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}><IcoX /></button>
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div><label className="form-label">Nombre *</label><input value={form.name} onChange={e => setForm({ ...form, name: e.target.value })} className="input" autoFocus /></div>
                            <div><label className="form-label">Teléfono</label><input value={form.phone} onChange={e => setForm({ ...form, phone: e.target.value })} className="input" /></div>
                            <div><label className="form-label">Email</label><input value={form.email} onChange={e => setForm({ ...form, email: e.target.value })} className="input" /></div>
                            <div><label className="form-label">Notas</label><textarea value={form.notes} onChange={e => setForm({ ...form, notes: e.target.value })} className="input" style={{ resize: 'none', height: 64 }} /></div>

                            {/* Datos fiscales: se piden una vez y sirven para todas
                                sus facturas. Plegados porque la mayoría no factura. */}
                            <button
                                type="button"
                                onClick={() => setShowFiscal(v => !v)}
                                style={{ display: 'flex', alignItems: 'center', gap: 8, background: 'none', border: 'none', padding: 0, cursor: 'pointer', fontFamily: 'inherit', fontSize: 12, fontWeight: 700, color: 'var(--primary)' }}
                            >
                                {showFiscal ? '−' : '+'} Datos para facturar {form.rfc && !showFiscal ? `(${form.rfc})` : ''}
                            </button>

                            {showFiscal && (
                                <div style={{ display: 'flex', flexDirection: 'column', gap: 12, padding: 14, borderRadius: 12, background: 'rgba(255,255,255,0.03)', border: '1px solid rgba(255,255,255,0.07)' }}>
                                    <div>
                                        <label className="form-label">RFC</label>
                                        <input value={form.rfc} onChange={e => setForm({ ...form, rfc: e.target.value.toUpperCase() })}
                                            className="input" placeholder="XAXX010101000" style={{ fontFamily: 'monospace' }} />
                                    </div>
                                    <div>
                                        <label className="form-label">Razón social</label>
                                        <input value={form.razon_social} onChange={e => setForm({ ...form, razon_social: e.target.value })}
                                            className="input" placeholder="Como aparece en su constancia fiscal" />
                                    </div>
                                    <div>
                                        <label className="form-label">Régimen fiscal</label>
                                        <select value={form.regimen_fiscal} onChange={e => setForm({ ...form, regimen_fiscal: e.target.value })} className="input">
                                            <option value="">Sin especificar</option>
                                            {catalogos.regimenes.map(r => <option key={r.clave} value={r.clave}>{r.clave} — {r.nombre}</option>)}
                                        </select>
                                    </div>
                                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1.4fr', gap: 10 }}>
                                        <div>
                                            <label className="form-label">C.P. fiscal</label>
                                            <input value={form.cp_fiscal} onChange={e => setForm({ ...form, cp_fiscal: e.target.value })}
                                                className="input" placeholder="64000" maxLength={5} />
                                        </div>
                                        <div>
                                            <label className="form-label">Uso del CFDI</label>
                                            <select value={form.uso_cfdi} onChange={e => setForm({ ...form, uso_cfdi: e.target.value })} className="input">
                                                <option value="">Sin especificar</option>
                                                {catalogos.usos_cfdi.map(u => <option key={u.clave} value={u.clave}>{u.clave} — {u.nombre}</option>)}
                                            </select>
                                        </div>
                                    </div>
                                </div>
                            )}
                            <div style={{ display: 'flex', gap: 10 }}>
                                <button onClick={() => setShowForm(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handleSave} disabled={processing} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>{processing && <IcoLoader />} {editing ? 'Guardar' : 'Crear'}</button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
