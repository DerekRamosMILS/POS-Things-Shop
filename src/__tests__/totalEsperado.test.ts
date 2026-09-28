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

describe('cuando el cobro se frena porque el total cambió', () => {
    // Si el precio cambió con el producto ya en el ticket, la pantalla seguía
    // con el viejo y cada reintento se frenaba igual: el cajero quedaba atorado
    // frente al cliente. La pantalla reconoce el aviso y se pone al día.
    const marca = 'El total cambió';

    it('el backend lo dice con una frase que la pantalla reconoce', () => {
        expect(leer('src-tauri/src/commands/sales.rs')).toContain(`"${marca}:`);
    });

    it('el punto de venta la reconoce y recarga antes de volver a cobrar', () => {
        const pos = leer('src/pages/POSPage.tsx');
        expect(pos).toContain(`'${marca}'`);
        const cobro = pos.slice(pos.indexOf('const handleCompleteSale'), pos.indexOf('const handleCompleteSale') + 6000);
        expect(cobro).toMatch(/ponerAlDia\(/);
    });

    it('el aviso del backend no culpa sólo a la promoción', () => {
        const rust = leer('src-tauri/src/commands/sales.rs');
        const aviso = rust.slice(rust.indexOf(`"${marca}:`), rust.indexOf(`"${marca}:`) + 300);
        expect(aviso).toMatch(/precio/);
    });
});
