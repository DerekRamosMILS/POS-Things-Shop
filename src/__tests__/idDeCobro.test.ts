/**
 * El identificador que evita cobrar dos veces lo mismo, sin impedir cobrar algo
 * distinto.
 */
import { describe, expect, it } from 'vitest';
import { firmaDelCobro, identidadParaCobrar } from '../utils/idDeCobro';
import type { CartItem, Product } from '../types';

const producto = (id: number): Product => ({
    id, sku: `P-${id}`, barcode: null, name: `P${id}`, description: null,
    category_id: null, category_name: null, supplier_id: null, supplier_name: null,
    purchase_price: 50, sale_price: 100, stock: 10, min_stock: 0,
    image_url: null, is_active: true, has_variants: false, created_at: '', updated_at: '',
} as Product);

const renglon = (id: number, quantity = 1, discount = 0): CartItem =>
    ({ product: producto(id), variant: null, quantity, discount }) as CartItem;

let n = 0;
const nuevo = () => `id-${++n}`;

describe('el identificador del cobro', () => {
    it('reintentar lo mismo conserva el identificador', () => {
        const firma = firmaDelCobro([renglon(1)], null, null, false);
        const primero = identidadParaCobrar(null, firma, nuevo);
        const reintento = identidadParaCobrar(primero, firmaDelCobro([renglon(1)], null, null, false), nuevo);
        expect(reintento.id).toBe(primero.id);
    });

    it('agregar un artículo después de un error da un identificador nuevo', () => {
        const primero = identidadParaCobrar(null, firmaDelCobro([renglon(1)], null, null, false), nuevo);
        const otro = identidadParaCobrar(primero, firmaDelCobro([renglon(1), renglon(2)], null, null, false), nuevo);
        expect(otro.id).not.toBe(primero.id);
    });

    it('cambiar cantidad, descuento, promoción o cliente también cuenta como otro cobro', () => {
        const base = firmaDelCobro([renglon(1, 1, 0)], null, null, false);
        for (const distinta of [
            firmaDelCobro([renglon(1, 2, 0)], null, null, false),
            firmaDelCobro([renglon(1, 1, 10)], null, null, false),
            firmaDelCobro([renglon(1, 1, 0)], 3, null, false),
            firmaDelCobro([renglon(1, 1, 0)], null, 9, false),
            firmaDelCobro([renglon(1, 1, 0)], null, null, true),
        ]) {
            expect(distinta).not.toBe(base);
        }
    });

    it('el orden de los renglones no cambia la compra', () => {
        expect(firmaDelCobro([renglon(1), renglon(2)], null, null, false))
            .toBe(firmaDelCobro([renglon(2), renglon(1)], null, null, false));
    });
});
