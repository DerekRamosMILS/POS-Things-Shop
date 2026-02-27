import { useEffect, useState } from 'react';
import { useSessionStore } from '../stores/useSessionStore';
import * as api from '../api';
import type { User } from '../types';
import { Plus, Lock, X, Loader2, Users } from 'lucide-react';

export default function UsersPage() {
    const { user: currentUser } = useSessionStore();
    const [users, setUsers] = useState<User[]>([]);
    const [loading, setLoading] = useState(true);
    const [showForm, setShowForm] = useState(false);
    const [showPassword, setShowPassword] = useState(false);
    const [passwordUser, setPasswordUser] = useState<User | null>(null);
    const [form, setForm] = useState({ username: '', password: '', full_name: '', role: 'cashier' });
    const [newPassword, setNewPassword] = useState('');
    const [processing, setProcessing] = useState(false);

    useEffect(() => { loadUsers(); }, []);

    const loadUsers = async () => {
        try { const u = await api.getUsers(); setUsers(u); } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    const handleCreate = async () => {
        if (!form.username || !form.password || !form.full_name) return;
        setProcessing(true);
        try { await api.createUser(form); setShowForm(false); setForm({ username: '', password: '', full_name: '', role: 'cashier' }); loadUsers(); } catch (err) { alert(String(err)); } finally { setProcessing(false); }
    };

    const handleChangePassword = async () => {
        if (!passwordUser || !newPassword) return;
        setProcessing(true);
        try { await api.changePassword({ user_id: passwordUser.id, new_password: newPassword }); setShowPassword(false); setNewPassword(''); alert('Contraseña actualizada'); } catch (err) { alert(String(err)); } finally { setProcessing(false); }
    };

    const handleToggleActive = async (u: User) => {
        try { await api.updateUser({ ...u, is_active: !u.is_active }); loadUsers(); } catch (err) { alert(String(err)); }
    };

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 space-y-8 animate-fade-in">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold text-text-primary">Usuarios</h1>
                    <p className="text-text-secondary text-sm mt-1">{users.length} usuarios registrados</p>
                </div>
                <button onClick={() => setShowForm(true)} className="flex items-center gap-2 px-6 py-3.5 bg-primary hover:bg-primary-hover text-white rounded-2xl font-medium text-sm"><Plus size={18} /> Nuevo Usuario</button>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {users.map((u) => (
                    <div key={u.id} className="glass rounded-2xl border border-border p-7">
                        <div className="flex items-start justify-between mb-4">
                            <div className="w-14 h-14 rounded-2xl bg-primary/20 flex items-center justify-center text-primary text-xl font-bold">{u.full_name.charAt(0)}</div>
                            <span className={`text-xs px-3 py-1.5 rounded-full ${u.is_active ? 'bg-success/10 text-success' : 'bg-danger/10 text-danger'}`}>{u.is_active ? 'Activo' : 'Inactivo'}</span>
                        </div>
                        <h3 className="text-lg font-medium text-text-primary">{u.full_name}</h3>
                        <p className="text-sm text-text-muted mt-0.5">@{u.username}</p>
                        <p className="text-xs text-text-secondary mt-1.5 capitalize">{u.role === 'admin' ? 'Administrador' : 'Cajero'}</p>
                        <div className="flex gap-3 mt-5">
                            <button onClick={() => { setPasswordUser(u); setShowPassword(true); }} className="flex-1 py-3 bg-bg-primary border border-border rounded-xl text-text-secondary text-sm hover:bg-surface-hover transition-colors flex items-center justify-center gap-2"><Lock size={14} /> Contraseña</button>
                            {u.id !== currentUser?.id && <button onClick={() => handleToggleActive(u)} className={`flex-1 py-3 border rounded-xl text-sm transition-colors ${u.is_active ? 'border-danger/20 text-danger hover:bg-danger/10' : 'border-success/20 text-success hover:bg-success/10'}`}>{u.is_active ? 'Desactivar' : 'Activar'}</button>}
                        </div>
                    </div>
                ))}
                {users.length === 0 && (
                    <div className="col-span-full text-center py-20 text-text-muted">
                        <Users size={56} className="mx-auto mb-3 opacity-30" /><p className="text-base">No hay usuarios registrados</p>
                    </div>
                )}
            </div>

            {showForm && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
                    <div className="bg-bg-secondary border border-border rounded-2xl w-full max-w-xl p-10 animate-fade-in">
                        <div className="flex justify-between mb-6"><h3 className="text-xl font-bold text-text-primary">Nuevo Usuario</h3><button onClick={() => setShowForm(false)} className="text-text-muted hover:text-text-primary"><X size={22} /></button></div>
                        <div className="space-y-6">
                            <div><label className="text-sm font-medium text-text-secondary block mb-2">Nombre Completo</label><input type="text" value={form.full_name} onChange={(e) => setForm({ ...form, full_name: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" /></div>
                            <div><label className="text-sm font-medium text-text-secondary block mb-2">Usuario</label><input type="text" value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" /></div>
                            <div><label className="text-sm font-medium text-text-secondary block mb-2">Contraseña</label><input type="password" value={form.password} onChange={(e) => setForm({ ...form, password: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" /></div>
                            <div><label className="text-sm font-medium text-text-secondary block mb-2">Rol</label><select value={form.role} onChange={(e) => setForm({ ...form, role: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm"><option value="cashier">Cajero</option><option value="admin">Administrador</option></select></div>
                        </div>
                        <div className="flex gap-4 mt-8"><button onClick={() => setShowForm(false)} className="flex-1 py-4 bg-bg-primary border border-border rounded-2xl text-text-secondary text-sm">Cancelar</button><button onClick={handleCreate} disabled={processing} className="flex-1 py-4 bg-primary text-white rounded-2xl text-sm flex items-center justify-center gap-2">{processing && <Loader2 size={18} className="animate-spin" />}Crear</button></div>
                    </div>
                </div>
            )}

            {showPassword && passwordUser && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
                    <div className="bg-bg-secondary border border-border rounded-2xl w-full max-w-xl p-10 animate-fade-in">
                        <h3 className="text-xl font-bold text-text-primary mb-6">Cambiar Contraseña — {passwordUser.full_name}</h3>
                        <input type="password" value={newPassword} onChange={(e) => setNewPassword(e.target.value)} placeholder="Nueva contraseña" className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm mb-6" autoFocus />
                        <div className="flex gap-4"><button onClick={() => setShowPassword(false)} className="flex-1 py-4 bg-bg-primary border border-border rounded-2xl text-text-secondary text-sm">Cancelar</button><button onClick={handleChangePassword} disabled={processing} className="flex-1 py-4 bg-primary text-white rounded-2xl text-sm flex items-center justify-center gap-2">{processing && <Loader2 size={18} className="animate-spin" />}Cambiar</button></div>
                    </div>
                </div>
            )}
        </div>
    );
}
