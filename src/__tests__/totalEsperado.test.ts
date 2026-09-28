/**
 * El cobro manda el total que vio el cajero, para que el backend se niegue a
 * registrar otro. Con tarjeta, la terminal ya cobró lo de la pantalla: antes la
 * venta se guardaba con el total del backend y el aviso llegaba después.
 */
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';

const raiz = resolve(__dirname, '../..');
const leer = (r: string) => readFileSync(resolve(raiz, r), 'utf8');

describe('el total que vio el cajero', () => {
    it('viaja en el cobro del punto de venta', () => {
        const pos = leer('src/pages/POSPage.tsx');
        const cobro = pos.slice(pos.indexOf('api.createSale('), pos.indexOf('api.createSale(') + 1500);
        expect(cobro).toMatch(/total_esperado:\s*total\b/);
    });

    it('el backend lo recibe con ese nombre', () => {
        expect(leer('src-tauri/src/models/sale.rs')).toContain('pub total_esperado: Option<f64>');
        expect(leer('src/types/index.ts')).toMatch(/total_esperado\?:\s*number/);
    });
});
