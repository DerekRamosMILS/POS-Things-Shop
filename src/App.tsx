import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { useSessionStore } from './stores/useSessionStore';
import { ToastProvider } from './contexts/ToastContext';
import { ConfirmProvider } from './contexts/ConfirmContext';
import MainLayout from './components/layout/MainLayout';
import LoginPage from './pages/LoginPage';
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

function ProtectedRoute({ children, adminOnly = false }: { children: React.ReactNode; adminOnly?: boolean }) {
  const { user } = useSessionStore();
  if (!user) return <Navigate to="/login" replace />;
  if (adminOnly && user.role !== 'admin') return <Navigate to="/pos" replace />;
  return <>{children}</>;
}

export default function App() {
  const { user } = useSessionStore();

  return (
    <ToastProvider>
    <ConfirmProvider>
    <BrowserRouter>
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
          <Route path="expenses" element={<ExpensesPage />} />
          <Route path="discounts" element={<DiscountsPage />} />
          <Route path="suppliers" element={<ProtectedRoute adminOnly><SuppliersPage /></ProtectedRoute>} />
          <Route path="reports" element={<ProtectedRoute adminOnly><ReportsPage /></ProtectedRoute>} />
          <Route path="users" element={<ProtectedRoute adminOnly><UsersPage /></ProtectedRoute>} />
          <Route path="settings" element={<ProtectedRoute adminOnly><SettingsPage /></ProtectedRoute>} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
    </ConfirmProvider>
    </ToastProvider>
  );
}

