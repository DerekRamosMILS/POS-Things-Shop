import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type {
    Product, CreateProductDto, UpdateProductDto, ProductFilters,
    Category, CreateCategoryDto,
    Supplier, CreateSupplierDto,
    Sale, CreateSaleDto, SaleFilters,
    User, LoginDto, LoginResponse,
    CashRegister,
    Expense, CreateExpenseDto,
    InventoryMovement,
    DashboardStats, DailySalesReport, TopProduct,
    SystemConfig,
    Promotion, CreatePromotionDto,
    Notification, CreateReminderDto,
} from '../types';

type InvokeArgs = Record<string, unknown> | undefined;

const isTauriRuntime = (): boolean => {
    if (typeof window === 'undefined') return false;
    return '__TAURI_INTERNALS__' in window || '__TAURI__' in window;
};

const nowIso = () => new Date().toISOString();

const webUsers: User[] = [
    {
        id: 1,
        username: 'admin',
        full_name: 'Administrador',
        role: 'admin',
        is_active: true,
        created_at: nowIso(),
        updated_at: nowIso(),
    },
    {
        id: 2,
        username: 'cajero',
        full_name: 'Cajero Demo',
        role: 'cashier',
        is_active: true,
        created_at: nowIso(),
        updated_at: nowIso(),
    },
];

const webCategories: Category[] = [
    { id: 1, name: 'Ropa', description: null, is_active: true, product_count: 2, created_at: nowIso(), updated_at: nowIso() },
    { id: 2, name: 'Calzado', description: null, is_active: true, product_count: 1, created_at: nowIso(), updated_at: nowIso() },
    { id: 3, name: 'Accesorios', description: null, is_active: true, product_count: 1, created_at: nowIso(), updated_at: nowIso() },
];

const webProducts: Product[] = [
    {
        id: 1, sku: 'CAM-001', barcode: '750000000001', name: 'Camisa Negra', description: null,
        category_id: 1, category_name: 'Ropa', supplier_id: null, supplier_name: null,
        purchase_price: 120, sale_price: 249, stock: 18, min_stock: 5, is_active: true,
        image_url: null, created_at: nowIso(), updated_at: nowIso(),
    },
    {
        id: 2, sku: 'PAN-002', barcode: '750000000002', name: 'Pantalón Slim', description: null,
        category_id: 1, category_name: 'Ropa', supplier_id: null, supplier_name: null,
        purchase_price: 180, sale_price: 369, stock: 10, min_stock: 4, is_active: true,
        image_url: null, created_at: nowIso(), updated_at: nowIso(),
    },
    {
        id: 3, sku: 'TEN-003', barcode: '750000000003', name: 'Tenis Urban', description: null,
        category_id: 2, category_name: 'Calzado', supplier_id: null, supplier_name: null,
        purchase_price: 420, sale_price: 799, stock: 7, min_stock: 3, is_active: true,
        image_url: null, created_at: nowIso(), updated_at: nowIso(),
    },
    {
        id: 4, sku: 'GOR-004', barcode: '750000000004', name: 'Gorra Logo', description: null,
        category_id: 3, category_name: 'Accesorios', supplier_id: null, supplier_name: null,
        purchase_price: 70, sale_price: 159, stock: 24, min_stock: 6, is_active: true,
        image_url: null, created_at: nowIso(), updated_at: nowIso(),
    },
];

let webSales: Sale[] = [];
let webExpenses: Expense[] = [];
let webOpenRegister: CashRegister | null = null;

