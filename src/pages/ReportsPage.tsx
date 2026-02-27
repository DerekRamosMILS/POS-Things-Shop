import { useEffect, useState } from 'react';
import { formatCurrency } from '../utils';
import * as api from '../api';
import type { DailySalesReport, TopProduct } from '../types';
import { BarChart3, Package } from 'lucide-react';

export default function ReportsPage() {
    const [dailyReport, setDailyReport] = useState<DailySalesReport[]>([]);
    const [topProducts, setTopProducts] = useState<TopProduct[]>([]);
    const [days, setDays] = useState(30);
    const [loading, setLoading] = useState(true);

    useEffect(() => { loadData(); }, [days]);

    const loadData = async () => {
        setLoading(true);
        try {
            const [dr, tp] = await Promise.all([api.getDailySalesReport(days), api.getTopProducts(days, 10)]);
            setDailyReport(dr); setTopProducts(tp);
        } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    const totalSales = dailyReport.reduce((s, d) => s + d.total_sales, 0);
    const totalCount = dailyReport.reduce((s, d) => s + d.sale_count, 0);

    if (loading) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    return (
        <div className="p-10 space-y-8 animate-fade-in">
            <div className="flex items-center justify-between">
                <div>
                    <h1 className="text-2xl font-bold text-text-primary">Reportes</h1>
                    <p className="text-text-secondary text-sm mt-1">Análisis de ventas y productos</p>
                </div>
                <select value={days} onChange={(e) => setDays(Number(e.target.value))} className="px-5 py-4 bg-bg-secondary border border-border rounded-2xl text-text-primary text-sm">
                    <option value={7}>Últimos 7 días</option><option value={15}>Últimos 15 días</option><option value={30}>Últimos 30 días</option><option value={90}>Últimos 90 días</option>
                </select>
            </div>

            <div className="grid grid-cols-2 gap-6">
                <div className="glass rounded-2xl p-7 border border-success/20">
                    <p className="text-xs text-text-muted">Ventas Totales ({days} días)</p>
                    <p className="text-3xl font-bold text-success mt-2">{formatCurrency(totalSales)}</p>
                    <p className="text-xs text-text-muted mt-2">{totalCount} ventas</p>
                </div>
                <div className="glass rounded-2xl p-7 border border-primary/20">
                    <p className="text-xs text-text-muted">Promedio Diario</p>
                    <p className="text-3xl font-bold text-primary mt-2">{formatCurrency(dailyReport.length > 0 ? totalSales / dailyReport.length : 0)}</p>
                    <p className="text-xs text-text-muted mt-2">{dailyReport.length > 0 ? Math.round(totalCount / dailyReport.length) : 0} ventas/día</p>
                </div>
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
                {/* Daily Sales Table */}
                <div className="glass rounded-2xl border border-border p-8">
                    <h2 className="text-lg font-semibold text-text-primary mb-5 flex items-center gap-2"><BarChart3 size={20} className="text-primary" /> Ventas por Día</h2>
                    <div className="space-y-2 max-h-[450px] overflow-y-auto">
                        {dailyReport.map((d) => (
                            <div key={d.date} className="flex items-center justify-between px-5 py-3.5 bg-bg-primary/50 rounded-xl">
                                <span className="text-sm text-text-secondary">{d.date}</span>
                                <div className="text-right">
                                    <span className="text-sm font-medium text-text-primary">{formatCurrency(d.total_sales)}</span>
                                    <span className="text-xs text-text-muted ml-2">({d.sale_count})</span>
                                </div>
                            </div>
                        ))}
                        {dailyReport.length === 0 && <p className="text-text-muted text-sm text-center py-12">Sin datos</p>}
                    </div>
                </div>

                {/* Top Products */}
                <div className="glass rounded-2xl border border-border p-8">
                    <h2 className="text-lg font-semibold text-text-primary mb-5 flex items-center gap-2"><Package size={20} className="text-accent" /> Top 10 Productos</h2>
                    <div className="space-y-4">
                        {topProducts.map((p, idx) => {
                            const maxQty = topProducts[0]?.total_quantity || 1;
                            return (
                                <div key={p.product_id} className="flex items-center gap-4">
                                    <span className="w-8 h-8 rounded-full bg-primary/10 text-primary text-sm font-bold flex items-center justify-center shrink-0">{idx + 1}</span>
                                    <div className="flex-1">
                                        <div className="flex justify-between text-sm mb-1.5">
                                            <span className="text-text-primary truncate">{p.product_name}</span>
                                            <span className="text-success font-medium">{formatCurrency(p.total_revenue)}</span>
                                        </div>
                                        <div className="w-full bg-bg-primary rounded-full h-2">
                                            <div className="bg-primary h-2 rounded-full" style={{ width: `${(p.total_quantity / maxQty) * 100}%` }} />
                                        </div>
                                        <p className="text-xs text-text-muted mt-1">{p.total_quantity} unidades</p>
                                    </div>
                                </div>
                            );
                        })}
                        {topProducts.length === 0 && <p className="text-text-muted text-sm text-center py-12">Sin datos</p>}
                    </div>
                </div>
            </div>
        </div>
    );
}
