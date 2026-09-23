import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { cashBreakdown, evaluateMixedTender, expectedCash, montoContado, round2, sumMoney } from '../utils/cash';
import type { CashRegister } from '../types';

const caja = (over: Partial<CashRegister> = {}): CashRegister => ({
    id: 1, user_id: 1, user_name: null,
    opening_amount: 1000, closing_amount: null, expected_amount: null, difference: null,
    total_sales: 0, total_cash_sales: 0, total_card_sales: 0, total_transfer_sales: 0,
    total_layaway_cash: 0, total_layaway_card: 0, total_layaway_transfer: 0,
    total_refunds_cash: 0, total_expenses: 0, sale_count: 0,
    status: 'open', opened_at: '', closed_at: null,
    ...over,
});

describe('aritmética de dinero', () => {
    it('no arrastra el error del punto flotante al sumar', () => {
        expect(0.1 + 0.2).not.toBe(0.3);
        expect(sumMoney([0.1, 0.2])).toBe(0.3);
    });

    it('un ticket largo suma exacto', () => {
        expect(sumMoney(Array(300).fill(19.99))).toBe(5997);
    });

    it('descarta valores no finitos en lugar de propagar NaN', () => {
        expect(sumMoney([10, NaN, 5])).toBe(15);
        expect(round2(NaN)).toBe(0);
    });
});

// Mismos casos que las pruebas de expected_cash en Rust: si una de las dos
// implementaciones deriva, estas fallan.
describe('efectivo esperado en el cajón', () => {
    it('un turno sin movimientos espera su fondo', () => {
        expect(expectedCash(caja())).toBe(1000);
    });

    it('tarjeta y transferencia nunca llegan al cajón', () => {
        expect(expectedCash(caja({ total_card_sales: 500, total_transfer_sales: 300 }))).toBe(1000);
    });

    it('los abonos en efectivo sí entran', () => {
        expect(expectedCash(caja({ total_layaway_cash: 500 }))).toBe(1500);
    });

    it('los abonos con tarjeta no', () => {
        expect(expectedCash(caja({ total_layaway_card: 500, total_layaway_transfer: 200 }))).toBe(1000);
    });

    it('las devoluciones en efectivo salen', () => {
        expect(expectedCash(caja({ total_cash_sales: 800, total_refunds_cash: 300 }))).toBe(1500);
    });

    it('todos los movimientos juntos', () => {
        const r = caja({
            total_cash_sales: 2500, total_card_sales: 900,
            total_layaway_cash: 400, total_layaway_card: 150,
            total_refunds_cash: 250, total_expenses: 180,
        });
        expect(expectedCash(r)).toBe(3470);
    });

    it('el desglose que se muestra suma exactamente lo esperado', () => {
        const r = caja({ total_cash_sales: 2500, total_layaway_cash: 400, total_expenses: 180 });
        const suma = sumMoney(cashBreakdown(r).map(l => l.amount));
        expect(suma).toBe(expectedCash(r));
    });
});

// Espejo de las pruebas de split_tender en Rust.
describe('cobro mixto', () => {
    it('el efectivo cubre el resto y calcula el cambio', () => {
        const t = evaluateMixedTender(249, 200, 0, 100);
        expect(t.nonCash).toBe(200);
        expect(t.cashDue).toBe(49);
        expect(t.change).toBe(51);
        expect(t.problem).toBeNull();
    });

    it('tarjeta y transferencia pueden cubrir todo sin efectivo', () => {
        const t = evaluateMixedTender(249, 200, 49, 0);
        expect(t.cashDue).toBe(0);
        expect(t.change).toBe(0);
        expect(t.problem).toBeNull();
    });

    it('no deja cobrar si lo que no es efectivo excede el total', () => {
        expect(evaluateMixedTender(249, 300, 0, 0).problem).toMatch(/suman más/);
    });

    it('no deja cobrar si falta efectivo', () => {
        expect(evaluateMixedTender(249, 100, 0, 50).problem).toMatch(/Falta efectivo/);
    });

    it('exige capturar al menos un método que no sea efectivo', () => {
        expect(evaluateMixedTender(249, 0, 0, 300).problem).toMatch(/tarjeta o transferencia/);
    });

    it('pagar al centavo exacto no deja cambio fantasma', () => {
        const t = evaluateMixedTender(0.6, 0.1, 0.2, 0.3);
        expect(t.cashDue).toBe(0.3);
        expect(t.change).toBe(0);
    });
});

