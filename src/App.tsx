import { useEffect, useState } from 'react';
import { HashRouter, Routes, Route, Navigate } from 'react-router-dom';
import { useSessionStore } from './stores/useSessionStore';
import * as api from './api';
import { setCurrencySymbol } from './utils';
import { ToastProvider } from './contexts/ToastContext';
import { ConfirmProvider } from './contexts/ConfirmContext';
import ErrorBoundary from './components/ErrorBoundary';
import MainLayout from './components/layout/MainLayout';
import LoginPage from './pages/LoginPage';
import ChangePasswordPage from './pages/ChangePasswordPage';
import DashboardPage from './pages/DashboardPage';
import POSPage from './pages/POSPage';
import ProductsPage from './pages/ProductsPage';
import SalesPage from './pages/SalesPage';
import CashRegisterPage from './pages/CashRegisterPage';
import InventoryPage from './pages/InventoryPage';
import UsersPage from './pages/UsersPage';
import ReportsPage from './pages/ReportsPage';
import SettingsPage from './pages/SettingsPage';
import SuppliersPage from './pages/SuppliersPage';
import ExpensesPage from './pages/ExpensesPage';
import DiscountsPage from './pages/DiscountsPage';
import CustomersPage from './pages/CustomersPage';
import LayawaysPage from './pages/LayawaysPage';

function ProtectedRoute({ children, adminOnly = false }: { children: React.ReactNode; adminOnly?: boolean }) {
  const { user } = useSessionStore();
  if (!user) return <Navigate to="/login" replace />;
  if (adminOnly && user.role !== 'admin') return <Navigate to="/pos" replace />;
  return <>{children}</>;
}

export default function App() {
  const { user, token, logout } = useSessionStore();
  // Sessions expire server-side, so a restored token has to be re-checked
  // before the shell is rendered with stale credentials.
  const [checkingSession, setCheckingSession] = useState(Boolean(token));

  useEffect(() => {
    if (!token) { setCheckingSession(false); return; }
    let cancelled = false;
    api.validateSession()
      .then((valid) => { if (!cancelled && !valid) logout(); })
      .catch(() => { if (!cancelled) logout(); })
      .finally(() => { if (!cancelled) setCheckingSession(false); });
    return () => { cancelled = true; };
  }, [token, logout]);

  useEffect(() => {
    // Config reads require a session, so this waits for a logged-in user.
    if (!user) return;
    api.getConfig('currency_symbol').then(setCurrencySymbol).catch(() => {});
  }, [user]);

  if (checkingSession) {
    return (
      <div style={{ height: '100%', display: 'grid', placeItems: 'center', background: 'var(--bg)' }}>
        <div style={{ width: 24, height: 24, border: '2px solid rgba(255,255,255,.2)', borderTopColor: 'var(--primary)', borderRadius: '50%', animation: 'spin .7s linear infinite' }} />
      </div>
    );
  }

  // A seeded or admin-reset password must be rotated before anything else.
  if (user?.must_change_password) {
    return (
      <ErrorBoundary>
        <ToastProvider>
          <ChangePasswordPage />
        </ToastProvider>
      </ErrorBoundary>
    );
  }

  return (
    <ErrorBoundary>
    <ToastProvider>
    <ConfirmProvider>
    <HashRouter>
      <Routes>
        <Route path="/login" element={user ? <Navigate to="/" replace /> : <LoginPage />} />
        <Route path="/" element={<ProtectedRoute><MainLayout /></ProtectedRoute>}>
          <Route index element={<Navigate to="pos" replace />} />
          <Route path="dashboard" element={<DashboardPage />} />
          <Route path="pos" element={<POSPage />} />
          <Route path="products" element={<ProtectedRoute adminOnly><ProductsPage /></ProtectedRoute>} />
          <Route path="inventory" element={<ProtectedRoute adminOnly><InventoryPage /></ProtectedRoute>} />
          <Route path="sales" element={<ProtectedRoute adminOnly><SalesPage /></ProtectedRoute>} />
          <Route path="cash-register" element={<CashRegisterPage />} />
          <Route path="layaways" element={<LayawaysPage />} />
          <Route path="customers" element={<CustomersPage />} />
          <Route path="expenses" element={<ExpensesPage />} />
          <Route path="discounts" element={<DiscountsPage />} />
          <Route path="suppliers" element={<ProtectedRoute adminOnly><SuppliersPage /></ProtectedRoute>} />
          <Route path="reports" element={<ProtectedRoute adminOnly><ReportsPage /></ProtectedRoute>} />
          <Route path="users" element={<ProtectedRoute adminOnly><UsersPage /></ProtectedRoute>} />
          <Route path="settings" element={<ProtectedRoute adminOnly><SettingsPage /></ProtectedRoute>} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </HashRouter>
    </ConfirmProvider>
    </ToastProvider>
    </ErrorBoundary>
  );
}

