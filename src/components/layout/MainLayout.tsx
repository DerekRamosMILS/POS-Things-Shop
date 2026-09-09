import { useCallback, useEffect, useRef, useState } from 'react';
import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom';
import { useSessionStore } from '../../stores/useSessionStore';
import * as api from '../../api';
import NotificationBell from './NotificationBell';
import GlobalSearch from './GlobalSearch';
const IcoLogOut = () => <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>;
const IcoAlertCircle = () => <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>;

// ─── Nav icons (inline SVG for crisp rendering) ─────────────────────────────

const IconDashboard = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/>
    <rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/>
  </svg>
);
const IconPOS = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="9" cy="21" r="1"/><circle cx="20" cy="21" r="1"/>
    <path d="M1 1h4l2.68 13.39a2 2 0 002 1.61h9.72a2 2 0 001.99-1.73L23 6H6"/>
  </svg>
);
const IconInventory = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/>
  </svg>
);
const IconSales = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>
  </svg>
);
const IconReports = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/>
  </svg>
);
const IconSettings = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="3"/>
    <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/>
  </svg>
);
const IconCash = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="1" y="4" width="22" height="16" rx="2"/>
    <line x1="1" y1="10" x2="23" y2="10"/>
  </svg>
);
const IconTag = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M20.59 13.41l-7.17 7.17a2 2 0 01-2.83 0L2 12V2h10l8.59 8.59a2 2 0 010 2.82z"/>
    <line x1="7" y1="7" x2="7.01" y2="7"/>
  </svg>
);
const IconUsers = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2"/>
    <circle cx="9" cy="7" r="4"/>
    <path d="M23 21v-2a4 4 0 00-3-3.87"/><path d="M16 3.13a4 4 0 010 7.75"/>
  </svg>
);
const IconSupplier = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="1" y="3" width="15" height="13"/><polygon points="16 8 20 8 23 11 23 16 16 16 16 8"/>
    <circle cx="5.5" cy="18.5" r="2.5"/><circle cx="18.5" cy="18.5" r="2.5"/>
  </svg>
);
const IconProducts = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M6 2L3 6v14a2 2 0 002 2h14a2 2 0 002-2V6l-3-4z"/>
    <line x1="3" y1="6" x2="21" y2="6"/>
    <path d="M16 10a4 4 0 01-8 0"/>
  </svg>
);
const IconExpense = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <line x1="12" y1="1" x2="12" y2="23"/>
    <path d="M17 5H9.5a3.5 3.5 0 000 7h5a3.5 3.5 0 010 7H6"/>
  </svg>
);
const IconStore = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2.3" strokeLinecap="round" strokeLinejoin="round">
    <path d="M6 2L3 6v14a2 2 0 002 2h14a2 2 0 002-2V6l-3-4z"/>
    <line x1="3" y1="6" x2="21" y2="6"/>
    <path d="M16 10a4 4 0 01-8 0"/>
  </svg>
);
const IconCustomer = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2"/><circle cx="12" cy="7" r="4"/>
  </svg>
);
const IconLayaway = () => (
  <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M19 21l-7-5-7 5V5a2 2 0 012-2h10a2 2 0 012 2z"/>
  </svg>
);