describe('el efectivo contado al cerrar', () => {
    it('un campo vacío no es un cero', () => {
        // Valía cero: se cerraba el turno sin contar y quedaba un faltante por
        // todo lo esperado, en un corte que no se puede reabrir.
        expect(montoContado('')).toBeNull();
        expect(montoContado('   ')).toBeNull();
    });

    it('lo que no es un número tampoco', () => {
        expect(montoContado('abc')).toBeNull();
        expect(montoContado('-50')).toBeNull();
    });

    it('un cero escrito a propósito sí cuenta', () => {
        expect(montoContado('0')).toBe(0);
    });

    it('acepta separador de miles y redondea al centavo', () => {
        expect(montoContado('1,250.5')).toBe(1250.5);
        expect(montoContado('99.999')).toBe(100);
    });
});

describe('la fórmula del efectivo esperado', () => {
    it('usa exactamente los mismos renglones que el backend', () => {
        // Está duplicada a propósito —la pantalla tiene que mostrar el esperado
        // antes de que el backend conteste— y esa duplicación es el riesgo: si una
        // de las dos deriva, el cajero cuenta contra un número y el corte queda
        // con otro, en un corte que ya no se puede reabrir. Las pruebas espejo no
        // atan nada: si alguien agrega una columna al turno y solo toca un lado,
        // las dos suites siguen verdes. Esto lee el Rust.
        const rust = readFileSync('src-tauri/src/commands/cash_register.rs', 'utf-8');
        const cuerpo = rust.slice(rust.indexOf('pub fn expected_cash'));
        const hasta = cuerpo.indexOf('\n}');
        const formula = cuerpo.slice(0, hasta > 0 ? hasta : undefined);

        // Los campos del turno que entran en la cuenta, con su signo.
        const delBackend = [...formula.matchAll(/([+-])\s*p\(r\.([a-z_]+)\)/g)]
            .map(m => `${m[1]}${m[2]}`);
        // El primero va sin signo en el Rust: `p(r.opening_amount) + ...`.
        const primero = formula.match(/=\s*p\(r\.([a-z_]+)\)/);
        expect(primero, 'no se encontró el primer término de expected_cash').not.toBeNull();
        const esperadoEnRust = new Set([`+${primero![1]}`, ...delBackend]);

        const columnas: Record<string, keyof CashRegister> = {
            'Fondo de apertura': 'opening_amount',
            'Ventas en efectivo': 'total_cash_sales',
            'Abonos de apartados': 'total_layaway_cash',
            'Devoluciones en efectivo': 'total_refunds_cash',
            'Gastos': 'total_expenses',
        };
        // Valores distintos por columna: así el signo se deduce del renglón y no
        // de que dos ceros se parezcan.
        const turno = caja({
            opening_amount: 11, total_cash_sales: 22, total_layaway_cash: 33,
            total_refunds_cash: 44, total_expenses: 55,
        });
        const dePantalla = new Set(
            cashBreakdown(turno).map(l => {
                const campo = columnas[l.label];
                if (!campo) throw new Error(`renglón sin columna conocida: ${l.label}`);
                // El signo sale de cómo lo arma `cashBreakdown`, no de los datos.
                const positivo = l.amount === (turno[campo] as number);
                return `${positivo ? '+' : '-'}${campo}`;
            }),
        );

        expect([...dePantalla].sort()).toEqual([...esperadoEnRust].sort());
    });
});
