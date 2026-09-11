import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { useSessionStore } from '../stores/useSessionStore';
import type {
    Product, CreateProductDto, UpdateProductDto, ProductFilters,
    ProductVariant, SaveVariantDto,
    Category, CreateCategoryDto,
    Supplier, CreateSupplierDto,
    Sale, CreateSaleDto, SaleFilters,
    User, LoginDto, LoginResponse,
    CashRegister,
    Expense, CreateExpenseDto,
    InventoryMovement,
    DashboardStats, DailySalesReport, TopProduct, CashierReport,
    SystemConfig,
    Promotion, CreatePromotionDto,
    Notification, CreateReminderDto,
    Customer, CreateCustomerDto, UpdateCustomerDto, CatalogoFiscal,
    VistaRelevo, ProductImage,
    PriceHistoryEntry, CreateReturnDto,
    Layaway, CreateLayawayDto,
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
        must_change_password: false,
        created_at: nowIso(),
        updated_at: nowIso(),
    },
    {
        id: 2,
        username: 'cajero',
        full_name: 'Cajero Demo',
        role: 'cashier',
        is_active: true,
        must_change_password: false,
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
        image_url: null,
        has_image: false, has_variants: false, created_at: nowIso(), updated_at: nowIso(),
    },
    {
        id: 2, sku: 'PAN-002', barcode: '750000000002', name: 'Pantalón Slim', description: null,
        category_id: 1, category_name: 'Ropa', supplier_id: null, supplier_name: null,
        purchase_price: 180, sale_price: 369, stock: 10, min_stock: 4, is_active: true,
        image_url: null,
        has_image: false, has_variants: false, created_at: nowIso(), updated_at: nowIso(),
    },
    {
        id: 3, sku: 'TEN-003', barcode: '750000000003', name: 'Tenis Urban', description: null,
        category_id: 2, category_name: 'Calzado', supplier_id: null, supplier_name: null,
        purchase_price: 420, sale_price: 799, stock: 7, min_stock: 3, is_active: true,
        image_url: null,
        has_image: false, has_variants: false, created_at: nowIso(), updated_at: nowIso(),
    },
    {
        id: 4, sku: 'GOR-004', barcode: '750000000004', name: 'Gorra Logo', description: null,
        category_id: 3, category_name: 'Accesorios', supplier_id: null, supplier_name: null,
        purchase_price: 70, sale_price: 159, stock: 24, min_stock: 6, is_active: true,
        image_url: null,
        has_image: false, has_variants: false, created_at: nowIso(), updated_at: nowIso(),
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

        case 'get_product':
            return (webProducts.find(p => p.id === (args?.id as number)) ?? null) as unknown as T;

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
                customer_id: data.customer_id ?? null,
                customer_name: null,
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
                    returned_quantity: 0,
                    variant_id: it.variant_id ?? null,
                    variant_label: null,
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
                image_url: null,
                has_image: false, has_variants: false, created_at: nowIso(), updated_at: nowIso(),
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
        case 'delete_promotion':
            return 'Promoción eliminada' as unknown as T;

        case 'update_promotion':
        case 'create_promotion':
        case 'mark_notification_read':
        case 'create_reminder':
        case 'update_expense':
        case 'delete_expense':
        case 'set_config':
        case 'dias_sin_copia_externa':
            return null as unknown as T;

        case 'export_database':
            return undefined as unknown as T;

        case 'get_product_images':
            return (args?.productIds as number[] ?? [])
                .map(id => [id, webProducts.find(p => p.id === id)?.image_url])
                .filter(([, url]) => Boolean(url)) as unknown as T;

        case 'get_low_stock_products':
            return webProducts.filter(p => p.stock <= p.min_stock) as unknown as T;

        case 'open_register': {
            webOpenRegister = {
                id: 1,
                user_id: 1,
                user_name: webUsers[0].full_name,
                opening_amount: (args?.data as { opening_amount: number }).opening_amount,
                closing_amount: null,
                expected_amount: null,
                difference: null,
                total_sales: 0,
                total_cash_sales: 0,
                total_card_sales: 0,
                total_transfer_sales: 0,
                total_layaway_cash: 0,
                total_layaway_card: 0,
                total_layaway_transfer: 0,
                total_refunds_cash: 0,
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

        case 'get_cashier_report':
            return [] as unknown as T;

        case 'create_backup':
            return `backup-web-${Date.now()}.db` as unknown as T;

        case 'get_backup_list':
            return [] as unknown as T;

        case 'capture_server_status':
            return { encendido: false, url: null, codigo: null, qr_svg: null, interfaz: null, alternativas: [] } as unknown as T;

        case 'start_capture_server':
        case 'stop_capture_server':
        case 'add_product_image':
        case 'get_product_photo':
        case 'delete_product_image':
        case 'reorder_product_images':
            throw new Error('Solo disponible en la app de escritorio');

        case 'get_product_image_list':
            return [] as unknown as T;

        case 'get_next_sku':
            return 'TS-000001' as unknown as T;

        case 'list_printers':
            return ['Impresora de ejemplo (modo web)'] as unknown as T;

        case 'open_cash_drawer':
        case 'test_printer':
            throw new Error('El hardware solo está disponible en la app de escritorio');

        case 'print_sale_receipt':
        case 'print_layaway_receipt':
            throw new Error('SIN_IMPRESORA');

        case 'get_catalogos_fiscales':
            return { regimenes: [], usos_cfdi: [] } as unknown as T;

        case 'exportar_pendientes_factura':
        case 'marcar_facturada':
        case 'generate_diagnostic_report':
            throw new Error('El diagnóstico solo está disponible en la app de escritorio');

        case 'get_log_path':
            return '(no disponible en modo web)' as unknown as T;

        case 'registrar_evento_actualizacion':
            return undefined as unknown as T;

        case 'logout':
            return undefined as unknown as T;

        case 'validate_session':
            return true as unknown as T;

        case 'change_own_password':
            return undefined as unknown as T;

        case 'restore_backup':
            return 'Restauración no disponible en modo web' as unknown as T;

        case 'seed_demo_data':
            return 'Datos de prueba no disponibles en modo web' as unknown as T;

        case 'get_variants':
            return [] as unknown as T;
        case 'get_variant_by_barcode':
            return null as unknown as T;
        case 'save_variants':
            return [] as unknown as T;

        case 'get_all_config':
            return [
                { key: 'store_name', value: 'Things Shop (Web Demo)', description: null },
                { key: 'store_address', value: '', description: null },
                { key: 'store_phone', value: '', description: null },
                { key: 'store_email', value: '', description: null },
                { key: 'ticket_footer', value: '¡Gracias por su compra!', description: null },
                { key: 'tax_rate', value: '0', description: null },
                { key: 'currency_symbol', value: '$', description: null },
                { key: 'low_stock_threshold', value: '5', description: null },
                { key: 'auto_backup', value: '1', description: null },
                { key: 'max_backups', value: '30', description: null },
                { key: 'session_hours', value: '12', description: null },
                { key: 'log_retention_days', value: '90', description: null },
            ] as unknown as T;

        case 'get_config':
            return '' as unknown as T;

        case 'get_customers':
            return [] as unknown as T;
        case 'get_price_history':
            return [] as unknown as T;
        case 'get_layaways':
            return [] as unknown as T;
        case 'create_return':
            return 0 as unknown as T;
        case 'cancel_layaway':
            return 'Apartado cancelado' as unknown as T;
        case 'update_customer':
        case 'delete_customer':
            return undefined as unknown as T;
        case 'create_customer':
        case 'create_layaway':
        case 'get_layaway_detail':
        case 'add_layaway_payment':
        case 'complete_layaway':
            throw new Error('Función no disponible en modo web');

        default:
            throw new Error(`Comando no soportado en modo web: ${command}`);
    }
};

const invoke = <T>(command: string, args?: InvokeArgs): Promise<T> => {
    // Attach the session token so backend commands can verify role. Reads that
    // don't declare a `token` param simply ignore it.
    const token = useSessionStore.getState().token;
    const merged = token ? { ...(args ?? {}), token } : args;
    if (isTauriRuntime()) {
        return tauriInvoke<T>(command, merged);
    }
    return webInvoke<T>(command, merged);
};

// Products
export const getProducts = (filters?: ProductFilters) => invoke<Product[]>('get_products', { filters });
export const getProductById = (id: number) => invoke<Product | null>('get_product', { id });
export const getProductByBarcode = (barcode: string) => invoke<Product | null>('get_product_by_barcode', { barcode });
export const createProduct = (data: CreateProductDto) => invoke<Product>('create_product', { data });
export const updateProduct = (data: UpdateProductDto) => invoke<Product>('update_product', { data });
export const deleteProduct = (id: number) => invoke<void>('delete_product', { id });
export const getProductImages = (productIds: number[]) =>
    invoke<[number, string][]>('get_product_images', { productIds });

// Variants
export const getVariants = (productId: number) => invoke<ProductVariant[]>('get_variants', { productId });
export const getVariantByBarcode = (barcode: string) => invoke<ProductVariant | null>('get_variant_by_barcode', { barcode });
export const saveVariants = (productId: number, variants: SaveVariantDto[]) =>
    invoke<ProductVariant[]>('save_variants', { productId, variants });

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
export const createSale = (cashRegisterId: number | null, data: CreateSaleDto) =>
    invoke<Sale>('create_sale', { cashRegisterId, data });
export const cancelSale = (saleId: number) => invoke<void>('cancel_sale', { saleId });
export const getSales = (filters?: SaleFilters) => invoke<Sale[]>('get_sales', { filters });
export const getSaleDetail = (saleId: number) => invoke<Sale>('get_sale_detail', { saleId });

// Inventory
export const getInventoryMovements = (productId?: number, limit?: number) =>
    invoke<InventoryMovement[]>('get_inventory_movements', { productId, limit });
export const adjustStock = (data: { product_id: number; quantity: number; reason: string }) =>
    invoke<void>('adjust_stock', { data });
export const registerPurchase = (data: { product_id: number; quantity: number; purchase_price?: number }) =>
    invoke<void>('register_purchase', { data });
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
export const changeOwnPassword = (data: { current_password: string; new_password: string }) =>
    invoke<void>('change_own_password', { data });
export const logout = () => invoke<void>('logout');
export const validateSession = () => invoke<boolean>('validate_session');

// Cash Register
export const openRegister = (data: { opening_amount: number }) =>
    invoke<CashRegister>('open_register', { data });
export const closeRegister = (data: { closing_amount: number }) => invoke<CashRegister>('close_register', { data });
export const getOpenRegister = () => invoke<CashRegister | null>('get_open_register');
export const getRegisterHistory = (limit?: number) => invoke<CashRegister[]>('get_register_history', { limit });

// Expenses
export const createExpense = (cashRegisterId: number | null, data: CreateExpenseDto) =>
    invoke<Expense>('create_expense', { cashRegisterId, data });
export const getExpenses = (cashRegisterId?: number) => invoke<Expense[]>('get_expenses', { cashRegisterId });
export const updateExpense = (data: Expense) => invoke<void>('update_expense', { data });
export const deleteExpense = (id: number) => invoke<void>('delete_expense', { id });

// Promotions
export const getPromotions = () => invoke<Promotion[]>('get_promotions');
export const createPromotion = (data: CreatePromotionDto) => invoke<Promotion>('create_promotion', { data });
export const updatePromotion = (data: Promotion) => invoke<void>('update_promotion', { data });
export const deletePromotion = (id: number) => invoke<string>('delete_promotion', { id });

// Reports
export const getDashboardStats = () => invoke<DashboardStats>('get_dashboard_stats');
export const getDailySalesReport = (days?: number) => invoke<DailySalesReport[]>('get_daily_sales_report', { days });
export const getTopProducts = (days?: number, limit?: number) => invoke<TopProduct[]>('get_top_products', { days, limit });
export const getCashierReport = (days?: number) => invoke<CashierReport[]>('get_cashier_report', { days });

// Backup
export const createBackup = () => invoke<string>('create_backup');
export const diasSinCopiaExterna = () => invoke<number | null>('dias_sin_copia_externa');
export const exportDatabase = (path: string) => invoke<string>('export_database', { path });
export const getBackupList = () => invoke<string[]>('get_backup_list');
export const getLogPath = () => invoke<string>('get_log_path');
export const generateDiagnosticReport = (path: string) =>
    invoke<string>('generate_diagnostic_report', { path });

// Facturación
export const getCatalogosFiscales = () => invoke<CatalogoFiscal>('get_catalogos_fiscales');
export const exportarPendientesFactura = (path: string, desde?: string, hasta?: string) =>
    invoke<number>('exportar_pendientes_factura', { path, desde, hasta });
export const marcarFacturada = (saleId: number, uuid: string) =>
    invoke<void>('marcar_facturada', { saleId, uuid });

// Captura desde el celular
export const relevoEstado = () => invoke<VistaRelevo>('relevo_estado');
export const relevoSincronizar = () => invoke<VistaRelevo>('relevo_sincronizar');
export const relevoRegenerar = () => invoke<VistaRelevo>('relevo_regenerar');

// Fotos de producto
export const addProductImage = (data: { product_id: number; photo: string; thumbnail: string }) =>
    invoke<ProductImage>('add_product_image', { data });
export const getProductImageList = (productId: number) =>
    invoke<ProductImage[]>('get_product_image_list', { productId });
export const getProductPhoto = (imageId: number) =>
    invoke<string>('get_product_photo', { imageId });
export const deleteProductImage = (imageId: number) =>
    invoke<void>('delete_product_image', { imageId });
export const reorderProductImages = (productId: number, imageIds: number[]) =>
    invoke<void>('reorder_product_images', { productId, imageIds });
export const getNextSku = () => invoke<string>('get_next_sku');

// Hardware de mostrador
export const listPrinters = () => invoke<string[]>('list_printers');
export const openCashDrawer = () => invoke<void>('open_cash_drawer');
export const testPrinter = (printer: string | null, openDrawer: boolean) =>
    invoke<void>('test_printer', { printer, openDrawer });
export const printSaleReceipt = (saleId: number, openDrawer?: boolean) =>
    invoke<void>('print_sale_receipt', { saleId, openDrawer });
export const printLayawayReceipt = (layawayId: number) =>
    invoke<void>('print_layaway_receipt', { layawayId });
export const restoreBackup = (filename: string) => invoke<string>('restore_backup', { filename });

// Demo data
export const seedDemoData = () => invoke<string>('seed_demo_data');

// Config
export const getAllConfig = () => invoke<SystemConfig[]>('get_all_config');
export const getConfig = (key: string) => invoke<string>('get_config', { key });
export const setConfig = (key: string, value: string) => invoke<void>('set_config', { key, value });
export const registrarEventoActualizacion = (mensaje: string) =>
    invoke<void>('registrar_evento_actualizacion', { mensaje });

// Notifications
export const getNotifications = () => invoke<Notification[]>('get_notifications');
export const markNotificationRead = (id: number, notificationType: string) => invoke<void>('mark_notification_read', { id, notificationType });
export const createReminder = (data: CreateReminderDto) => invoke<Notification>('create_reminder', { data });

// Customers
export const getCustomers = () => invoke<Customer[]>('get_customers');
export const createCustomer = (data: CreateCustomerDto) => invoke<Customer>('create_customer', { data });
export const updateCustomer = (data: UpdateCustomerDto) => invoke<void>('update_customer', { data });
export const deleteCustomer = (id: number) => invoke<void>('delete_customer', { id });

// Price history
export const getPriceHistory = (productId: number) => invoke<PriceHistoryEntry[]>('get_price_history', { productId });

// Returns
export const createReturn = (data: CreateReturnDto) => invoke<number>('create_return', { data });

// Layaways (apartados)
export const createLayaway = (data: CreateLayawayDto) => invoke<Layaway>('create_layaway', { data });
export const getLayaways = (status?: string) => invoke<Layaway[]>('get_layaways', { status });
export const getLayawayDetail = (layawayId: number) => invoke<Layaway>('get_layaway_detail', { layawayId });
export const addLayawayPayment = (layawayId: number, amount: number, paymentMethod: string) =>
    invoke<Layaway>('add_layaway_payment', { layawayId, amount, paymentMethod });
export const completeLayaway = (layawayId: number) => invoke<Layaway>('complete_layaway', { layawayId });
export const cancelLayaway = (layawayId: number) => invoke<string>('cancel_layaway', { layawayId });

