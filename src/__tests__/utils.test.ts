import { describe, expect, it, beforeEach } from 'vitest';
import {
    formatCurrency,
    setCurrencySymbol,
    PAYMENT_METHOD_LABELS,
    STATUS_LABELS,
    cn,
} from '../utils';

describe('formatCurrency', () => {
    beforeEach(() => setCurrencySymbol('$'));

    it('siempre muestra dos decimales', () => {
        expect(formatCurrency(249)).toBe('$249.00');
        expect(formatCurrency(1234.5)).toBe('$1,234.50');
    });

    it('no rompe con valores no finitos', () => {
        expect(formatCurrency(NaN)).toBe('$0.00');
        expect(formatCurrency(Infinity)).toBe('$0.00');
    });

    it('respeta el símbolo configurado en Ajustes', () => {
        setCurrencySymbol('MXN ');
        expect(formatCurrency(10)).toBe('MXN10.00');
    });

    it('ignora un símbolo vacío en vez de dejar el precio sin marca', () => {
        setCurrencySymbol('€');
        setCurrencySymbol('   ');
        expect(formatCurrency(10)).toBe('€10.00');
    });
});

describe('etiquetas', () => {
    it('el pago mixto tiene nombre propio en los listados', () => {
        expect(PAYMENT_METHOD_LABELS.mixed).toBe('Pago mixto');
    });

    it('cubre todos los estados que el backend puede devolver', () => {
        expect(Object.keys(STATUS_LABELS).sort()).toEqual(['cancelled', 'completed', 'returned']);
    });
});

describe('cn', () => {
    it('descarta clases condicionales apagadas', () => {
        expect(cn('btn', false, undefined, 'btn-primary')).toBe('btn btn-primary');
    });
});
