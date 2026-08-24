import type { CashRegister } from '../types';

/**
 * Cálculos de efectivo que la pantalla necesita mostrar antes de que el backend
 * responda.
 *
 * Duplican lógica que también vive en Rust, y esa duplicación es precisamente el
 * riesgo: si una de las dos deriva, el cajero ve un número y el sistema cobra
 * otro. Están aquí, aisladas y con pruebas que reflejan los mismos casos que las
 * del backend, para que la divergencia se detecte antes de llegar al mostrador.
 */

const CENTS = 100;

/** Redondea al centavo, evitando el arrastre del punto flotante. */
export function round2(value: number): number {
    if (!Number.isFinite(value)) return 0;
    return Math.round(value * CENTS) / CENTS;
}

/** Suma importes sin acumular error: se opera en centavos enteros. */
export function sumMoney(values: number[]): number {
    const cents = values.reduce((acc, v) => acc + Math.round((Number.isFinite(v) ? v : 0) * CENTS), 0);
    return cents / CENTS;
}

/** Renglón del desglose que se muestra al cerrar la caja. */
export interface CashLine {
    label: string;
    amount: number;
}

/**
 * Todo lo que entra y sale del cajón durante un turno.
 *
 * Debe coincidir con `expected_cash` del backend: fondo, ventas en efectivo y
 * abonos en efectivo entran; devoluciones en efectivo y gastos salen.
 */
export function cashBreakdown(register: CashRegister): CashLine[] {
    return [
        { label: 'Fondo de apertura', amount: register.opening_amount },
        { label: 'Ventas en efectivo', amount: register.total_cash_sales },
        { label: 'Abonos de apartados', amount: register.total_layaway_cash },
        { label: 'Devoluciones en efectivo', amount: -register.total_refunds_cash },
        { label: 'Gastos', amount: -register.total_expenses },
    ];
}

/** Efectivo que debería haber físicamente en el cajón. */
export function expectedCash(register: CashRegister): number {
    return sumMoney(cashBreakdown(register).map(l => l.amount));
}

export interface MixedTender {
    /** Cubierto con tarjeta y transferencia. */
    nonCash: number;
    /** Lo que falta cubrir en efectivo. */
    cashDue: number;
    /** Cambio a devolver. */
    change: number;
    /** Motivo por el que no se puede cobrar, o null si todo está bien. */
    problem: string | null;
}

/**
 * Reparte un cobro mixto igual que lo hará el backend, para poder mostrar el
 * faltante y el cambio mientras el cajero captura.
 */
export function evaluateMixedTender(
    total: number,
    card: number,
    transfer: number,
    cash: number,
): MixedTender {
    const nonCash = sumMoney([card, transfer]);
    const cashDue = round2(Math.max(0, total - nonCash));
    const change = round2(Math.max(0, cash - cashDue));

    let problem: string | null = null;
    if (nonCash <= 0) {
        problem = 'Captura cuánto va en tarjeta o transferencia';
    } else if (nonCash > total) {
        problem = 'Tarjeta y transferencia suman más que el total de la venta';
    } else if (cash < cashDue) {
        problem = 'Falta efectivo para completar el pago';
    }

    return { nonCash, cashDue, change, problem };
}