// ─── Topbar titles by route ──────────────────────────────────────────────────
const PAGE_META: Record<string, { title: string; sub: string }> = {
  '/dashboard':     { title: 'Vista General',    sub: 'ThingsShop POS' },
  '/pos':           { title: 'Punto de Venta',   sub: 'Crear transacción · F10 cobrar' },
  '/inventory':     { title: 'Inventario',        sub: 'Control de stock y movimientos' },
  '/sales':         { title: 'Ventas',            sub: 'Historial de transacciones' },
  '/products':      { title: 'Productos',         sub: 'Catálogo del negocio' },
  '/cash-register': { title: 'Caja',              sub: 'Apertura y cierre de turno' },
  '/layaways':      { title: 'Apartados',         sub: 'Apartados y abonos' },
  '/customers':     { title: 'Clientes',          sub: 'Directorio de clientes' },
  '/expenses':      { title: 'Gastos',            sub: 'Registro de egresos' },
  '/discounts':     { title: 'Promociones',       sub: 'Códigos y descuentos' },
  '/suppliers':     { title: 'Proveedores',       sub: 'Gestión de proveedores' },
  '/users':         { title: 'Usuarios',          sub: 'Accesos y roles' },
  '/reports':       { title: 'Reportes',          sub: 'Análisis y métricas' },
  '/settings':      { title: 'Configuración',     sub: 'Ajustes del sistema' },
};