const webInvoke = async <T>(command: string, args?: InvokeArgs): Promise<T> => {
    switch (command) {
        case 'login': {
            const payload = args?.data as LoginDto;
            const found = webUsers.find(u => u.username === payload.username && u.is_active);
            if (!found || !payload.password) throw new Error('Credenciales inválidas');
            return { user: found, token: `web-token-${found.id}` } as unknown as T;
        }

        case 'get_products': {
            const filters = (args?.filters as ProductFilters | undefined) ?? {};
            const search = (filters.search ?? '').toLowerCase();
            const filtered = webProducts.filter(p => {
                if (filters.is_active !== undefined && p.is_active !== filters.is_active) return false;
                if (filters.category_id !== undefined && p.category_id !== filters.category_id) return false;
                if (filters.low_stock && p.stock > p.min_stock) return false;
                if (search && !(`${p.name} ${p.sku} ${p.barcode ?? ''}`).toLowerCase().includes(search)) return false;
                return true;
            });
            return filtered as unknown as T;
        }

        case 'get_product_by_barcode': {
            const barcode = args?.barcode as string;
            const found = webProducts.find(p => p.barcode === barcode) ?? null;
            return found as unknown as T;
        }

        case 'get_categories':
            return webCategories as unknown as T;

        case 'get_suppliers':
            return [] as unknown as T;

        case 'get_promotions':
            return [] as unknown as T;

        case 'get_users':
            return webUsers as unknown as T;

        case 'get_notifications':
            return [] as unknown as T;

        case 'get_sales':
            return webSales as unknown as T;

        case 'get_sale_detail': {
            const saleId = args?.saleId as number;
            const sale = webSales.find(s => s.id === saleId);
            if (!sale) throw new Error('Venta no encontrada');
            return sale as unknown as T;
        }

        case 'create_sale': {
            const data = args?.data as CreateSaleDto;
            const saleId = webSales.length + 1;
            const subtotal = data.items.reduce((sum, it) => sum + it.unit_price * it.quantity, 0);
            const total = Math.max(0, subtotal - (data.discount_total ?? 0));
            const sale: Sale = {
                id: saleId,
                folio: `WEB-${String(saleId).padStart(6, '0')}`,
                user_id: args?.userId as number,
                user_name: webUsers.find(u => u.id === (args?.userId as number))?.full_name ?? 'Web User',
                cash_register_id: (args?.cashRegisterId as number | null) ?? null,
                subtotal,
                discount_total: data.discount_total ?? 0,
                tax: 0,
                total,
                payment_method: data.payment_method,
                amount_paid: data.amount_paid,
                change_amount: Math.max(0, data.amount_paid - total),
                status: 'completed',
                notes: data.notes ?? null,
                items: data.items.map((it, idx) => ({
                    id: idx + 1,
                    sale_id: saleId,
                    product_id: it.product_id,
                    product_name: webProducts.find(p => p.id === it.product_id)?.name ?? 'Producto',
                    product_sku: webProducts.find(p => p.id === it.product_id)?.sku ?? '',
                    quantity: it.quantity,
                    unit_price: it.unit_price,
                    discount: it.discount,
                    subtotal: Math.max(0, it.unit_price * it.quantity - it.discount),
                })),
                created_at: nowIso(),
            };
            webSales = [sale, ...webSales];
            return sale as unknown as T;
        }

        case 'cancel_sale':
            return undefined as unknown as T;

        case 'get_inventory_movements':
            return [] as unknown as T;

        case 'set_product_image': {
            const { productId, imageUrl } = args as { productId: number; imageUrl: string | null };
            const p = webProducts.find(p => p.id === productId);
            if (p) p.image_url = imageUrl ?? null;
            return undefined as unknown as T;
        }

        case 'create_product': {
            const data = args?.data as CreateProductDto;
            const id = webProducts.length + 1;
            const newP: Product = {
                id, sku: data.sku, barcode: data.barcode ?? null, name: data.name,
                description: data.description ?? null, category_id: data.category_id ?? null,
                category_name: webCategories.find(c => c.id === data.category_id)?.name ?? null,
                supplier_id: data.supplier_id ?? null, supplier_name: null,
                purchase_price: data.purchase_price, sale_price: data.sale_price,
                stock: data.stock, min_stock: data.min_stock, is_active: true,
                image_url: null, created_at: nowIso(), updated_at: nowIso(),
            };
            webProducts.push(newP);
            return newP as unknown as T;
        }
        case 'update_product': {
            const data = args?.data as UpdateProductDto;
            const idx = webProducts.findIndex(p => p.id === data.id);
            if (idx !== -1) {
                webProducts[idx] = { ...webProducts[idx], ...data, updated_at: nowIso() };
                return webProducts[idx] as unknown as T;
            }
            return webProducts[0] as unknown as T;
        }
        case 'adjust_stock':
        case 'register_purchase':
        case 'delete_product':
        case 'create_category':
        case 'update_category':
        case 'delete_category':
        case 'create_supplier':
        case 'update_supplier':
        case 'delete_supplier':
        case 'create_user':
        case 'update_user':
        case 'change_password':
        case 'update_promotion':
        case 'create_promotion':
        case 'delete_promotion':
        case 'mark_notification_read':
        case 'create_reminder':
        case 'update_expense':
        case 'delete_expense':
        case 'set_config':
        case 'export_database':
            return undefined as unknown as T;

        case 'get_low_stock_products':
            return webProducts.filter(p => p.stock <= p.min_stock) as unknown as T;

        case 'open_register': {
            webOpenRegister = {
                id: 1,
                user_id: args?.userId as number,
                user_name: webUsers.find(u => u.id === (args?.userId as number))?.full_name ?? 'Web User',
                opening_amount: (args?.data as { opening_amount: number }).opening_amount,
                closing_amount: null,
                expected_amount: null,
                difference: null,
                total_sales: 0,
                total_cash_sales: 0,
                total_card_sales: 0,
                total_transfer_sales: 0,
                total_expenses: 0,
                sale_count: 0,
                status: 'open',
                opened_at: nowIso(),
                closed_at: null,
            };
            return webOpenRegister as unknown as T;
        }

        case 'close_register': {
            if (!webOpenRegister) throw new Error('No hay caja abierta');
            webOpenRegister.status = 'closed';
            webOpenRegister.closed_at = nowIso();
            webOpenRegister.closing_amount = (args?.data as { closing_amount: number }).closing_amount;
            return webOpenRegister as unknown as T;
        }

        case 'get_open_register':
            return (webOpenRegister?.status === 'open' ? webOpenRegister : null) as unknown as T;

        case 'get_register_history':
            return (webOpenRegister ? [webOpenRegister] : []) as unknown as T;

        case 'create_expense': {
            const data = args?.data as CreateExpenseDto;
            const id = webExpenses.length + 1;
            const expense: Expense = {
                id,
                cash_register_id: (args?.cashRegisterId as number | null) ?? null,
                category: data.category,
                description: data.description,
                amount: data.amount,
                user_id: args?.userId as number,
                user_name: webUsers.find(u => u.id === (args?.userId as number))?.full_name ?? 'Web User',
                created_at: nowIso(),
            };
            webExpenses = [expense, ...webExpenses];
            return expense as unknown as T;
        }

        case 'get_expenses':
            return webExpenses as unknown as T;

        case 'get_dashboard_stats': {
            const todaySales = webSales.reduce((sum, s) => sum + s.total, 0);
            return {
                today_sales: todaySales,
                today_count: webSales.length,
                month_sales: todaySales,
                month_count: webSales.length,
                total_products: webProducts.length,
                low_stock_count: webProducts.filter(p => p.stock <= p.min_stock).length,
                today_profit: todaySales * 0.28,
            } as unknown as T;
        }

        case 'get_daily_sales_report':
            return [] as unknown as T;

        case 'get_top_products':
            return [] as unknown as T;

        case 'create_backup':
            return `backup-web-${Date.now()}.db` as unknown as T;

        case 'get_backup_list':
            return [] as unknown as T;

        case 'get_all_config':
            return [
                { key: 'store_name', value: 'Things Shop (Web Demo)', description: null },
                { key: 'tax_rate', value: '0', description: null },
                { key: 'currency_symbol', value: '$', description: null },
                { key: 'low_stock_threshold', value: '5', description: null },
                { key: 'max_backups', value: '90', description: null },
            ] as unknown as T;

        case 'get_config':
            return '' as unknown as T;

        default:
            throw new Error(`Comando no soportado en modo web: ${command}`);
    }
};

