import { afterEach, describe, expect, it, beforeEach, vi } from 'vitest';
import {
    formatCurrency,
    formatDate,
    hoyLocal,
    fechaLocal,
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

describe('la fecha de hoy', () => {
    afterEach(() => vi.useRealTimers());

    /// Las siete de la tarde en México ya es el día siguiente en UTC. La caja
    /// compara siempre contra la fecha local —`date('now','localtime')` en el
    /// backend—, así que sacar "hoy" de `toISOString()` daba mañana: una
    /// promoción creada después de las seis arrancaba al día siguiente y ni
    /// aparecía en el punto de venta, sin un mensaje que lo dijera.
    it('es la local, no la de UTC', () => {
        // 2026-09-21 19:30 en UTC-6 = 2026-09-22 01:30 UTC.
        vi.useFakeTimers();
        vi.setSystemTime(new Date('2026-09-22T01:30:00Z'));

        const local = new Date().toLocaleDateString('en-CA');
        expect(hoyLocal()).toBe(local);
        if (new Date().getTimezoneOffset() > 0) {
            expect(hoyLocal()).not.toBe(new Date().toISOString().slice(0, 10));
        }
    });

    it('corre los días en local', () => {
        vi.useFakeTimers();
        vi.setSystemTime(new Date('2026-09-22T01:30:00Z'));

        const hoy = hoyLocal();
        expect(fechaLocal(0)).toBe(hoy);
        // Treinta días atrás desde el día local, no desde el de UTC.
        const esperado = new Date();
        esperado.setDate(esperado.getDate() - 30);
        expect(fechaLocal(-30)).toBe(esperado.toLocaleDateString('en-CA'));
    });
});

describe('formatDate', () => {
    it('no corre un día las fechas sin hora', () => {
        // `new Date('2026-09-21')` es medianoche UTC, que en México es el 20 a
        // las seis de la tarde: la fecha se mostraba un día antes.
        expect(formatDate('2026-09-21')).toBe('21/09/2026');
    });

    it('sigue sirviendo para una fecha con hora', () => {
        expect(formatDate('2026-09-21 14:30:00')).toBe('21/09/2026');
    });
});
