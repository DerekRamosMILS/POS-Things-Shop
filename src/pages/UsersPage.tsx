import { useEffect, useState } from 'react';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { User } from '../types';
import { useToast } from '../contexts/ToastContext';

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoPlus   = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>;
const IcoX      = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>;
const IcoLock   = () => <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>;
const IcoEdit   = () => <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>;
const IcoSearch = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>;
const IcoLoader = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>;
const IcoUsers  = () => <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" className="opacity-40"><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>;

// ─── Gradient helper ─────────────────────────────────────────────────────────
const GRAD_PAIRS: [string, string][] = [
    ['#8B78F5','#F0C547'],['#F45270','#F0C547'],['#22D3A0','#8B78F5'],
    ['#F5A842','#F45270'],['#8B78F5','#22D3A0'],
];
function getGrad(name: string): [string, string] {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = (h * 31 + name.charCodeAt(i)) >>> 0;
    return GRAD_PAIRS[h % GRAD_PAIRS.length];
}

export default function UsersPage() {
    const { user: currentUser } = useSessionStore();
    const [users, setUsers] = useState<User[]>([]);
    const [loading, setLoading] = useState(true);
    const [showForm, setShowForm] = useState(false);
    const [showPassword, setShowPassword] = useState(false);
    const [passwordUser, setPasswordUser] = useState<User | null>(null);
    const [editingUser, setEditingUser] = useState<User | null>(null);
    const [form, setForm] = useState({ username: '', password: '', full_name: '', role: 'cashier' });
    const [newPassword, setNewPassword] = useState('');
    const [search, setSearch] = useState('');
    const [roleFilter, setRoleFilter] = useState<'all' | 'admin' | 'cashier'>('all');
    const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'inactive'>('all');
    const [processing, setProcessing] = useState(false);
    const { showToast } = useToast();

    useEffect(() => { loadUsers(); }, []);

    const loadUsers = async () => {
        try { const u = await api.getUsers(); setUsers(u); } catch (err) { showToast(String(err), 'error'); } finally { setLoading(false); }
    };

    const openCreate = () => {
        setEditingUser(null); setForm({ username: '', password: '', full_name: '', role: 'cashier' }); setShowForm(true);
    };
    const openEdit = (u: User) => {
        setEditingUser(u); setForm({ username: u.username, password: '', full_name: u.full_name, role: u.role }); setShowForm(true);
    };

    const handleCreate = async () => {
        if (!form.username || !form.full_name || (!editingUser && !form.password)) return;
        if (!editingUser && form.password.length < 6) {
            showToast('La contraseña debe tener al menos 6 caracteres', 'error'); return;
        }
        setProcessing(true);
        try {
            if (editingUser) {
                await api.updateUser({ id: editingUser.id, username: form.username, full_name: form.full_name, role: form.role, is_active: editingUser.is_active });
            } else {
                await api.createUser(form);
            }
            setShowForm(false); setEditingUser(null); setForm({ username: '', password: '', full_name: '', role: 'cashier' }); loadUsers();
        } catch (err) { showToast(String(err), 'error'); } finally { setProcessing(false); }
    };

    const handleChangePassword = async () => {
        if (!passwordUser || !newPassword) return;
        if (newPassword.length < 6) {
            showToast('La contraseña debe tener al menos 6 caracteres', 'error'); return;
        }
        setProcessing(true);
        try {
            await api.changePassword({ user_id: passwordUser.id, new_password: newPassword });
            setShowPassword(false); setNewPassword(''); showToast('Contraseña actualizada');
        } catch (err) { showToast(String(err), 'error'); } finally { setProcessing(false); }
    };

    const handleToggleActive = async (u: User) => {
        try { await api.updateUser({ ...u, is_active: !u.is_active }); loadUsers(); } catch (err) { showToast(String(err), 'error'); }
    };

    const filtered = users.filter(u => {
        const q = search.toLowerCase().trim();
        if (q && ![u.full_name, u.username, u.role].some(v => v.toLowerCase().includes(q))) return false;
        if (roleFilter !== 'all' && u.role !== roleFilter) return false;
        if (statusFilter === 'active' && !u.is_active) return false;
        if (statusFilter === 'inactive' && u.is_active) return false;
        return true;
    });

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
                    <h1 className="page-title">Usuarios</h1>
                    <p className="page-subtitle">{filtered.length} de {users.length} usuarios registrados</p>
                </div>
                <button onClick={openCreate} className="btn btn-primary" style={{ gap: 7 }}>
                    <IcoPlus /> Nuevo Usuario
                </button>
            </div>

            {/* Filters */}
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
                <div style={{ flex: 1, minWidth: 240, position: 'relative' }}>
                    <span style={{ position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', color: 'var(--t3)', pointerEvents: 'none' }}><IcoSearch /></span>
                    <input value={search} onChange={e => setSearch(e.target.value)} className="input" style={{ paddingLeft: 36 }} placeholder="Buscar usuario, nombre o rol…" />
                </div>
                <select value={roleFilter} onChange={e => setRoleFilter(e.target.value as 'all' | 'admin' | 'cashier')} className="input" style={{ width: 'auto', minWidth: 150 }}>
                    <option value="all">Todos los roles</option>
                    <option value="admin">Administradores</option>
                    <option value="cashier">Cajeros</option>
                </select>
                <select value={statusFilter} onChange={e => setStatusFilter(e.target.value as 'all' | 'active' | 'inactive')} className="input" style={{ width: 'auto', minWidth: 130 }}>
                    <option value="all">Todos</option>
                    <option value="active">Activos</option>
                    <option value="inactive">Inactivos</option>
                </select>
            </div>

            {/* User cards */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))', gap: 14 }}>
                {filtered.map(u => {
                    const [gradA, gradB] = getGrad(u.full_name);
                    return (
                        <div key={u.id} className="card" style={{ padding: 20 }}>
                            <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', marginBottom: 14 }}>
                                <div style={{ width: 44, height: 44, borderRadius: 13, background: `linear-gradient(135deg,${gradA},${gradB})`, display: 'grid', placeItems: 'center', color: '#fff', fontWeight: 800, fontSize: 18, flexShrink: 0 }}>
                                    {u.full_name.charAt(0).toUpperCase()}
                                </div>
                                <span className={`badge ${u.is_active ? 'badge-success' : 'badge-muted'}`}>
                                    {u.is_active ? 'Activo' : 'Inactivo'}
                                </span>
                            </div>
                            <h3 style={{ fontSize: 14, fontWeight: 700, color: 'var(--t1)' }}>{u.full_name}</h3>
                            <p style={{ fontSize: 12, color: 'var(--t3)', marginTop: 2 }}>@{u.username}</p>
                            <p style={{ fontSize: 11, fontWeight: 600, color: u.role === 'admin' ? 'var(--primary)' : 'var(--t3)', marginTop: 4, textTransform: 'uppercase', letterSpacing: '0.06em' }}>
                                {u.role === 'admin' ? 'Administrador' : 'Cajero'}
                            </p>
                            <div style={{ display: 'flex', gap: 8, marginTop: 14 }}>
                                <button onClick={() => openEdit(u)} className="btn btn-ghost btn-sm" style={{ gap: 6 }}>
                                    <IcoEdit />
                                </button>
                                <button onClick={() => { setPasswordUser(u); setShowPassword(true); }} className="btn btn-ghost btn-sm" style={{ flex: 1, justifyContent: 'center', gap: 6 }}>
                                    <IcoLock /> Contraseña
                                </button>
                                {u.id !== currentUser?.id && (
                                    <button onClick={() => handleToggleActive(u)} className={`btn btn-sm ${u.is_active ? 'btn-danger' : 'btn-success'}`} style={{ flex: 1, justifyContent: 'center' }}>
                                        {u.is_active ? 'Desactivar' : 'Activar'}
                                    </button>
                                )}
                            </div>
                        </div>
                    );
                })}
                {filtered.length === 0 && (
                    <div style={{ gridColumn: '1 / -1', padding: '64px 0', textAlign: 'center', color: 'var(--t3)' }}>
                        <IcoUsers />
                        <p style={{ marginTop: 12, fontSize: 13 }}>No hay usuarios registrados</p>
                    </div>
                )}
            </div>

            {/* Create/Edit Modal */}
            {showForm && (
                <div className="modal-overlay" onClick={() => setShowForm(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 360, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                            <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>{editingUser ? 'Editar Usuario' : 'Nuevo Usuario'}</h3>
                            <button onClick={() => setShowForm(false)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t1)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; }}
                            ><IcoX /></button>
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div>
                                <label className="form-label">Nombre Completo</label>
                                <input type="text" value={form.full_name} onChange={e => setForm({ ...form, full_name: e.target.value })} className="input" />
                            </div>
                            <div>
                                <label className="form-label">Usuario</label>
                                <input type="text" value={form.username} onChange={e => setForm({ ...form, username: e.target.value })} className="input" />
                            </div>
                            {!editingUser && (
                                <div>
                                    <label className="form-label">Contraseña</label>
                                    <input type="password" value={form.password} onChange={e => setForm({ ...form, password: e.target.value })} className="input" />
                                </div>
                            )}
                            <div>
                                <label className="form-label">Rol</label>
                                <select value={form.role} onChange={e => setForm({ ...form, role: e.target.value })} className="input">
                                    <option value="cashier">Cajero</option>
                                    <option value="admin">Administrador</option>
                                </select>
                            </div>
                            <div style={{ display: 'flex', gap: 10, paddingTop: 4 }}>
                                <button onClick={() => setShowForm(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handleCreate} disabled={processing} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>
                                    {processing && <IcoLoader />} {editingUser ? 'Guardar' : 'Crear'}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {/* Change Password Modal */}
            {showPassword && passwordUser && (
                <div className="modal-overlay" onClick={() => setShowPassword(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 360, padding: 24 }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                            <div>
                                <h3 style={{ fontSize: 16, fontWeight: 800, color: 'var(--t1)' }}>Cambiar Contraseña</h3>
                                <p style={{ fontSize: 12, color: 'var(--t3)', marginTop: 2 }}>{passwordUser.full_name}</p>
                            </div>
                            <button onClick={() => setShowPassword(false)} style={{ padding: 6, borderRadius: 9, color: 'var(--t3)' }}
                                onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.background = 'rgba(255,255,255,0.08)'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t1)'; }}
                                onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.background = 'transparent'; (e.currentTarget as HTMLButtonElement).style.color = 'var(--t3)'; }}
                            ><IcoX /></button>
                        </div>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                            <div>
                                <label className="form-label">Nueva Contraseña</label>
                                <input type="password" value={newPassword} onChange={e => setNewPassword(e.target.value)} className="input" placeholder="Mínimo 6 caracteres" autoFocus />
                            </div>
                            <div style={{ display: 'flex', gap: 10 }}>
                                <button onClick={() => setShowPassword(false)} className="btn btn-ghost" style={{ flex: 1, justifyContent: 'center' }}>Cancelar</button>
                                <button onClick={handleChangePassword} disabled={processing} className="btn btn-primary" style={{ flex: 1, justifyContent: 'center' }}>
                                    {processing && <IcoLoader />} Cambiar
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
