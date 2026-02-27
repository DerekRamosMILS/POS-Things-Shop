// Product types
export interface Product {
    id: number;
    sku: string;
    barcode: string | null;
    name: string;
    description: string | null;
    category_id: number | null;
    category_name: string | null;
    supplier_id: number | null;
    supplier_name: string | null;
    purchase_price: number;
    sale_price: number;
    stock: number;
    min_stock: number;
    is_active: boolean;
    created_at: string;
    updated_at: string;
}

export interface CreateProductDto {
    sku: string;
    barcode: string | null;
    name: string;
    description: string | null;
    category_id: number | null;
    supplier_id: number | null;
    purchase_price: number;
    sale_price: number;
    stock: number;
    min_stock: number;
}

export interface UpdateProductDto {
    id: number;
    sku: string;
    barcode: string | null;
    name: string;
    description: string | null;
    category_id: number | null;
    supplier_id: number | null;
    purchase_price: number;
    sale_price: number;
    min_stock: number;
    is_active: boolean;
}

export interface ProductFilters {
    search?: string;
    category_id?: number;
    is_active?: boolean;
    low_stock?: boolean;
}

// Category types
export interface Category {
    id: number;
    name: string;
    description: string | null;
    is_active: boolean;
    product_count: number | null;
    created_at: string;
    updated_at: string;
}

export interface CreateCategoryDto {
    name: string;
    description: string | null;
}

// Supplier types
export interface Supplier {
    id: number;
    name: string;
    contact_name: string | null;
    phone: string | null;
    email: string | null;
    address: string | null;
    notes: string | null;
    is_active: boolean;
    created_at: string;
    updated_at: string;
}

export interface CreateSupplierDto {
    name: string;
    contact_name?: string | null;
    phone?: string | null;
    email?: string | null;
    address?: string | null;
    notes?: string | null;
}

// Sale types
export interface Sale {
    id: number;
    folio: string;
    user_id: number;
    user_name: string | null;
    cash_register_id: number | null;
    subtotal: number;
    discount_total: number;
    tax: number;
    total: number;
    payment_method: string;
    amount_paid: number;
    change_amount: number;
    status: string;
    notes: string | null;
    items: SaleItem[] | null;
    created_at: string;
}

export interface SaleItem {
    id: number;
    sale_id: number;
    product_id: number;
    product_name: string;
    product_sku: string;
    quantity: number;
    unit_price: number;
    discount: number;
    subtotal: number;
}

export interface CreateSaleDto {
    items: CreateSaleItemDto[];
    payment_method: string;
    amount_paid: number;
    discount_total: number;
    notes?: string | null;
}

export interface CreateSaleItemDto {
    product_id: number;
    quantity: number;
    unit_price: number;
    discount: number;
}

export interface SaleFilters {
    date_from?: string;
    date_to?: string;
    status?: string;
    payment_method?: string;
    user_id?: number;
}

// User types
export interface User {
    id: number;
    username: string;
    full_name: string;
    role: 'admin' | 'cashier';
    is_active: boolean;
    created_at: string;
    updated_at: string;
}

export interface LoginDto {
    username: string;
    password: string;
}

export interface LoginResponse {
    user: User;
    token: string;
}

// Cash register types
export interface CashRegister {
    id: number;
    user_id: number;
    user_name: string | null;
    opening_amount: number;
    closing_amount: number | null;
    expected_amount: number | null;
    difference: number | null;
    total_sales: number;
    total_cash_sales: number;
    total_card_sales: number;
    total_transfer_sales: number;
    total_expenses: number;
    sale_count: number;
    status: string;
    opened_at: string;
    closed_at: string | null;
}

// Expense
export interface Expense {
    id: number;
    cash_register_id: number | null;
    category: string;
    description: string;
    amount: number;
    user_id: number;
    user_name: string | null;
    created_at: string;
}

export interface CreateExpenseDto {
    category: string;
    description: string;
    amount: number;
}

// Inventory
export interface InventoryMovement {
    id: number;
    product_id: number;
    product_name: string | null;
    movement_type: string;
    quantity: number;
    previous_stock: number;
    new_stock: number;
    reference_id: number | null;
    reason: string | null;
    user_id: number | null;
    user_name: string | null;
    created_at: string;
}

// Reports
export interface DashboardStats {
    today_sales: number;
    today_count: number;
    month_sales: number;
    month_count: number;
    total_products: number;
    low_stock_count: number;
    today_profit: number;
}

export interface DailySalesReport {
    date: string;
    total_sales: number;
    sale_count: number;
    total_cash: number;
    total_card: number;
    total_transfer: number;
    total_expenses: number;
    gross_profit: number;
}

export interface TopProduct {
    product_id: number;
    product_name: string;
    total_quantity: number;
    total_revenue: number;
}

// Config
export interface SystemConfig {
    key: string;
    value: string;
    description: string | null;
}

// Cart types (frontend only)
export interface CartItem {
    product: Product;
    quantity: number;
    discount: number;
}

// Notification types
export interface Notification {
    id: number;
    product_id: number | null;
    message: string;
    target_date: string | null;
    is_read: boolean;
    notification_type: 'low_stock' | 'restock_reminder';
    created_at: string;
}

export interface CreateReminderDto {
    product_id: number;
    target_date: string;
}

// Promotion types
export interface Promotion {
    id: number;
    name: string;
    description: string | null;
    discount_type: string;
    discount_value: number;
    start_date: string;
    end_date: string;
    is_active: boolean;
    applies_to: string;
    target_id: number | null;
    created_at: string;
}

export interface CreatePromotionDto {
    name: string;
    description: string | null;
    discount_type: string;
    discount_value: number;
    start_date: string;
    end_date: string;
    applies_to: string;
    target_id: number | null;
}
