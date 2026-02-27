import { useEffect, useState } from 'react';
import * as api from '../api';
import type { Supplier } from '../types';
import { Truck, Plus, Edit2, Trash2, X, Loader2, Phone, Mail, MapPin } from 'lucide-react';

export default function SuppliersPage() {
    const [suppliers, setSuppliers] = useState<Supplier[]>([]);
    const [loading, setLoading] = useState(true);
    const [showForm, setShowForm] = useState(false);
    const [editing, setEditing] = useState<Supplier | null>(null);
    const [form, setForm] = useState({ name: '', contact_name: '', phone: '', email: '', address: '', notes: '' });
    const [processing, setProcessing] = useState(false);

    useEffect(() => { load(); }, []);
    const load = async () => { try { setSuppliers(await api.getSuppliers()); } catch (e) { console.error(e); } finally { setLoading(false); } };

    const openCreate = () => { setEditing(null); setForm({ name: '', contact_name: '', phone: '', email: '', address: '', notes: '' }); setShowForm(true); };
    const openEdit = (s: Supplier) => { setEditing(s); setForm({ name: s.name, contact_name: s.contact_name || '', phone: s.phone || '', email: s.email || '', address: s.address || '', notes: s.notes || '' }); setShowForm(true); };

    const handleSave = async () => {
        if (!form.name) return;
        setProcessing(true);
        try {
            if (editing) { await api.updateSupplier({ ...editing, ...form, contact_name: form.contact_name || null, phone: form.phone || null, email: form.email || null, address: form.address || null, notes: form.notes || null }); }
            else { await api.createSupplier({ name: form.name, contact_name: form.contact_name || null, phone: form.phone || null, email: form.email || null, address: form.address || null, notes: form.notes || null }); }
            setShowForm(false); load();
        } catch (e) { alert(String(e)); } finally { setProcessing(false); }
    };

    const handleDelete = async (id: number) => {
        if (!confirm('¿Eliminar proveedor?')) return;
        try { await api.deleteSupplier(id); load(); } catch (e) { alert(String(e)); }
    };

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 space-y-8 animate-fade-in">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold text-text-primary">Proveedores</h1>
                    <p className="text-text-secondary text-sm mt-1">{suppliers.length} proveedores registrados</p>
                </div>
                <button onClick={openCreate} className="flex items-center gap-2 px-6 py-3.5 bg-primary hover:bg-primary-hover text-white rounded-2xl font-medium text-sm transition-colors">
                    <Plus size={18} /> Nuevo Proveedor
                </button>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                {suppliers.map((s) => (
                    <div key={s.id} className="glass rounded-2xl border border-border p-7 hover:border-primary/30 transition-colors">
                        <div className="flex justify-between mb-4">
                            <div className="w-14 h-14 rounded-2xl bg-primary/10 flex items-center justify-center">
                                <Truck size={26} className="text-primary" />
                            </div>
                            <div className="flex gap-2">
                                <button onClick={() => openEdit(s)} className="p-2.5 text-text-muted hover:text-primary rounded-xl hover:bg-primary/10 transition-colors"><Edit2 size={16} /></button>
                                <button onClick={() => handleDelete(s.id)} className="p-2.5 text-text-muted hover:text-danger rounded-xl hover:bg-danger/10 transition-colors"><Trash2 size={16} /></button>
                            </div>
                        </div>
                        <h3 className="font-semibold text-text-primary text-lg">{s.name}</h3>
                        {s.contact_name && <p className="text-sm text-text-secondary mt-1">{s.contact_name}</p>}
                        <div className="mt-4 space-y-2">
                            {s.phone && <p className="text-sm text-text-muted flex items-center gap-2"><Phone size={14} /> {s.phone}</p>}
                            {s.email && <p className="text-sm text-text-muted flex items-center gap-2"><Mail size={14} /> {s.email}</p>}
                            {s.address && <p className="text-sm text-text-muted flex items-center gap-2"><MapPin size={14} /> {s.address}</p>}
                        </div>
                    </div>
                ))}
                {suppliers.length === 0 && (
                    <div className="col-span-full text-center py-20 text-text-muted">
                        <Truck size={56} className="mx-auto mb-3 opacity-30" />
                        <p className="text-base">Sin proveedores registrados</p>
                    </div>
                )}
            </div>

            {/* Modal */}
            {showForm && (
                <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
                    <div className="bg-bg-secondary border border-border rounded-2xl w-full max-w-xl p-10 max-h-[90vh] overflow-y-auto animate-fade-in">
                        <div className="flex justify-between mb-6">
                            <h3 className="text-xl font-bold text-text-primary">{editing ? 'Editar' : 'Nuevo'} Proveedor</h3>
                            <button onClick={() => setShowForm(false)} className="text-text-muted hover:text-text-primary"><X size={22} /></button>
                        </div>
                        <div className="space-y-6">
                            {([
                                { key: 'name', label: 'Nombre *', placeholder: 'Nombre del proveedor' },
                                { key: 'contact_name', label: 'Persona de Contacto', placeholder: 'Nombre del contacto' },
                                { key: 'phone', label: 'Teléfono', placeholder: '+52 ...' },
                                { key: 'email', label: 'Email', placeholder: 'correo@proveedor.com' },
                                { key: 'address', label: 'Dirección', placeholder: 'Dirección completa' },
                                { key: 'notes', label: 'Notas', placeholder: 'Notas adicionales...' },
                            ] as const).map((f) => (
                                <div key={f.key}>
                                    <label className="text-sm font-medium text-text-secondary block mb-2">{f.label}</label>
                                    <input type="text" value={(form as any)[f.key]} onChange={(e) => setForm({ ...form, [f.key]: e.target.value })} className="w-full px-5 py-4 bg-bg-primary border border-border rounded-2xl text-text-primary text-sm" placeholder={f.placeholder} />
                                </div>
                            ))}
                        </div>
                        <div className="flex gap-4 mt-8">
                            <button onClick={() => setShowForm(false)} className="flex-1 py-4 bg-bg-primary border border-border rounded-2xl text-text-secondary text-sm font-medium hover:bg-surface-hover transition-colors">Cancelar</button>
                            <button onClick={handleSave} disabled={processing} className="flex-1 py-4 bg-primary text-white rounded-2xl text-sm font-medium flex items-center justify-center gap-2">{processing && <Loader2 size={18} className="animate-spin" />}Guardar</button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
