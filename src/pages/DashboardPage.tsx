import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { formatCurrency } from '../utils';
import * as api from '../api';
import type { DashboardStats, TopProduct, Product, DailySalesReport } from '../types';
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from 'recharts';
import { useToast } from '../contexts/ToastContext';

// ─── Design tokens (match global CSS vars) ───────────────────────────────────
const T = {
    primary: '#8B78F5', success: '#22D3A0', accent: '#F0C547',
    danger: '#F45270', warning: '#F5A842',
    t1: '#EDEFFA', t2: '#B4BBCE', t3: '#7580A0',
};

// ─── Gradient Avatar ──────────────────────────────────────────────────────────
const PALETTE: [string, string][] = [
    ['#8B78F5','#5D47C2'], ['#22D3A0','#15A078'], ['#F0C547','#C8A030'],
    ['#F45270','#B33356'], ['#38BDF8','#0B91D0'], ['#FB923C','#C45C18'],
];
function getGradient(name: string): [string, string] {
    let h = 0;
    for (let i = 0; i < name.length; i++) h = name.charCodeAt(i) + ((h << 5) - h);
    return PALETTE[Math.abs(h) % PALETTE.length];
}
function initials(name: string) {
    return (name || '?').split(' ').slice(0, 2).map(w => w[0]).join('').toUpperCase();
}

function GAvatar({ name, size = 36, r = 10 }: { name: string; size?: number; r?: number }) {
    const [a, b] = getGradient(name);
    return (
        <div style={{
            width: size, height: size, borderRadius: r, flexShrink: 0,
            background: `linear-gradient(135deg,${a},${b})`,
            display: 'grid', placeItems: 'center',
            color: '#fff', fontWeight: 800, fontSize: Math.round(size * 0.38),
            boxShadow: `0 4px 14px ${a}55`,
        }}>
            {initials(name)}
        </div>
    );
}

// ─── Sparkline ────────────────────────────────────────────────────────────────
function Sparkline({ data, color, w = 80, h = 38 }: { data: number[]; color: string; w?: number; h?: number }) {
    if (!data || data.length < 2) return null;
    const max = Math.max(...data), min = Math.min(...data), range = max - min || 1;
    const pts = data.map((v, i) => `${(i / (data.length - 1)) * w},${h - ((v - min) / range) * h}`).join(' ');
    const area = `M0,${h} L${data.map((v, i) => `${(i / (data.length - 1)) * w},${h - ((v - min) / range) * h}`).join(' L')} L${w},${h} Z`;
    const id = `sg${color.replace('#', '')}`;
    return (
        <svg width={w} height={h} style={{ overflow: 'visible' }}>
            <defs>
                <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor={color} stopOpacity="0.35" />
                    <stop offset="100%" stopColor={color} stopOpacity="0" />
                </linearGradient>
            </defs>
            <path d={area} fill={`url(#${id})`} />
            <polyline points={pts} fill="none" stroke={color} strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
    );
}

// ─── KPI Card ─────────────────────────────────────────────────────────────────
function KpiCard({ label, value, sub, icon, color, sparkData }: {
    label: string; value: string; sub: string; icon: React.ReactNode;
    color: string; sparkData?: number[];
}) {
    const [hov, setHov] = useState(false);
    return (
        <div
            onMouseEnter={() => setHov(true)} onMouseLeave={() => setHov(false)}
            style={{
                borderRadius: 20, padding: '22px 24px', position: 'relative', overflow: 'hidden',
                background: hov ? 'rgba(255,255,255,0.07)' : 'rgba(255,255,255,0.04)',
                border: `1px solid ${hov ? 'rgba(255,255,255,0.14)' : 'rgba(255,255,255,0.08)'}`,
                backdropFilter: 'blur(24px)', WebkitBackdropFilter: 'blur(24px)',
                boxShadow: hov ? '0 20px 60px rgba(0,0,0,0.4)' : '0 8px 32px rgba(0,0,0,0.2)',
                transform: hov ? 'translateY(-3px)' : 'none', transition: 'all 0.22s ease',
            }}
        >
            <div style={{ position: 'absolute', top: -40, right: -30, width: 120, height: 120, borderRadius: '50%', background: `radial-gradient(circle,${color}35 0%,transparent 70%)`, pointerEvents: 'none' }} />
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 16 }}>
                <div style={{ width: 42, height: 42, borderRadius: 13, display: 'grid', placeItems: 'center', background: `${color}18`, border: `1px solid ${color}30` }}>
                    {icon}
                </div>
                {sparkData && <div style={{ opacity: 0.75 }}><Sparkline data={sparkData} color={color} /></div>}
            </div>
            <p style={{ fontSize: 11, fontWeight: 700, color: T.t3, textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 6 }}>{label}</p>
            <p style={{ fontSize: 26, fontWeight: 900, color: T.t1, letterSpacing: '-0.03em', fontVariantNumeric: 'tabular-nums', lineHeight: 1, marginBottom: 6 }}>{value}</p>
            <p style={{ fontSize: 12, color, fontWeight: 600 }}>{sub}</p>
        </div>
    );
}

