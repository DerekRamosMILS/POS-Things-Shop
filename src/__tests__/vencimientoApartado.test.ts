/**
 * La fecha límite de un apartado se pedía al crearlo y no se enseñaba en
 * ningún lado: la tienda no tenía cómo saber qué apartados ya vencieron.
 */
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { vencimientoDeApartado } from '../utils/apartados';

describe('el vencimiento de un apartado', () => {
    const hoy = '2026-09-27';

    it('sin fecha límite no vence', () => {
        expect(vencimientoDeApartado(null, 'active', hoy)).toEqual({ texto: '—', vencido: false });
    });

    it('el último día todavía no está vencido', () => {
        const v = vencimientoDeApartado('2026-09-27', 'active', hoy);
        expect(v.vencido).toBe(false);
        expect(v.texto).toBe('27/09/2026');
    });

    it('al día siguiente sí', () => {
        expect(vencimientoDeApartado('2026-09-26', 'active', hoy).vencido).toBe(true);
    });

    it('uno entregado o cancelado ya no vence', () => {
        expect(vencimientoDeApartado('2026-01-01', 'completed', hoy).vencido).toBe(false);
        expect(vencimientoDeApartado('2026-01-01', 'cancelled', hoy).vencido).toBe(false);
    });

    it('la pantalla de apartados lo enseña', () => {
        const pagina = readFileSync(resolve(__dirname, '../pages/LayawaysPage.tsx'), 'utf8');
        expect(pagina).toContain('vencimientoDeApartado(');
        expect(pagina).toMatch(/<th[^>]*>Vence<\/th>/);
    });
});
