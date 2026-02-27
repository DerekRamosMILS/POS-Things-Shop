import { useEffect, useState } from 'react';
import { formatCurrency, formatDateTime, STATUS_LABELS, PAYMENT_METHOD_LABELS } from '../utils';
import * as api from '../api';
import type { Sale } from '../types';
import { Receipt, Eye, XCircle, Search } from 'lucide-react';
import { useSessionStore } from '../stores/useSessionStore';

export default function SalesPage() {
    const [sales, setSales] = useState<Sale[]>([]);
    const [loading, setLoading] = useState(true);
    const [detail, setDetail] = useState<Sale | null>(null);
    const [search, setSearch] = useState('');
    const { user } = useSessionStore();

    useEffect(() => { loadSales(); }, []);

    const loadSales = async () => {
        try { const s = await api.getSales(); setSales(s); } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    const handleViewDetail = async (saleId: number) => {
        try { const d = await api.getSaleDetail(saleId); setDetail(d); } catch (err) { alert(String(err)); }
    };

    const handleCancel = async (saleId: number) => {
        if (!user || !confirm('¿Cancelar esta venta? Se restaurará el inventario.')) return;
        try { await api.cancelSale(saleId, user.id); loadSales(); setDetail(null); } catch (err) { alert(String(err)); }
    };

    const filtered = sales.filter((s) => !search || s.folio.toLowerCase().includes(search.toLowerCase()));

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 h-full flex flex-col animate-fade-in">
            <div className="flex items-center justify-between mb-8">
                <div>
                    <h1 className="text-4xl font-bold text-text-primary">Ventas</h1>
                    <p className="text-text-secondary text-lg mt-2">{sales.length} ventas registradas</p>
                </div>
                <div className="relative w-96">
                    <Search className="absolute left-6 top-1/2 -translate-y-1/2 text-text-muted" size={24} />
                    <input type="text" value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Buscar por folio..." className="w-full pl-[68px] pr-6 py-5 bg-bg-secondary border border-border rounded-2xl text-text-primary text-xl placeholder-text-muted focus:border-primary transition-colors focus:shadow-[0_0_15px_rgba(0,224,90,0.15)] shadow-[inset_0_2px_10px_rgba(0,0,0,0.2)]" />
                </div>
            </div>

            <div className="glass rounded-[32px] border border-border flex-1 flex flex-col overflow-hidden shadow-[0_10px_50px_rgba(0,0,0,0.5)]">
                <div className="overflow-auto flex-1">
                    <table className="w-full">
                        <thead>
                            <tr className="border-b border-border">
                                <th className="text-left text-base font-bold text-text-muted uppercase px-8 py-6 tracking-wide">Folio</th>
                                <th className="text-left text-base font-bold text-text-muted uppercase px-8 py-6 tracking-wide">Fecha</th>
                                <th className="text-left text-base font-bold text-text-muted uppercase px-8 py-6 tracking-wide">Cajero</th>
                                <th className="text-left text-base font-bold text-text-muted uppercase px-8 py-6 tracking-wide">Pago</th>
                                <th className="text-right text-base font-bold text-text-muted uppercase px-8 py-6 tracking-wide">Total</th>
                                <th className="text-center text-base font-bold text-text-muted uppercase px-8 py-6 tracking-wide">Estado</th>
                                <th className="text-center text-base font-bold text-text-muted uppercase px-8 py-6 tracking-wide">Acciones</th>
                            </tr>
                        </thead>
                        <tbody>
                            {filtered.map((s) => (
                                <tr key={s.id} className="border-b border-border/50 hover:bg-white/5 transition-colors">
                                    <td className="px-8 py-6 text-xl font-bold text-primary">{s.folio}</td>
                                    <td className="px-8 py-6 text-lg text-text-secondary font-mono tracking-wider">{formatDateTime(s.created_at)}</td>
                                    <td className="px-8 py-6 text-xl text-text-secondary">{s.user_name}</td>
                                    <td className="px-8 py-6 text-xl text-text-secondary">{PAYMENT_METHOD_LABELS[s.payment_method] || s.payment_method}</td>
                                    <td className="px-8 py-6 text-right text-2xl font-black text-text-primary tracking-tight">{formatCurrency(s.total)}</td>
                                    <td className="px-8 py-6 text-center"><span className={`text-base font-bold px-5 py-2.5 rounded-xl border border-transparent ${s.status === 'completed' ? 'bg-success/10 text-success border-success/30' : 'bg-danger/10 text-danger border-danger/30'}`}>{STATUS_LABELS[s.status] || s.status}</span></td>
                                    <td className="px-8 py-6"><div className="flex items-center justify-center gap-4">
                                        <button onClick={() => handleViewDetail(s.id)} className="p-4 text-text-muted hover:text-primary hover:bg-primary/10 rounded-xl transition-all border border-transparent hover:border-primary/30 hover:scale-110 shadow-[0_4px_10px_rgba(0,0,0,0.1)]"><Eye size={24} /></button>
                                        {s.status === 'completed' && <button onClick={() => handleCancel(s.id)} className="p-4 text-text-muted hover:text-danger hover:bg-danger/10 rounded-xl transition-all border border-transparent hover:border-danger/30 hover:scale-110 shadow-[0_4px_10px_rgba(0,0,0,0.1)]"><XCircle size={24} /></button>}
                                    </div></td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {filtered.length === 0 && <div className="py-24 text-center text-text-muted"><Receipt size={72} strokeWidth={1.5} className="mx-auto mb-5 opacity-30" /><p className="text-xl font-medium tracking-wide">No se encontraron ventas</p></div>}
                </div>
            </div>

            {/* Detail Modal */}
            {detail && (
                <div className="fixed inset-0 bg-black/80 backdrop-blur-md flex items-center justify-center z-50 animate-fade-in" onClick={() => setDetail(null)}>
                    <div className="bg-bg-secondary border border-white/10 rounded-[32px] w-full max-w-3xl p-12 lg:p-14 animate-fade-in shadow-[0_20px_80px_rgba(0,0,0,0.9)]" onClick={(e) => e.stopPropagation()}>
                        <div className="flex justify-between mb-10">
                            <h3 className="text-3xl font-black text-text-primary">Venta {detail.folio}</h3>
                            <button onClick={() => setDetail(null)} className="text-text-muted hover:text-text-primary text-3xl font-bold">✕</button>
                        </div>
                        <div className="space-y-8">
                            <div className="grid grid-cols-2 gap-6">
                                <div className="bg-bg-primary rounded-2xl p-6 border border-border">
                                    <p className="text-base font-bold text-text-muted uppercase tracking-widest mb-2">Fecha</p>
                                    <p className="text-xl font-bold text-text-primary font-mono tracking-wider">{formatDateTime(detail.created_at)}</p>
                                </div>
                                <div className="bg-bg-primary rounded-2xl p-6 border border-border">
                                    <p className="text-base font-bold text-text-muted uppercase tracking-widest mb-2">Método de Pago</p>
                                    <p className="text-xl font-bold text-text-primary">{PAYMENT_METHOD_LABELS[detail.payment_method]}</p>
                                </div>
                            </div>
                            <div className="bg-bg-primary rounded-2xl p-8 space-y-4 border border-border">
                                {detail.items?.map((item) => (
                                    <div key={item.id} className="flex justify-between text-xl py-4 border-b border-border/50 last:border-0">
                                        <span className="text-text-secondary font-medium"><span className="text-text-muted mr-3">{item.quantity}x</span> {item.product_name}</span>
                                        <span className="text-text-primary font-black drop-shadow-[0_0_8px_rgba(255,255,255,0.1)]">{formatCurrency(item.subtotal)}</span>
                                    </div>
                                ))}
                            </div>
                            <div className="bg-primary/10 border-2 border-primary/30 rounded-3xl p-8 flex justify-between items-center shadow-[0_10px_30px_rgba(0,224,90,0.15)]">
                                <span className="text-3xl font-black text-text-primary uppercase tracking-widest">Total</span>
                                <span className="text-5xl font-black text-primary drop-shadow-[0_0_15px_rgba(0,224,90,0.4)]">{formatCurrency(detail.total)}</span>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
