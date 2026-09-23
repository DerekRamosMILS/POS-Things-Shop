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

/**
 * La fecha de hoy en el calendario de la tienda, como `YYYY-MM-DD`.
 *
 * `toISOString()` da la fecha en UTC, que a partir de las seis de la tarde en
 * México ya es la de mañana. Todo lo que compara fechas —el backend con
 * `date('now','localtime')` y el punto de venta con la vigencia de las
 * promociones— usa la local, así que sacar "hoy" de UTC producía desfases de un
 * día que no avisaban de nada.
 */
export function hoyLocal(): string {
    return new Date().toLocaleDateString('en-CA');
}

/** La fecha local corrida `dias` días (negativo hacia atrás), como `YYYY-MM-DD`. */
export function fechaLocal(dias: number): string {
    const d = new Date();
    d.setDate(d.getDate() + dias);
    return d.toLocaleDateString('en-CA');
}

export function formatDate(date: string): string {
    // Una fecha sin hora la interpreta el navegador como medianoche **UTC**, que
    // en México es el día anterior por la tarde: se mostraba un día antes. Con
    // hora explícita se interpreta en local, que es lo que guarda la base.
    const local = /^\d{4}-\d{2}-\d{2}$/.test(date.trim()) ? `${date.trim()}T00:00:00` : date;
    return new Date(local).toLocaleDateString('es-MX', {
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

/**
 * Largo mínimo de una contraseña, el mismo que exige el backend.
 *
 * Estaba escrito tres veces —8 en Rust, 8 en el cambio obligatorio y 6 en la
 * pantalla de usuarios—, y por esa tercera copia un administrador creaba una
 * cajera con seis caracteres: la pantalla la aceptaba prometiendo que bastaban y
 * el backend la rechazaba pidiendo ocho. La prueba `contrasena` falla si este
 * número deja de coincidir con `MIN_PASSWORD_LEN` de `users.rs`.
 */
export const MIN_CONTRASENA = 8;

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

/**
 * Cuántas ventas devuelve el backend por consulta.
 *
 * Tiene que coincidir con el `LIMIT` de `get_sales`: al alcanzarlo, la pantalla
 * lo dice en vez de dejar creer que eso es todo lo que hay.
 */
export const TOPE_LISTA_VENTAS = 1000;
