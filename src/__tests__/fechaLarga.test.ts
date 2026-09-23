import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { fechaLarga } from '../utils';

describe('fechaLarga', () => {
    it('sube sólo la primera letra', () => {
        expect(fechaLarga(new Date(2026, 8, 23))).toBe('Miércoles, 23 de septiembre de 2026');
    });

    it('no deja preposiciones en mayúscula ningún día del año', () => {
        for (let dia = 0; dia < 365; dia++) {
            const d = new Date(2026, 0, 1 + dia);
            const texto = fechaLarga(d);
            expect(texto, `día ${d.toISOString()}`).not.toMatch(/\bDe\b/);
            expect(texto[0]).toBe(texto[0].toUpperCase());
        }
    });

    it('el dashboard usa el ayudante y no capitalize', () => {
        const pagina = readFileSync(resolve(__dirname, '../pages/DashboardPage.tsx'), 'utf8');
        expect(pagina).toContain('fechaLarga');
        expect(pagina).not.toMatch(/textTransform:\s*'capitalize'/);
    });
});
