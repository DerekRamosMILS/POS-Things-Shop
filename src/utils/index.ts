// Currency symbol is configurable from Settings (system_config.currency_symbol).
// Loaded once at startup via setCurrencySymbol.
let CURRENCY_SYMBOL = '$';

export function setCurrencySymbol(symbol: string) {
    if (symbol && symbol.trim()) CURRENCY_SYMBOL = symbol.trim();
}

export function formatCurrency(amount: number): string {
    const n = new Intl.NumberFormat('es-MX', {
        minimumFractionDigits: 2,
        maximumFractionDigits: 2,
    }).format(Number.isFinite(amount) ? amount : 0);
    return `${CURRENCY_SYMBOL}${n}`;
}

export function formatDate(date: string): string {
    return new Date(date).toLocaleDateString('es-MX', {
        day: '2-digit',
        month: '2-digit',
        year: 'numeric',
    });
}

export function formatDateTime(date: string): string {
    return new Date(date).toLocaleString('es-MX', {
        day: '2-digit',
        month: '2-digit',
        year: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    });
}

export function formatTime(date: string): string {
    return new Date(date).toLocaleTimeString('es-MX', {
        hour: '2-digit',
        minute: '2-digit',
    });
}

export function cn(...classes: (string | boolean | undefined | null)[]): string {
    return classes.filter(Boolean).join(' ');
}

export const MOVEMENT_TYPE_LABELS: Record<string, string> = {
    sale: 'Venta',
    purchase: 'Compra',
    adjustment: 'Ajuste',
    return: 'Devolución',
    cancellation: 'Cancelación',
};

export const PAYMENT_METHOD_LABELS: Record<string, string> = {
    cash: 'Efectivo',
    card: 'Tarjeta',
    transfer: 'Transferencia',
    mixed: 'Pago mixto',
};

export const STATUS_LABELS: Record<string, string> = {
    completed: 'Completada',
    cancelled: 'Cancelada',
    returned: 'Devuelta',
};

export const EXPENSE_CATEGORIES = [
    'Operativos',
    'Servicios',
    'Suministros',
    'Transporte',
    'Comida',
    'Mantenimiento',
    'Otros',
];
