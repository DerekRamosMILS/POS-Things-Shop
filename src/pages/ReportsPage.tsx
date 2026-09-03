import { useEffect, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { formatCurrency } from '../utils';
import * as api from '../api';
import type { DailySalesReport, TopProduct, CashierReport } from '../types';
import { useToast } from '../contexts/ToastContext';

// ─── Inline SVGs ─────────────────────────────────────────────────────────────
const IcoDownload = () => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>;

export default function ReportsPage() {
    const [dailyReport, setDailyReport] = useState<DailySalesReport[]>([]);
    const [topProducts, setTopProducts] = useState<TopProduct[]>([]);
    const [cashierReport, setCashierReport] = useState<CashierReport[]>([]);
    const [days, setDays] = useState(30);
    const [loading, setLoading] = useState(true);
    const { showToast } = useToast();

    useEffect(() => { loadData(); }, [days]);

    /// Exporta las ventas marcadas como facturables que aún no tienen folio
    /// fiscal, en el formato que un PAC o el contador puede procesar.
    const exportarFacturas = async () => {
        try {
            const desde = new Date(Date.now() - days * 86400000).toISOString().slice(0, 10);
            const target = await save({
                title: 'Exportar ventas por facturar',
                defaultPath: `por-facturar-${desde}.csv`,
                filters: [{ name: 'CSV', extensions: ['csv'] }],
            });
            if (!target) return;
            const count = await api.exportarPendientesFactura(target, desde);
            showToast(count === 0
                ? 'No hay ventas pendientes de facturar en el periodo'
                : `${count} venta(s) exportadas`);
        } catch (err) { showToast(String(err), 'error'); }
    };

    const exportCSV = () => {
        try {
            const header = ['Fecha', 'Ventas', 'Transacciones', 'Ganancia Bruta', 'Gastos'];
            const rows = dailyReport.map(d => [d.date, d.total_sales.toFixed(2), d.sale_count.toString(), d.gross_profit.toFixed(2), d.total_expenses.toFixed(2)]);
            const csv = [header, ...rows].map(r => r.join(',')).join('\n');
            const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url; a.download = `reporte_ventas_${days}d_${new Date().toISOString().slice(0, 10)}.csv`; a.click();
            URL.revokeObjectURL(url);
            showToast('Reporte exportado exitosamente');
        } catch { showToast('Error al exportar reporte', 'error'); }
    };

    const loadData = async () => {
        setLoading(true);
        try {
            const [dr, tp, cr] = await Promise.all([api.getDailySalesReport(days), api.getTopProducts(days, 10), api.getCashierReport(days)]);
            setDailyReport(dr); setTopProducts(tp); setCashierReport(cr);
        } catch (err) { showToast(String(err), 'error'); } finally { setLoading(false); }
    };

    const totalSales = dailyReport.reduce((s, d) => s + d.total_sales, 0);
    const totalCount = dailyReport.reduce((s, d) => s + d.sale_count, 0);
    const totalProfit = dailyReport.reduce((s, d) => s + d.gross_profit, 0);
    const totalExpenses = dailyReport.reduce((s, d) => s + d.total_expenses, 0);
    const netProfit = totalProfit - totalExpenses;
    const avgDaily = dailyReport.length > 0 ? totalSales / dailyReport.length : 0;
    const avgCount = dailyReport.length > 0 ? Math.round(totalCount / dailyReport.length) : 0;
    const margin = totalSales > 0 ? (netProfit / totalSales) * 100 : 0;

    if (loading) return (
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--primary)" strokeWidth="2.5" strokeLinecap="round" className="animate-spin"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
        </div>
    );

    return (
        <div className="page-container">
            {/* Header */}
            <div className="page-header">
                <div>
                    <h1 className="page-title">Reportes</h1>
                    <p className="page-subtitle">Análisis de ventas y productos · {days} días</p>
                </div>
                <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
                    <select value={days} onChange={e => setDays(Number(e.target.value))} className="input" style={{ width: 'auto' }}>
                        <option value={7}>Últimos 7 días</option>
                        <option value={15}>Últimos 15 días</option>
                        <option value={30}>Últimos 30 días</option>
                        <option value={90}>Últimos 90 días</option>
                    </select>
                    <button onClick={exportCSV} disabled={dailyReport.length === 0} className="btn btn-ghost" style={{ gap: 7 }}>
                        <IcoDownload /> Exportar CSV
                    </button>
                    <button onClick={exportarFacturas} className="btn btn-ghost" style={{ gap: 7 }}>
                        <IcoDownload /> Por facturar
                    </button>
                </div>
            </div>

            {/* KPI row */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 14 }}>
                {[
                    { label: 'Ventas Totales', value: formatCurrency(totalSales), sub: `${days} días`, color: 'var(--success)' },
                    { label: 'Transacciones', value: totalCount.toString(), sub: 'total', color: 'var(--primary)' },
                    { label: 'Utilidad Neta', value: formatCurrency(netProfit), sub: `${margin.toFixed(1)}% margen`, color: netProfit >= 0 ? 'var(--accent)' : 'var(--danger)' },
                    { label: 'Promedio Diario', value: formatCurrency(avgDaily), sub: `${avgCount} tickets/día`, color: 'var(--accent)' },
                ].map(card => (
                    <div key={card.label} className="card" style={{ padding: '20px 22px' }}>
                        <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>{card.label}</p>
                        <p style={{ fontSize: 20, fontWeight: 900, color: card.color, fontVariantNumeric: 'tabular-nums', letterSpacing: '-0.02em' }}>{card.value}</p>
                        <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 4 }}>{card.sub}</p>
                    </div>
                ))}
            </div>

            {/* Second KPI row */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 14 }}>
                {[
                    { label: 'Utilidad Bruta', value: formatCurrency(totalProfit), color: 'var(--success)' },
                    { label: 'Gastos del Periodo', value: formatCurrency(totalExpenses), color: 'var(--danger)' },
                    { label: 'Top Productos', value: topProducts.length.toString(), color: 'var(--accent)' },
                ].map(k => (
                    <div key={k.label} className="card" style={{ padding: '20px 22px' }}>
                        <p style={{ fontSize: 11, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em', color: 'var(--t3)', marginBottom: 8 }}>{k.label}</p>
                        <p style={{ fontSize: 26, fontWeight: 900, color: k.color, fontVariantNumeric: 'tabular-nums' }}>{k.value}</p>
                    </div>
                ))}
            </div>

            {/* Two-panel */}
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
                {/* Daily sales */}
                <div className="card" style={{ padding: '20px 0 0 0', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                    <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', padding: '0 20px 14px' }}>Ventas por Día</p>
                    <div style={{ overflowY: 'auto', flex: 1, maxHeight: 420 }}>
                        {dailyReport.length === 0 ? (
                            <p style={{ textAlign: 'center', padding: '40px 0', fontSize: 13, color: 'var(--t3)' }}>Sin datos en el periodo</p>
                        ) : (
                            dailyReport.map(d => (
                                <div key={d.date} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '10px 20px', borderRadius: 0, transition: 'background 0.12s' }}
                                    onMouseEnter={e => (e.currentTarget as HTMLDivElement).style.background = 'rgba(255,255,255,0.03)'}
                                    onMouseLeave={e => (e.currentTarget as HTMLDivElement).style.background = 'transparent'}
                                >
                                    <span style={{ fontSize: 12, color: 'var(--t3)', fontFamily: 'monospace' }}>{d.date}</span>
                                    <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                                        <span className="badge badge-muted">{d.sale_count}</span>
                                        <span style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(d.total_sales)}</span>
                                        <span style={{ fontSize: 11, color: 'var(--success)', fontVariantNumeric: 'tabular-nums', minWidth: 60, textAlign: 'right' }}>{formatCurrency(d.gross_profit)}</span>
                                    </div>
                                </div>
                            ))
                        )}
                    </div>
                </div>

                {/* Top products */}
                <div className="card" style={{ padding: '20px 0 0 0', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                    <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', padding: '0 20px 14px' }}>Top 10 Productos</p>
                    <div style={{ overflowY: 'auto', flex: 1, maxHeight: 420, padding: '0 20px 20px' }}>
                        {topProducts.length === 0 ? (
                            <p style={{ textAlign: 'center', padding: '40px 0', fontSize: 13, color: 'var(--t3)' }}>Sin datos en el periodo</p>
                        ) : (
                            <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                                {topProducts.map((p, idx) => {
                                    const maxQty = topProducts[0]?.total_quantity || 1;
                                    return (
                                        <div key={p.product_id} style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                                            <span style={{ width: 24, height: 24, borderRadius: 7, background: 'rgba(240,197,71,0.12)', color: 'var(--accent)', display: 'grid', placeItems: 'center', fontSize: 11, fontWeight: 800, flexShrink: 0 }}>
                                                {idx + 1}
                                            </span>
                                            <div style={{ flex: 1, minWidth: 0 }}>
                                                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 6 }}>
                                                    <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--t1)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{p.product_name}</span>
                                                    <span style={{ fontSize: 12, fontWeight: 700, color: 'var(--success)', fontVariantNumeric: 'tabular-nums', marginLeft: 8, flexShrink: 0 }}>{formatCurrency(p.total_revenue)}</span>
                                                </div>
                                                <div style={{ width: '100%', height: 4, borderRadius: 100, background: 'rgba(255,255,255,0.07)' }}>
                                                    <div style={{ height: '100%', borderRadius: 100, background: 'linear-gradient(90deg, var(--primary), var(--accent))', width: `${(p.total_quantity / maxQty) * 100}%` }} />
                                                </div>
                                                <p style={{ fontSize: 11, color: 'var(--t3)', marginTop: 4 }}>{p.total_quantity} unidades</p>
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        )}
                    </div>
                </div>
            </div>

            {/* Ventas por cajero */}
            <div className="card" style={{ padding: '20px 0 0 0', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
                <p style={{ fontSize: 13, fontWeight: 700, color: 'var(--t1)', padding: '0 20px 14px' }}>Ventas por Cajero</p>
                {cashierReport.length === 0 ? (
                    <p style={{ textAlign: 'center', padding: '32px 0', fontSize: 13, color: 'var(--t3)' }}>Sin datos en el periodo</p>
                ) : (
                    <div style={{ overflowX: 'auto' }}>
                        <table className="table-base">
                            <thead>
                                <tr>
                                    <th>Cajero</th>
                                    <th style={{ textAlign: 'right' }}>Tickets</th>
                                    <th style={{ textAlign: 'right' }}>Ventas</th>
                                    <th style={{ textAlign: 'right' }}>Ticket prom.</th>
                                    <th style={{ textAlign: 'right' }}>Utilidad bruta</th>
                                </tr>
                            </thead>
                            <tbody>
                                {cashierReport.map(c => (
                                    <tr key={c.user_id}>
                                        <td style={{ fontWeight: 600, color: 'var(--t1)', fontSize: 13 }}>{c.user_name}</td>
                                        <td style={{ textAlign: 'right', fontSize: 13, color: 'var(--t2)', fontVariantNumeric: 'tabular-nums' }}>{c.sale_count}</td>
                                        <td style={{ textAlign: 'right', fontSize: 13, fontWeight: 700, color: 'var(--success)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(c.total_sales)}</td>
                                        <td style={{ textAlign: 'right', fontSize: 13, color: 'var(--t2)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(c.sale_count > 0 ? c.total_sales / c.sale_count : 0)}</td>
                                        <td style={{ textAlign: 'right', fontSize: 13, color: 'var(--accent)', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(c.gross_profit)}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                )}
            </div>
        </div>
    );
}
