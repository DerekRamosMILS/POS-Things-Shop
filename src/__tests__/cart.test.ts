import { describe, expect, it, beforeEach } from 'vitest';
import { useCartStore, lineIdOf, stockOf, cartLineId } from '../stores/useCartStore';
import type { CartVariant, Product } from '../types';

const product = (over: Partial<Product> = {}): Product => ({
    id: 1, sku: 'CAM-001', barcode: null, name: 'Camisa', description: null,
    category_id: 1, category_name: 'Ropa', supplier_id: null, supplier_name: null,
    purchase_price: 100, sale_price: 249, stock: 10, min_stock: 2,
    image_url: null, is_active: true, has_variants: false,
    created_at: '', updated_at: '',
    ...over,
} as Product);

const variant = (over: Partial<CartVariant> = {}): CartVariant =>
    ({ id: 5, size: 'M', color: 'Negro', stock: 3, ...over } as CartVariant);

describe('identidad de línea', () => {
    it('separa el mismo producto en líneas distintas por variante', () => {
        expect(lineIdOf(1, null)).toBe('p1');
        expect(lineIdOf(1, 5)).toBe('1:5');
        expect(lineIdOf(1, 5)).not.toBe(lineIdOf(1, 6));
    });

    it('el stock de una línea con variante es el de la variante', () => {
        const item = { product: product({ stock: 10 }), variant: variant({ stock: 3 }), quantity: 1, discount: 0 };
        expect(stockOf(item)).toBe(3);
        expect(cartLineId(item)).toBe('1:5');
    });
});

describe('carrito', () => {
    beforeEach(() => useCartStore.getState().clear());

    it('agrupa el mismo producto en una sola línea', () => {
        const { addItem } = useCartStore.getState();
        addItem(product());
        addItem(product());

        const { items, getItemCount } = useCartStore.getState();
        expect(items).toHaveLength(1);
        expect(getItemCount()).toBe(2);
    });

    it('no deja vender más unidades de las que hay en stock', () => {
        const { addItem } = useCartStore.getState();
        const p = product({ stock: 2 });
        addItem(p); addItem(p); addItem(p);

        expect(useCartStore.getState().items[0].quantity).toBe(2);
    });

    it('respeta el stock de la variante, no el del producto', () => {
        const { addItem } = useCartStore.getState();
        const p = product({ stock: 100 });
        const v = variant({ stock: 2 });
        addItem(p, v); addItem(p, v); addItem(p, v);

        expect(useCartStore.getState().items[0].quantity).toBe(2);
    });

    it('el total descuenta lo aplicado por línea', () => {
        const { addItem, applyDiscount } = useCartStore.getState();
        addItem(product());
        addItem(product());
        applyDiscount('p1', 50);

        const s = useCartStore.getState();
        expect(s.getSubtotal()).toBe(498);
        expect(s.getDiscountTotal()).toBe(50);
        expect(s.getTotal()).toBe(448);
    });

    it('quitar una línea la saca del total', () => {
        const { addItem, removeItem } = useCartStore.getState();
        addItem(product());
        removeItem('p1');

        expect(useCartStore.getState().items).toHaveLength(0);
        expect(useCartStore.getState().getTotal()).toBe(0);
    });
});

describe('descuento y cantidad', () => {
    beforeEach(() => useCartStore.getState().clear());

    it('al bajar la cantidad el descuento no puede valer más que la línea', () => {
        const { addItem, updateQuantity, applyDiscount } = useCartStore.getState();
        const p = product({ sale_price: 100, stock: 10 });
        addItem(p);
        updateQuantity('p1', 3);
        applyDiscount('p1', 250);

        updateQuantity('p1', 1);

        const linea = useCartStore.getState().items[0];
        expect(linea.discount).toBe(100);
        expect(useCartStore.getState().getTotal()).toBe(0);
    });

    it('subir la cantidad deja el descuento como estaba', () => {
        const { addItem, updateQuantity, applyDiscount } = useCartStore.getState();
        addItem(product({ sale_price: 100, stock: 10 }));
        applyDiscount('p1', 40);
        updateQuantity('p1', 4);

        expect(useCartStore.getState().items[0].discount).toBe(40);
        expect(useCartStore.getState().getTotal()).toBe(360);
    });
});
