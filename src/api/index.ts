import { invoke } from '@tauri-apps/api/core';
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

// Products
export const getProducts = (filters?: ProductFilters) => invoke<Product[]>('get_products', { filters });
export const getProductByBarcode = (barcode: string) => invoke<Product | null>('get_product_by_barcode', { barcode });
export const createProduct = (data: CreateProductDto) => invoke<Product>('create_product', { data });
export const updateProduct = (data: UpdateProductDto) => invoke<Product>('update_product', { data });
export const deleteProduct = (id: number) => invoke<void>('delete_product', { id });

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

