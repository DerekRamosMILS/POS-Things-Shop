import { describe, expect, it } from 'vitest';
import { cashBreakdown, evaluateMixedTender, expectedCash, round2, sumMoney } from '../utils/cash';
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