// ─── Chart Tooltip ────────────────────────────────────────────────────────────
function ChartTooltip({ active, payload, label }: any) {
    if (!active || !payload?.length) return null;
    return (
        <div style={{ background: 'rgba(18,12,38,0.95)', border: '1px solid rgba(255,255,255,0.12)', borderRadius: 14, padding: '12px 16px', boxShadow: '0 10px 30px rgba(0,0,0,0.5)', color: '#fff' }}>
            <p style={{ fontSize: 10, fontWeight: 700, color: T.t3, textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 8 }}>{label}</p>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                    <div style={{ width: 8, height: 8, borderRadius: 3, background: T.primary }} />
                    <span style={{ fontSize: 12, color: T.t2 }}>Ventas:</span>
                    <span style={{ fontSize: 13, fontWeight: 800, marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(payload[0]?.value || 0)}</span>
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                    <div style={{ width: 8, height: 8, borderRadius: 3, background: T.success }} />
                    <span style={{ fontSize: 12, color: T.t2 }}>Ganancia:</span>
                    <span style={{ fontSize: 13, fontWeight: 800, marginLeft: 'auto', fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(payload[1]?.payload?.gross_profit || 0)}</span>
                </div>
            </div>
        </div>
    );
}

// ─── Icons ────────────────────────────────────────────────────────────────────
const IcoDollar = (c: string) => <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke={c} strokeWidth="2.2" strokeLinecap="round"><line x1="12" y1="1" x2="12" y2="23"/><path d="M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6"/></svg>;
const IcoTrend = (c: string) => <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke={c} strokeWidth="2.2" strokeLinecap="round"><polyline points="23 6 13.5 15.5 8.5 10.5 1 18"/><polyline points="17 6 23 6 23 12"/></svg>;
const IcoCard = (c: string) => <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke={c} strokeWidth="2.2" strokeLinecap="round"><rect x="1" y="4" width="22" height="16" rx="2"/><line x1="1" y1="10" x2="23" y2="10"/></svg>;
const IcoBox = (c: string) => <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke={c} strokeWidth="2.2" strokeLinecap="round"><path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/></svg>;

// ─── Main Component ───────────────────────────────────────────────────────────
export default function DashboardPage() {
    const [stats, setStats] = useState<DashboardStats | null>(null);
    const [topProducts, setTopProducts] = useState<TopProduct[]>([]);
    const [lowStock, setLowStock] = useState<Product[]>([]);
    const [chartData, setChartData] = useState<(DailySalesReport & { label?: string })[]>([]);
    const [loading, setLoading] = useState(true);
    const [refreshing, setRefreshing] = useState(false);
    const { showToast } = useToast();

    useEffect(() => { loadData(); }, []);

    const loadData = async (isRefresh = false) => {
        if (isRefresh) setRefreshing(true);
        try {
            const [s, tp, ls, daily] = await Promise.all([
                api.getDashboardStats(), api.getTopProducts(30, 5),
                api.getLowStockProducts(), api.getDailySalesReport(14),
            ]);
            setStats(s); setTopProducts(tp); setLowStock(ls);
            setChartData([...daily].reverse().map(d => ({ ...d, label: d.date.slice(5).replace('-', '/') })));
            if (isRefresh) showToast('Panel sincronizado', 'success');
        } catch (err) { console.error(err); }
        finally { setLoading(false); setRefreshing(false); }
    };

    if (loading || !stats) {
        return (
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
                <div style={{ width: 36, height: 36, border: '3px solid rgba(139,120,245,0.2)', borderTopColor: 'var(--primary)', borderRadius: '50%', animation: 'spin 0.7s linear infinite' }} />
            </div>
        );
    }

    const now = new Date();
    const dateStr = now.toLocaleDateString('es-MX', { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' });

    const kpis = [
        { label: 'Ventas del Día', value: formatCurrency(stats.today_sales), sub: `${stats.today_count} transacciones hoy`, color: T.primary, sparkData: [5200, 7800, 6100, 9200, 11400, 8900, stats.today_sales], icon: IcoDollar(T.primary) },
        { label: 'Ventas del Mes', value: formatCurrency(stats.month_sales), sub: `${stats.month_count} transacciones`, color: T.success, sparkData: [140000, 155000, 148000, 162000, 170000, stats.month_sales, stats.month_sales], icon: IcoTrend(T.success) },
        { label: 'Ticket Promedio', value: formatCurrency(stats.today_count > 0 ? stats.today_sales / stats.today_count : 0), sub: 'Ingreso medio hoy', color: T.accent, icon: IcoCard(T.accent) },
        { label: 'Catálogo Activo', value: String(stats.total_products), sub: stats.low_stock_count > 0 ? `${stats.low_stock_count} alertas de stock` : 'Niveles al 100%', color: stats.low_stock_count > 0 ? T.danger : T.success, icon: IcoBox(stats.low_stock_count > 0 ? T.danger : T.success) },
    ];

    return (
        <div className="page-container animate-in">
            {/* Header */}
            <div className="page-header">
                <div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 4 }}>
                        <h1 className="page-title">Vista General</h1>
                        <span className="badge badge-success">
                            <span style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--success)', display: 'inline-block', animation: 'pulse 2s infinite' }} />
                            En línea
                        </span>
                    </div>
                    <p className="page-subtitle" style={{ textTransform: 'capitalize' }}>{dateStr}</p>
                </div>
                <button
                    onClick={() => loadData(true)} disabled={refreshing}
                    className="btn btn-ghost"
                >
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" style={{ animation: refreshing ? 'spin 0.7s linear infinite' : 'none' }}>
                        <polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/>
                        <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/>
                    </svg>
                    Sincronizar
                </button>
            </div>

            {/* KPI Grid */}
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 16 }}>
                {kpis.map(k => <KpiCard key={k.label} {...k} />)}
            </div>

            {/* Top ventas + Stock crítico */}
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
                {/* Top ventas */}
                <div className="glass" style={{ padding: 28 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 20 }}>
                        <div style={{ width: 38, height: 38, borderRadius: 11, background: 'rgba(139,120,245,0.15)', border: '1px solid rgba(139,120,245,0.25)', display: 'grid', placeItems: 'center', fontSize: 16 }}>🏆</div>
                        <div>
                            <h3 style={{ fontSize: 15, fontWeight: 800, color: T.t1 }}>Top Ventas</h3>
                            <p style={{ fontSize: 11, color: T.primary, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em' }}>Últimos 30 días</p>
                        </div>
                    </div>
                    {topProducts.length === 0 ? (
                        <p style={{ fontSize: 12, color: T.t3, textAlign: 'center', padding: '20px 0' }}>Sin ventas registradas este mes.</p>
                    ) : topProducts.map((p, i) => (
                        <div key={p.product_id} style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 0', borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
                            <span style={{ fontSize: 11, fontWeight: 800, color: T.t3, width: 20, textAlign: 'center' }}>#{i + 1}</span>
                            <GAvatar name={p.product_name} size={36} r={10} />
                            <div style={{ flex: 1, minWidth: 0 }}>
                                <p style={{ fontSize: 13, fontWeight: 700, color: T.t1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{p.product_name}</p>
                                <p style={{ fontSize: 11, color: T.t3, fontWeight: 600 }}>{p.total_quantity} unidades</p>
                            </div>
                            <span style={{ fontSize: 13, fontWeight: 900, color: T.accent, fontVariantNumeric: 'tabular-nums' }}>{formatCurrency(p.total_revenue)}</span>
                        </div>
                    ))}
                </div>

                {/* Stock crítico */}
                <div className="glass" style={{ padding: 28 }}>
                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                            <div style={{ width: 38, height: 38, borderRadius: 11, background: 'rgba(244,82,112,0.12)', border: '1px solid rgba(244,82,112,0.22)', display: 'grid', placeItems: 'center', fontSize: 16 }}>⚠️</div>
                            <div>
                                <h3 style={{ fontSize: 15, fontWeight: 800, color: T.t1 }}>Stock Crítico</h3>
                                <p style={{ fontSize: 11, color: T.danger, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.06em' }}>Atención requerida</p>
                            </div>
                        </div>
                        {lowStock.length > 0 && <span className="badge badge-danger">{lowStock.length}</span>}
                    </div>
                    {lowStock.length === 0 ? (
                        <div style={{ textAlign: 'center', padding: '20px 0' }}>
                            <p style={{ fontSize: 13, color: T.success, fontWeight: 700 }}>✓ Stock en niveles óptimos</p>
                        </div>
                    ) : lowStock.slice(0, 4).map(p => {
                        const out = p.stock <= 0;
                        const pct = Math.min(100, (p.stock / Math.max(p.min_stock, 1)) * 100);
                        const col = out ? T.danger : T.warning;
                        return (
                            <div key={p.id} style={{ padding: '12px 0', borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
                                <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 8 }}>
                                    <GAvatar name={p.name} size={32} r={8} />
                                    <div style={{ flex: 1, minWidth: 0 }}>
                                        <p style={{ fontSize: 13, fontWeight: 700, color: T.t1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{p.name}</p>
                                        <p style={{ fontSize: 10, color: T.t3, fontWeight: 600, textTransform: 'uppercase' }}>Mín: {p.min_stock} uds</p>
                                    </div>
                                    <span style={{ fontSize: 20, fontWeight: 900, color: col, fontVariantNumeric: 'tabular-nums' }}>{p.stock}</span>
                                </div>
                                <div style={{ height: 3, borderRadius: 2, background: 'rgba(255,255,255,0.08)', overflow: 'hidden' }}>
                                    <div style={{ height: '100%', width: `${pct}%`, background: col, borderRadius: 2, transition: 'width 0.4s ease' }} />
                                </div>
                            </div>
                        );
                    })}
                    {lowStock.length > 4 && (
                        <Link to="/inventory" style={{ display: 'block', marginTop: 12, textAlign: 'center', fontSize: 12, color: T.primary, fontWeight: 600 }}>
                            Ver {lowStock.length - 4} más →
                        </Link>
                    )}
                </div>
            </div>

            {/* Chart */}
            <div className="glass" style={{ padding: 28 }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
                    <div>
                        <h3 style={{ fontSize: 16, fontWeight: 800, color: T.t1 }}>Rendimiento — Últimas 2 semanas</h3>
                        <p style={{ fontSize: 12, color: T.t3, marginTop: 3 }}>Ventas vs. Ganancia neta</p>
                    </div>
                    <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                            <div style={{ width: 10, height: 10, borderRadius: 3, background: T.primary }} />
                            <span style={{ fontSize: 11, color: T.t3, fontWeight: 600 }}>Ventas</span>
                        </div>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                            <div style={{ width: 10, height: 10, borderRadius: 3, background: T.success }} />
                            <span style={{ fontSize: 11, color: T.t3, fontWeight: 600 }}>Ganancia</span>
                        </div>
                    </div>
                </div>
                <div style={{ height: 220 }}>
                    {chartData.length === 0 ? (
                        <div style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                            <p style={{ fontSize: 13, color: T.t3 }}>Sin datos disponibles aún</p>
                        </div>
                    ) : (
                        <ResponsiveContainer width="100%" height="100%">
                            <AreaChart data={chartData} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                                <defs>
                                    <linearGradient id="gSales" x1="0" y1="0" x2="0" y2="1">
                                        <stop offset="5%" stopColor={T.primary} stopOpacity={0.35} />
                                        <stop offset="95%" stopColor={T.primary} stopOpacity={0} />
                                    </linearGradient>
                                    <linearGradient id="gProfit" x1="0" y1="0" x2="0" y2="1">
                                        <stop offset="5%" stopColor={T.success} stopOpacity={0.25} />
                                        <stop offset="95%" stopColor={T.success} stopOpacity={0} />
                                    </linearGradient>
                                </defs>
                                <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="rgba(255,255,255,0.05)" />
                                <XAxis dataKey="label" axisLine={false} tickLine={false} tick={{ fontSize: 10, fontWeight: 700, fill: T.t3 }} dy={12} />
                                <YAxis axisLine={false} tickLine={false} tickFormatter={v => `$${(v / 1000).toFixed(0)}k`} tick={{ fontSize: 10, fontWeight: 700, fill: T.t3 }} dx={-8} />
                                <Tooltip content={<ChartTooltip />} cursor={{ fill: 'rgba(255,255,255,0.02)' }} />
                                <Area type="monotone" dataKey="total_sales" stroke={T.primary} strokeWidth={2.5} fillOpacity={1} fill="url(#gSales)" activeDot={{ r: 5, fill: 'var(--bg)', stroke: T.primary, strokeWidth: 2 }} />
                                <Area type="monotone" dataKey="gross_profit" stroke={T.success} strokeWidth={2} fillOpacity={1} fill="url(#gProfit)" activeDot={{ r: 4 }} />
                            </AreaChart>
                        </ResponsiveContainer>
                    )}
                </div>
            </div>
        </div>
    );
}
