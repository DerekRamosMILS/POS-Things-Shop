/**
 * Un carrito guardado se pone al día antes de cobrarse.
 *
 * El cobro toma el precio de la base y descarta el que manda la pantalla, para
 * que nadie pueda cobrarse de menos. El efecto secundario es que un carrito que
 * sobrevivió a un cambio de precio enseñaba un total y cobraba otro.
 */
import { describe, expect, it } from 'vitest';
import { avisoDeCarrito, refrescarCarrito } from '../utils/carrito';
import type { CartItem, Product } from '../types';

function producto(id: number, nombre: string, precio: number, stock: number): Product {
    return {
        id, sku: `TS-${id}`, barcode: null, name: nombre, description: null,
        category_id: null, supplier_id: null, purchase_price: 1, sale_price: precio,
        stock, min_stock: 0, is_active: true, low_stock_ignored: false,
        created_at: '', updated_at: '', category_name: null, supplier_name: null,
        image_url: null, has_image: false, has_variants: false,
    } as Product;
}

function linea(p: Product, cantidad = 1, discount = 0): CartItem {
    return { product: p, variant: null, quantity: cantidad, discount };
}

describe('el carrito guardado', () => {
    it('toma el precio de ahora y lo dice', () => {
        const viejo = producto(1, 'Vestido', 249, 10);
        const ahora = producto(1, 'Vestido', 299, 10);

        const { items, cambios } = refrescarCarrito([linea(viejo, 2)], [ahora]);

        expect(items[0].product.sale_price).toBe(299);
        expect(cambios).toEqual([{ nombre: 'Vestido', motivo: 'precio', antes: 249, ahora: 299 }]);
        expect(avisoDeCarrito(cambios)).toContain('cambió de precio');
    });

    it('saca del ticket lo que se dio de baja', () => {
        const viejo = producto(1, 'Blusa', 100, 5);

        const { items, cambios } = refrescarCarrito([linea(viejo)], []);

        expect(items).toEqual([]);
        expect(cambios[0].motivo).toBe('baja');
        expect(avisoDeCarrito(cambios)).toContain('ya no está disponible');
    });

    it('saca lo que se quedó sin existencia', () => {
        const viejo = producto(1, 'Falda', 100, 3);
        const ahora = producto(1, 'Falda', 100, 0);

        const { items, cambios } = refrescarCarrito([linea(viejo, 2)], [ahora]);

        expect(items).toEqual([]);
        expect(cambios[0].motivo).toBe('sin-stock');
    });

    it('recorta la cantidad a lo que queda', () => {
        const viejo = producto(1, 'Falda', 100, 10);
        const ahora = producto(1, 'Falda', 100, 2);

        const { items } = refrescarCarrito([linea(viejo, 6)], [ahora]);

        expect(items[0].quantity).toBe(2);
    });

    it('no deja un descuento mayor que el renglón cuando el precio baja', () => {
        const viejo = producto(1, 'Saco', 1000, 5);
        const ahora = producto(1, 'Saco', 200, 5);

        const { items } = refrescarCarrito([linea(viejo, 1, 500)], [ahora]);

        expect(items[0].discount).toBe(200);
        expect(items[0].product.sale_price * items[0].quantity - items[0].discount).toBe(0);
    });

    it('a un renglón con talla no le toca el tope del producto', () => {
        const viejo = producto(1, 'Vestido', 249, 10);
        viejo.has_variants = true;
        const ahora = producto(1, 'Vestido', 249, 0); // el total del producto es la suma de tallas
        ahora.has_variants = true;
        const conTalla: CartItem = {
            product: viejo, variant: { id: 7, size: 'M', color: null, stock: 4 },
            quantity: 2, discount: 0,
        };

        const { items, cambios } = refrescarCarrito([conTalla], [ahora]);

        expect(items).toHaveLength(1);
        expect(items[0].quantity).toBe(2);
        expect(cambios).toEqual([]);
    });

    it('un carrito que no cambió no avisa de nada', () => {
        const p = producto(1, 'Playera', 150, 8);
        const { items, cambios } = refrescarCarrito([linea(p, 3)], [producto(1, 'Playera', 150, 8)]);

        expect(items).toHaveLength(1);
        expect(cambios).toEqual([]);
        expect(avisoDeCarrito(cambios)).toBeNull();
    });
});