export default function MainLayout() {
  const { user, cashRegisterId, setCashRegisterId, logout } = useSessionStore();
  const navigate = useNavigate();
  const location = useLocation();

  // Qué turno está abierto lo sabe la base, no la sesión de quien entra.
  //
  // Antes solo se averiguaba al visitar la pantalla de Caja. Como salir borra el
  // id, la persona del siguiente turno entraba, caía en el punto de venta y se
  // encontraba el botón de cobrar apagado con un "la caja no está abierta" que
  // era falso: la caja seguía abierta, y al intentar abrirla de nuevo el sistema
  // contestaba que ya lo estaba. Se quedaba encerrada sin poder vender hasta que
  // alguien atinaba a entrar a Caja, que lo arreglaba de rebote.
  useEffect(() => {
    if (!user) return;
    let cancelado = false;
    api.getOpenRegister()
      .then((reg) => { if (!cancelado) setCashRegisterId(reg?.id ?? null); })
      .catch(() => { /* se reintenta al entrar a Caja */ });
    return () => { cancelado = true; };
  }, [user?.id, setCashRegisterId]); // eslint-disable-line react-hooks/exhaustive-deps

  // The nav is taller than the sidebar on short screens; the fade only shows
  // while there is still something below the fold.
  const navRef = useRef<HTMLElement>(null);
  const [navOverflowing, setNavOverflowing] = useState(false);

  const updateNavOverflow = useCallback(() => {
    const el = navRef.current;
    if (!el) return;
    setNavOverflowing(el.scrollHeight - el.scrollTop - el.clientHeight > 4);
  }, []);

  useEffect(() => {
    updateNavOverflow();
    window.addEventListener('resize', updateNavOverflow);
    return () => window.removeEventListener('resize', updateNavOverflow);
  }, [updateNavOverflow, user?.role]);

  const handleLogout = async () => {
    // Revoke server-side first so the token cannot be replayed.
    try { await api.logout(); } catch { /* logging out locally regardless */ }
    // `logout` se encarga de vaciar el carrito y las órdenes en espera: hay más
    // de una salida y todas pasan por ahí.
    logout();
    navigate('/login');
  };

  const isPOS = location.pathname === '/pos';
  const meta = PAGE_META[location.pathname] ?? { title: 'ThingsShop', sub: '' };
  const isAdmin = user?.role === 'admin';

  return (
    <div className="app-shell">
      {/* Background blobs */}
      <div className="blobs" />

      {/* ── Sidebar ── */}
      <aside className="sidebar">
        {/* Logo */}
        <div className="sidebar-logo">
          <div className="sidebar-logo-inner">
            <div className="sidebar-logo-mark">
              <IconStore />
            </div>
            <div>
              <p style={{ fontSize: 14, fontWeight: 800, color: 'var(--t1)', letterSpacing: '-0.02em', lineHeight: 1 }}>
                ThingsShop
              </p>
              <p style={{ fontSize: 10, color: 'var(--t3)', fontWeight: 600, marginTop: 2 }}>POS v2.0</p>
            </div>
          </div>
        </div>

        {/* Nav */}
        <div className="sidebar-nav-wrap" data-overflowing={navOverflowing}>
        <nav className="sidebar-nav" ref={navRef} onScroll={updateNavOverflow}>
          <NavLink to="/dashboard" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
            <span className="nav-icon"><IconDashboard /></span>Dashboard
          </NavLink>
          <NavLink to="/pos" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
            <span className="nav-icon"><IconPOS /></span>Punto de Venta
          </NavLink>
          {isAdmin && (
            <NavLink to="/inventory" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
              <span className="nav-icon"><IconInventory /></span>Inventario
            </NavLink>
          )}
          {isAdmin && (
            <NavLink to="/sales" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
              <span className="nav-icon"><IconSales /></span>Ventas
            </NavLink>
          )}
          {isAdmin && (
            <NavLink to="/reports" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
              <span className="nav-icon"><IconReports /></span>Reportes
            </NavLink>
          )}
          <NavLink to="/cash-register" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
            <span className="nav-icon"><IconCash /></span>Caja
          </NavLink>
          <NavLink to="/layaways" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
            <span className="nav-icon"><IconLayaway /></span>Apartados
          </NavLink>
          <NavLink to="/customers" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
            <span className="nav-icon"><IconCustomer /></span>Clientes
          </NavLink>
          <NavLink to="/discounts" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
            <span className="nav-icon"><IconTag /></span>Promociones
          </NavLink>
          <NavLink to="/expenses" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
            <span className="nav-icon"><IconExpense /></span>Gastos
          </NavLink>
          {isAdmin && (
            <NavLink to="/products" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
              <span className="nav-icon"><IconProducts /></span>Productos
            </NavLink>
          )}
          {isAdmin && (
            <NavLink to="/suppliers" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
              <span className="nav-icon"><IconSupplier /></span>Proveedores
            </NavLink>
          )}
          {isAdmin && (
            <NavLink to="/users" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
              <span className="nav-icon"><IconUsers /></span>Usuarios
            </NavLink>
          )}
          {isAdmin && (
            <NavLink to="/settings" className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}>
              <span className="nav-icon"><IconSettings /></span>Configuración
            </NavLink>
          )}
        </nav>
        </div>

        {/* Footer */}
        <div className="sidebar-footer">
          {!cashRegisterId && (
            <NavLink
              to="/cash-register"
              className="nav-item"
              style={{ color: 'var(--danger)', background: 'rgba(244,82,112,0.07)', border: '1px solid rgba(244,82,112,0.15)' }}
            >
              <IcoAlertCircle /> Caja cerrada
            </NavLink>
          )}
          <div className="sidebar-user">
            <div className="sidebar-avatar">
              {user?.full_name?.charAt(0).toUpperCase()}
            </div>
            <div style={{ flex: 1, minWidth: 0 }}>
              <p style={{ fontSize: 12, fontWeight: 700, color: 'var(--t1)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                {user?.full_name}
              </p>
              <p style={{ fontSize: 10, color: 'var(--t3)', fontWeight: 600, textTransform: 'uppercase' }}>
                {user?.role === 'admin' ? 'Administrador' : 'Cajero'}
              </p>
            </div>
          </div>
          <button onClick={handleLogout} className="sidebar-logout">
            <IcoLogOut /> Cerrar sesión
          </button>
        </div>
      </aside>

      {/* ── Main content ── */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', position: 'relative', zIndex: 1 }}>
        {/* Topbar — hidden on POS */}
        {!isPOS && (
          <div className="topbar">
            <div>
              <p className="topbar-title-kicker">{meta.sub}</p>
              <h1 className="topbar-title">{meta.title}</h1>
            </div>
            <div className="topbar-actions">
              <GlobalSearch isAdmin={isAdmin} />
              <NotificationBell />
            </div>
          </div>
        )}

        {/* Page outlet */}
        <div className="content-area">
          {isPOS ? (
            <Outlet />
          ) : (
            <div className="content-scroll no-scrollbar">
              <Outlet />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
