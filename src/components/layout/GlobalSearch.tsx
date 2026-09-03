import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import * as api from '../../api';
import { formatCurrency } from '../../utils';
import type { Product, Customer } from '../../types';

type PageItem = { label: string; sub: string; path: string; admin: boolean };

const PAGES: PageItem[] = [
    { label: 'Dashboard', sub: 'Vista general', path: '/dashboard', admin: false },
    { label: 'Punto de Venta', sub: 'Cobrar', path: '/pos', admin: false },
    { label: 'Caja', sub: 'Turno y cortes', path: '/cash-register', admin: false },
    { label: 'Apartados', sub: 'Apartados y abonos', path: '/layaways', admin: false },
    { label: 'Clientes', sub: 'Directorio', path: '/customers', admin: false },
    { label: 'Promociones', sub: 'Descuentos', path: '/discounts', admin: false },
    { label: 'Gastos', sub: 'Egresos', path: '/expenses', admin: false },
    { label: 'Inventario', sub: 'Stock y movimientos', path: '/inventory', admin: true },
    { label: 'Ventas', sub: 'Historial', path: '/sales', admin: true },
    { label: 'Reportes', sub: 'Métricas', path: '/reports', admin: true },
    { label: 'Productos', sub: 'Catálogo', path: '/products', admin: true },
    { label: 'Proveedores', sub: 'Gestión', path: '/suppliers', admin: true },
    { label: 'Usuarios', sub: 'Accesos', path: '/users', admin: true },
    { label: 'Configuración', sub: 'Ajustes', path: '/settings', admin: true },
];

export default function GlobalSearch({ isAdmin }: { isAdmin: boolean }) {
    const [open, setOpen] = useState(false);
    const [query, setQuery] = useState('');
    const [products, setProducts] = useState<Product[]>([]);
    const [customers, setCustomers] = useState<Customer[]>([]);
    const inputRef = useRef<HTMLInputElement>(null);
    const navigate = useNavigate();

    // Global ⌘K / Ctrl+K shortcut
    useEffect(() => {
        const onKey = (e: KeyboardEvent) => {
            if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); setOpen(true); }
            if (e.key === 'Escape') setOpen(false);
        };
        window.addEventListener('keydown', onKey);
        return () => window.removeEventListener('keydown', onKey);
    }, []);

    useEffect(() => { if (open) setTimeout(() => inputRef.current?.focus(), 40); else { setQuery(''); setProducts([]); setCustomers([]); } }, [open]);

    useEffect(() => {
        const q = query.trim();
        if (q.length < 2) { setProducts([]); setCustomers([]); return; }
        let cancelled = false;
        (async () => {
            try {
                const [prods, custs] = await Promise.all([
                    isAdmin ? api.getProducts({ search: q, is_active: true }) : Promise.resolve([]),
                    api.getCustomers().catch(() => []),
                ]);
                if (cancelled) return;
                setProducts(prods.slice(0, 6));
                const ql = q.toLowerCase();
                setCustomers(custs.filter(c => c.is_active && [c.name, c.phone || '', c.email || ''].some(v => v.toLowerCase().includes(ql))).slice(0, 5));
            } catch { /* silent */ }
        })();
        return () => { cancelled = true; };
    }, [query, isAdmin]);

    const go = (path: string) => { setOpen(false); navigate(path); };

    const pageMatches = PAGES.filter(p => (!p.admin || isAdmin) && (query.trim().length < 1 || `${p.label} ${p.sub}`.toLowerCase().includes(query.toLowerCase()))).slice(0, 6);

    return (
        <>
            <button className="topbar-search" onClick={() => setOpen(true)} style={{ cursor: 'pointer', textAlign: 'left', border: 'none' }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--t3)" strokeWidth="2.2" strokeLinecap="round">
                    <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
                </svg>
                <span style={{ fontSize: 12, color: 'var(--t3)', fontWeight: 500 }}>Buscar...</span>
                <span style={{ marginLeft: 'auto', fontSize: 10, color: 'var(--t3)', fontWeight: 600, padding: '1px 6px', borderRadius: 5, background: 'rgba(255,255,255,0.06)' }}>⌘K</span>
            </button>

            {open && (
                <div className="modal-overlay" style={{ alignItems: 'flex-start', paddingTop: '12vh' }} onClick={() => setOpen(false)}>
                    <div className="glass-modal animate-scale-in" style={{ width: '100%', maxWidth: 560, overflow: 'hidden' }} onClick={e => e.stopPropagation()}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '14px 18px', borderBottom: '1px solid rgba(255,255,255,0.08)' }}>
                            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--t3)" strokeWidth="2.2" strokeLinecap="round"><circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" /></svg>
                            <input ref={inputRef} value={query} onChange={e => setQuery(e.target.value)} placeholder="Buscar páginas, productos o clientes…"
                                style={{ flex: 1, background: 'none', border: 'none', outline: 'none', color: 'var(--t1)', fontSize: 15, fontFamily: 'inherit' }} />
                            <span style={{ fontSize: 10, color: 'var(--t3)', fontWeight: 600, padding: '2px 6px', borderRadius: 5, background: 'rgba(255,255,255,0.06)' }}>ESC</span>
                        </div>

                        <div style={{ maxHeight: '52vh', overflowY: 'auto', padding: 8 }}>
                            {pageMatches.length > 0 && <p className="gs-group">Ir a</p>}
                            {pageMatches.map(p => (
                                <button key={p.path} onClick={() => go(p.path)} className="gs-item">
                                    <span style={{ fontWeight: 600, color: 'var(--t1)', fontSize: 13 }}>{p.label}</span>
                                    <span style={{ fontSize: 11, color: 'var(--t3)', marginLeft: 'auto' }}>{p.sub}</span>
                                </button>
                            ))}

                            {products.length > 0 && <p className="gs-group">Productos</p>}
                            {products.map(pr => (
                                <button key={`p${pr.id}`} onClick={() => go(`/products?search=${encodeURIComponent(pr.sku)}`)} className="gs-item">
                                    <span style={{ fontWeight: 600, color: 'var(--t1)', fontSize: 13 }}>{pr.name}</span>
                                    <span style={{ fontSize: 11, color: 'var(--t3)', fontFamily: 'monospace' }}>{pr.sku}</span>
                                    <span style={{ fontSize: 12, color: 'var(--primary)', fontWeight: 700, marginLeft: 'auto' }}>{formatCurrency(pr.sale_price)}</span>
                                    <span style={{ fontSize: 11, color: pr.stock <= pr.min_stock ? 'var(--danger)' : 'var(--t3)' }}>{pr.stock} ud.</span>
                                </button>
                            ))}

                            {customers.length > 0 && <p className="gs-group">Clientes</p>}
                            {customers.map(c => (
                                <button key={`c${c.id}`} onClick={() => go(`/customers?search=${encodeURIComponent(c.name)}`)} className="gs-item">
                                    <span style={{ fontWeight: 600, color: 'var(--t1)', fontSize: 13 }}>{c.name}</span>
                                    <span style={{ fontSize: 11, color: 'var(--t3)', marginLeft: 'auto' }}>{c.phone || c.email || ''}</span>
                                </button>
                            ))}

                            {query.trim().length >= 2 && pageMatches.length === 0 && products.length === 0 && customers.length === 0 && (
                                <div style={{ padding: '28px 16px', textAlign: 'center', color: 'var(--t3)', fontSize: 13 }}>Sin resultados</div>
                            )}
                        </div>
                    </div>
                </div>
            )}
        </>
    );
}