const invoke = <T>(command: string, args?: InvokeArgs): Promise<T> => {
    if (isTauriRuntime()) {
        return tauriInvoke<T>(command, args);
    }
    return webInvoke<T>(command, args);
};

// Products
export const getProducts = (filters?: ProductFilters) => invoke<Product[]>('get_products', { filters });
export const getProductByBarcode = (barcode: string) => invoke<Product | null>('get_product_by_barcode', { barcode });
export const createProduct = (data: CreateProductDto) => invoke<Product>('create_product', { data });
export const updateProduct = (data: UpdateProductDto) => invoke<Product>('update_product', { data });
export const deleteProduct = (id: number) => invoke<void>('delete_product', { id });
export const setProductImage = (productId: number, imageUrl: string | null) =>
    invoke<void>('set_product_image', { productId, imageUrl });

// Categories
export const getCategories = () => invoke<Category[]>('get_categories');
export const createCategory = (data: CreateCategoryDto) => invoke<Category>('create_category', { data });
export const updateCategory = (data: { id: number; name: string; description: string | null; is_active: boolean }) => invoke<void>('update_category', { data });
export const deleteCategory = (id: number) => invoke<void>('delete_category', { id });

// Suppliers
export const getSuppliers = () => invoke<Supplier[]>('get_suppliers');
export const createSupplier = (data: CreateSupplierDto) => invoke<Supplier>('create_supplier', { data });
export const updateSupplier = (data: Supplier) => invoke<void>('update_supplier', { data });
export const deleteSupplier = (id: number) => invoke<void>('delete_supplier', { id });

