import { useEffect, useState } from 'react';
import { formatCurrency } from '../utils';
import * as api from '../api';
import type { DashboardStats, TopProduct, Product } from '../types';
import {
    DollarSign,
    ShoppingCart,
    TrendingUp,
    Package,
    AlertTriangle,
    ArrowUp,
} from 'lucide-react';

export default function DashboardPage() {
    const [stats, setStats] = useState<DashboardStats | null>(null);
    const [topProducts, setTopProducts] = useState<TopProduct[]>([]);
    const [lowStock, setLowStock] = useState<Product[]>([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => { loadData(); }, []);

    const loadData = async () => {
        try {
            const [s, tp, ls] = await Promise.all([
                api.getDashboardStats(),
                api.getTopProducts(30, 5),
                api.getLowStockProducts(),
            ]);
            setStats(s); setTopProducts(tp); setLowStock(ls);
        } catch (err) { console.error(err); } finally { setLoading(false); }
    };

    if (loading || !stats) return <div className="flex items-center justify-center h-full"><div className="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin" /></div>;

    const statCards = [
        { title: 'Ventas Hoy', value: formatCurrency(stats.today_sales), sub: `${stats.today_count} ventas`, icon: DollarSign, color: 'text-success', bg: 'bg-success/10', border: 'border-success/20' },
        { title: 'Ventas del Mes', value: formatCurrency(stats.month_sales), sub: `${stats.month_count} ventas`, icon: TrendingUp, color: 'text-primary', bg: 'bg-primary/10', border: 'border-primary/20' },
        { title: 'Ganancia Hoy', value: formatCurrency(stats.today_profit), sub: 'Ganancia bruta', icon: ArrowUp, color: 'text-accent', bg: 'bg-accent/10', border: 'border-accent/20' },
        { title: 'Productos Activos', value: stats.total_products.toString(), sub: `${stats.low_stock_count} con stock bajo`, icon: Package, color: 'text-warning', bg: 'bg-warning/10', border: 'border-warning/20' },
    ];

    return (
        <div className="p-10 lg:p-14 xl:p-16 h-full flex flex-col animate-fade-in">
            <div className="mb-10">
                <h1 className="text-4xl font-bold text-text-primary">Dashboard</h1>
                <p className="text-text-secondary text-lg mt-2">Resumen general de Things Shop</p>
            </div>

            {/* Stat Cards */}
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-8 mb-10">
                {statCards.map((card) => (
                    <div key={card.title} className={`glass rounded-[20px] p-8 lg:p-10 border ${card.border} animate-fade-in`}>
                        <div className="flex items-start justify-between">
                            <div>
                                <p className="text-base text-text-muted mb-4">{card.title}</p>
                                <p className={`text-4xl font-bold tracking-tight ${card.color}`}>{card.value}</p>
                                <p className="text-base text-text-muted mt-4">{card.sub}</p>
                            </div>
                            <div className={`p-4 lg:p-5 rounded-2xl ${card.bg}`}>
                                <card.icon size={32} className={card.color} strokeWidth={2.5} />
                            </div>
                        </div>
                    </div>
                ))}
            </div>

            <div className="grid grid-cols-1 lg:grid-cols-2 gap-8 flex-1 min-h-0">
                {/* Top Products */}
                <div className="glass rounded-[24px] border border-border p-8 lg:p-10 flex flex-col">
                    <h2 className="text-xl font-semibold text-text-primary mb-8 flex items-center gap-4">
                        <ShoppingCart size={26} className="text-primary" />
                        Productos Más Vendidos
                    </h2>
                    {topProducts.length === 0 ? (
                        <p className="text-text-muted text-lg text-center py-12 flex-1 flex items-center justify-center">Sin datos aún</p>
                    ) : (
                        <div className="space-y-5 flex-1 p-2 overflow-y-auto">
                            {topProducts.map((p, idx) => (
                                <div key={p.product_id} className="flex items-center gap-5 p-5 bg-bg-primary rounded-2xl border border-white/5">
                                    <span className="w-12 h-12 rounded-xl bg-primary/10 text-primary text-lg font-bold flex items-center justify-center shrink-0">{idx + 1}</span>
                                    <div className="flex-1 min-w-0">
                                        <p className="text-lg text-text-primary truncate">{p.product_name}</p>
                                        <p className="text-base text-text-muted mt-0.5">{p.total_quantity} vendidos</p>
                                    </div>
                                    <p className="text-xl font-bold tracking-tight text-success">{formatCurrency(p.total_revenue)}</p>
                                </div>
                            ))}
                        </div>
                    )}
                </div>

                {/* Low Stock Alerts */}
                <div className="glass rounded-[24px] border border-border p-8 lg:p-10 flex flex-col">
                    <h2 className="text-xl font-semibold text-text-primary mb-8 flex items-center gap-4">
                        <AlertTriangle size={26} className="text-warning" />
                        Alertas de Stock Bajo
                    </h2>
                    {lowStock.length === 0 ? (
                        <p className="text-text-muted text-lg text-center py-12 flex-1 flex items-center justify-center">Sin alertas</p>
                    ) : (
                        <div className="space-y-5 flex-1 p-2 overflow-y-auto">
                            {lowStock.slice(0, 8).map((p) => (
                                <div key={p.id} className="flex items-center justify-between px-6 py-5 rounded-2xl bg-bg-primary border border-white/5">
                                    <div className="flex-1 min-w-0">
                                        <p className="text-lg text-text-primary truncate">{p.name}</p>
                                        <p className="text-base text-text-muted mt-0.5">Mínimo: {p.min_stock}</p>
                                    </div>
                                    <span className={`text-lg font-bold px-5 py-2.5 rounded-xl ${p.stock === 0 ? 'text-danger bg-danger/10' : 'text-warning bg-warning/10'}`}>
                                        {p.stock} ud.
                                    </span>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
