import { NavLink, Outlet, useNavigate } from 'react-router-dom';
import { useSessionStore } from '../../stores/useSessionStore';
import NotificationBell from './NotificationBell';
import {
    LayoutDashboard,
    ShoppingCart,
    Package,
    Boxes,
    Receipt,
    Users,
    BarChart3,
    Settings,
    LogOut,
    Truck,
    Wallet,
    DollarSign,
    BadgePercent,
    ShoppingBag,
} from 'lucide-react';
import { cn } from '../../utils';

const NAV_ITEMS = [
    { to: '/', icon: LayoutDashboard, label: 'Dashboard', roles: ['admin', 'cashier'] },
    { to: '/pos', icon: ShoppingCart, label: 'Punto de Venta', roles: ['admin', 'cashier'] },
    { to: '/products', icon: Package, label: 'Productos', roles: ['admin'] },
    { to: '/inventory', icon: Boxes, label: 'Inventario', roles: ['admin'] },
    { to: '/sales', icon: Receipt, label: 'Ventas', roles: ['admin'] },
    { to: '/cash-register', icon: Wallet, label: 'Caja', roles: ['admin', 'cashier'] },
    { to: '/expenses', icon: DollarSign, label: 'Gastos', roles: ['admin', 'cashier'] },
    { to: '/discounts', icon: BadgePercent, label: 'Descuentos', roles: ['admin', 'cashier'] },
    { to: '/suppliers', icon: Truck, label: 'Proveedores', roles: ['admin'] },
    { to: '/reports', icon: BarChart3, label: 'Reportes', roles: ['admin'] },
    { to: '/users', icon: Users, label: 'Usuarios', roles: ['admin'] },
    { to: '/settings', icon: Settings, label: 'Configuración', roles: ['admin'] },
];