// Sales
export const createSale = (userId: number, cashRegisterId: number | null, data: CreateSaleDto) =>
    invoke<Sale>('create_sale', { userId, cashRegisterId, data });
export const cancelSale = (saleId: number, userId: number) => invoke<void>('cancel_sale', { saleId, userId });
export const getSales = (filters?: SaleFilters) => invoke<Sale[]>('get_sales', { filters });
export const getSaleDetail = (saleId: number) => invoke<Sale>('get_sale_detail', { saleId });

// Inventory
export const getInventoryMovements = (productId?: number, limit?: number) =>
    invoke<InventoryMovement[]>('get_inventory_movements', { productId, limit });
export const adjustStock = (userId: number, data: { product_id: number; quantity: number; reason: string }) =>
    invoke<void>('adjust_stock', { userId, data });
export const registerPurchase = (userId: number, data: { product_id: number; quantity: number; purchase_price?: number }) =>
    invoke<void>('register_purchase', { userId, data });
export const getLowStockProducts = () => invoke<Product[]>('get_low_stock_products');

// Users
export const login = (data: LoginDto) => invoke<LoginResponse>('login', { data });
export const createUser = (data: { username: string; password: string; full_name: string; role: string }) =>
    invoke<User>('create_user', { data });
export const getUsers = () => invoke<User[]>('get_users');
export const updateUser = (data: { id: number; username: string; full_name: string; role: string; is_active: boolean }) =>
    invoke<void>('update_user', { data });
export const changePassword = (data: { user_id: number; new_password: string }) =>
    invoke<void>('change_password', { data });

// Cash Register
export const openRegister = (userId: number, data: { opening_amount: number }) =>
    invoke<CashRegister>('open_register', { userId, data });
export const closeRegister = (data: { closing_amount: number }) => invoke<CashRegister>('close_register', { data });
export const getOpenRegister = () => invoke<CashRegister | null>('get_open_register');
export const getRegisterHistory = (limit?: number) => invoke<CashRegister[]>('get_register_history', { limit });

// Expenses
export const createExpense = (userId: number, cashRegisterId: number | null, data: CreateExpenseDto) =>
    invoke<Expense>('create_expense', { userId, cashRegisterId, data });
export const getExpenses = (cashRegisterId?: number) => invoke<Expense[]>('get_expenses', { cashRegisterId });
export const updateExpense = (data: Expense) => invoke<void>('update_expense', { data });
export const deleteExpense = (id: number) => invoke<void>('delete_expense', { id });

// Promotions
export const getPromotions = () => invoke<Promotion[]>('get_promotions');
export const createPromotion = (data: CreatePromotionDto) => invoke<Promotion>('create_promotion', { data });
export const updatePromotion = (data: Promotion) => invoke<void>('update_promotion', { data });
export const deletePromotion = (id: number) => invoke<void>('delete_promotion', { id });

// Reports
export const getDashboardStats = () => invoke<DashboardStats>('get_dashboard_stats');
export const getDailySalesReport = (days?: number) => invoke<DailySalesReport[]>('get_daily_sales_report', { days });
export const getTopProducts = (days?: number, limit?: number) => invoke<TopProduct[]>('get_top_products', { days, limit });

// Backup
export const createBackup = () => invoke<string>('create_backup');
export const exportDatabase = (path: string) => invoke<void>('export_database', { path });
export const getBackupList = () => invoke<string[]>('get_backup_list');

// Config
export const getAllConfig = () => invoke<SystemConfig[]>('get_all_config');
export const getConfig = (key: string) => invoke<string>('get_config', { key });
export const setConfig = (key: string, value: string) => invoke<void>('set_config', { key, value });

// Notifications
export const getNotifications = () => invoke<Notification[]>('get_notifications');
export const markNotificationRead = (id: number, notificationType: string) => invoke<void>('mark_notification_read', { id, notificationType });
export const createReminder = (data: CreateReminderDto) => invoke<Notification>('create_reminder', { data });

