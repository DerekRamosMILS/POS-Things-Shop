/**
 * Una promoción creada de tarde tiene que servir esa misma tarde.
 *
 * La pantalla de descuentos propone las fechas y el punto de venta decide si
 * están vigentes; el cobro lo vuelve a decidir en el backend con
 * `date('now','localtime')`. Las tres puntas tienen que hablar de la misma
 * fecha. Sacaba "hoy" de `toISOString()`, que después de las seis de la tarde en
 * México ya es mañana: la promoción arrancaba al día siguiente, no aparecía
 * siquiera en la lista del mostrador, y nada decía por qué.
 */
import { readFileSync } from 'node:fs';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fechaLocal, hoyLocal } from '../utils';

/** La misma regla que aplica el punto de venta. */
function vigenteHoy(start: string, end: string): boolean {
    const hoy = new Date().toLocaleDateString('en-CA');
    if (start && hoy < start) return false;
    if (end && hoy > end) return false;
    return true;
}

describe('una promoción nueva', () => {
    afterEach(() => vi.useRealTimers());

    it('está vigente el mismo día en que se crea, a cualquier hora', () => {
        for (const instante of [
            '2026-09-22T01:30:00Z', // 19:30 del 21 en UTC-6
            '2026-09-22T05:59:00Z', // 23:59 del 21
            '2026-09-22T12:00:00Z', // mediodía del 22
            '2026-01-01T06:30:00Z', // año nuevo recién estrenado
        ]) {
            vi.useFakeTimers();
            vi.setSystemTime(new Date(instante));

            expect(vigenteHoy(hoyLocal(), fechaLocal(30)), `falló en ${instante}`).toBe(true);

            vi.useRealTimers();
        }
    });

    it('la pantalla de descuentos no vuelve a sacar la fecha de UTC', () => {
        // El arreglo vive en una línea; esto es lo que impide que regrese.
        const pantalla = readFileSync('src/pages/DiscountsPage.tsx', 'utf-8');
        expect(pantalla).not.toMatch(/toISOString/);
    });

    it('el punto de venta sigue comparando en local', () => {
        const pos = readFileSync('src/pages/POSPage.tsx', 'utf-8');
        expect(pos).toMatch(/toLocaleDateString\('en-CA'\)/);
    });
});