export default function MainLayout() {
    const { user, logout } = useSessionStore();
    const navigate = useNavigate();

    const handleLogout = () => {
        logout();
        navigate('/login');
    };

    const visibleItems = NAV_ITEMS.filter(
        (item) => user && item.roles.includes(user.role)
    );

    return (
        <div className="h-full flex no-select">
            {/* ============================================
                BRUTALIST GLASSMORPHISM SIDEBAR
                ============================================ */}
            <aside
                className="w-[360px] flex flex-col shrink-0 animate-fade-down"
                style={{
                    background: 'rgba(255, 255, 255, 0.06)',
                    backdropFilter: 'blur(18px)',
                    WebkitBackdropFilter: 'blur(18px)',
                    borderRight: '1px solid rgba(255, 255, 255, 0.18)',
                    boxShadow: '0 20px 60px rgba(0, 0, 0, 0.6)',
                }}
            >
                {/* Logo Section */}
                <div
                    className="px-8 py-8"
                    style={{ borderBottom: '1px solid rgba(255, 255, 255, 0.08)' }}
                >
                    <div className="flex items-center gap-5">
                        {/* Logo icon with glow */}
                        <div
                            className="w-16 h-16 rounded-2xl flex items-center justify-center shrink-0"
                            style={{
                                background: 'linear-gradient(135deg, rgba(0, 224, 90, 0.15), rgba(0, 255, 163, 0.08))',
                                border: '1px solid rgba(0, 224, 90, 0.25)',
                                boxShadow: '0 0 20px rgba(0, 224, 90, 0.15)',
                            }}
                        >
                            <ShoppingBag className="w-8 h-8" style={{ color: '#00E05A' }} />
                        </div>
                        <div>
                            <h1
                                className="text-2xl font-bold leading-tight tracking-wider"
                                style={{
                                    fontFamily: "'Space Grotesk', sans-serif",
                                    textTransform: 'uppercase',
                                    color: '#00E05A',
                                }}
                            >
                                Things Shop
                            </h1>
                            <p
                                className="text-sm tracking-widest"
                                style={{
                                    fontFamily: "'IBM Plex Mono', monospace",
                                    color: '#9CA0AA',
                                    textTransform: 'uppercase',
                                    letterSpacing: '0.15em',
                                    marginTop: '4px',
                                }}
                            >
                                punto de venta
                            </p>
                        </div>
                    </div>
                </div>

                {/* Navigation */}
                <nav className="flex-1 py-6 px-6 space-y-2 overflow-y-auto">
                    {visibleItems.map((item, index) => (
                        <NavLink
                            key={item.to}
                            to={item.to}
                            end={item.to === '/'}
                            className={({ isActive }) =>
                                cn(
                                    'flex items-center gap-5 px-6 py-4 rounded-xl text-base font-medium transition-all duration-200',
                                    isActive
                                        ? 'sidebar-link-active'
                                        : 'sidebar-link'
                                )
                            }
                            style={({ isActive }) => ({
                                fontFamily: "'Space Grotesk', sans-serif",
                                fontSize: '16px',
                                color: isActive ? '#00E05A' : '#9CA0AA',
                                background: isActive
                                    ? 'rgba(0, 224, 90, 0.08)'
                                    : 'transparent',
                                borderLeft: isActive
                                    ? '4px solid #00E05A'
                                    : '4px solid transparent',
                                boxShadow: isActive
                                    ? 'inset 0 0 20px rgba(0, 224, 90, 0.05)'
                                    : 'none',
                                animationDelay: `${index * 40}ms`,
                            })}
                            onMouseEnter={(e) => {
                                const el = e.currentTarget;
                                if (!el.classList.contains('sidebar-link-active')) {
                                    el.style.color = '#f1f5f9';
                                    el.style.background = 'rgba(255, 255, 255, 0.04)';
                                }
                            }}
                            onMouseLeave={(e) => {
                                const el = e.currentTarget;
                                if (!el.classList.contains('sidebar-link-active')) {
                                    el.style.color = '#9CA0AA';
                                    el.style.background = 'transparent';
                                }
                            }}
                        >
                            <item.icon size={24} strokeWidth={1.8} />
                            <span>{item.label}</span>
                        </NavLink>
                    ))}
                </nav>

                {/* User & Logout */}
                <div
                    className="p-6"
                    style={{ borderTop: '1px solid rgba(255, 255, 255, 0.08)' }}
                >
                    {/* User card */}
                    <div
                        className="flex items-center gap-4 mb-4 px-4 py-4 rounded-xl"
                        style={{
                            background: 'rgba(255, 255, 255, 0.03)',
                            border: '1px solid rgba(255, 255, 255, 0.06)',
                        }}
                    >
                        <div
                            className="w-12 h-12 rounded-xl flex items-center justify-center text-base font-bold shrink-0"
                            style={{
                                background: 'linear-gradient(135deg, #00E05A, #00FFA3)',
                                color: '#0B0B0F',
                                boxShadow: '0 4px 15px rgba(0, 224, 90, 0.25)',
                            }}
                        >
                            {user?.full_name?.charAt(0).toUpperCase()}
                        </div>
                        <div className="flex-1 min-w-0">
                            <p
                                className="text-base font-semibold truncate"
                                style={{
                                    fontFamily: "'Space Grotesk', sans-serif",
                                    color: '#f1f5f9',
                                }}
                            >
                                {user?.full_name}
                            </p>
                            <p
                                className="text-sm capitalize mt-0.5"
                                style={{
                                    fontFamily: "'IBM Plex Mono', monospace",
                                    color: '#64748b',
                                }}
                            >
                                {user?.role === 'admin' ? 'Administrador' : 'Cajero'}
                            </p>
                        </div>
                    </div>

                    {/* Logout button — outline style */}
                    <button
                        onClick={handleLogout}
                        className="w-full flex items-center justify-center gap-3 px-6 py-4 rounded-xl text-base font-medium transition-all duration-200 cursor-pointer"
                        style={{
                            fontFamily: "'Space Grotesk', sans-serif",
                            background: 'transparent',
                            border: '1px solid rgba(239, 68, 68, 0.25)',
                            color: '#ef4444',
                        }}
                        onMouseEnter={(e) => {
                            e.currentTarget.style.background = 'rgba(239, 68, 68, 0.08)';
                            e.currentTarget.style.borderColor = 'rgba(239, 68, 68, 0.4)';
                        }}
                        onMouseLeave={(e) => {
                            e.currentTarget.style.background = 'transparent';
                            e.currentTarget.style.borderColor = 'rgba(239, 68, 68, 0.25)';
                        }}
                    >
                        <LogOut size={22} />
                        <span>Cerrar sesión</span>
                    </button>
                </div>
            </aside>

            {/* Main content */}
            <main className="flex-1 overflow-y-auto relative" style={{ backgroundColor: '#0B0B0F' }}>
                <div className="fixed bottom-10 right-10 z-50">
                    <NotificationBell />
                </div>
                <Outlet />
            </main>
        </div>
    );
}
