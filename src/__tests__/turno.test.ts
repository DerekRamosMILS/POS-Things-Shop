import { describe, expect, it, beforeEach } from 'vitest';
import { useSessionStore } from '../stores/useSessionStore';
import { useCartStore } from '../stores/useCartStore';
import { useHoldsStore } from '../stores/useHoldsStore';
import type { CartItem, Product, User } from '../types';

const producto = (): Product => ({
    id: 1, sku: 'CAM-001', barcode: null, name: 'Camisa', description: null,
    category_id: 1, category_name: 'Ropa', supplier_id: null, supplier_name: null,
    purchase_price: 100, sale_price: 249, stock: 10, min_stock: 2,
    image_url: null, is_active: true, has_variants: false,
    created_at: '', updated_at: '',
} as Product);

const cajera = (id: number, nombre: string): User => ({
    id, username: nombre, full_name: nombre, role: 'cashier',
    is_active: true, must_change_password: false, created_at: '', updated_at: '',
} as User);

const linea = (): CartItem => ({ product: producto(), variant: null, quantity: 2, discount: 0 });

describe('salir de la sesión', () => {
    beforeEach(() => {
        useCartStore.getState().clear();
        useHoldsStore.getState().setHolds([null, null, null]);
    });

    it('no le deja el ticket a medias a la siguiente persona', () => {
        useSessionStore.getState().setSession(cajera(1, 'ana'), 'tok-ana');
        useCartStore.getState().addItem(producto());
        expect(useCartStore.getState().items).toHaveLength(1);

        useSessionStore.getState().logout();

        expect(useCartStore.getState().items).toHaveLength(0);
    });

    it('tampoco le deja las órdenes en espera', () => {
        useSessionStore.getState().setSession(cajera(1, 'ana'), 'tok-ana');
        useHoldsStore.getState().setHolds([
            { items: [linea()], customerName: 'Lupita', orderNotes: '', serviceType: 'direct', orderNo: 1001, promo: null },
            null,
            null,
        ]);

        useSessionStore.getState().logout();

        expect(useHoldsStore.getState().holds).toEqual([null, null, null]);
    });

    // Las tres salidas terminan en el mismo `logout`: el botón de la barra, la
    // sesión que venció al arrancar y el botón de cambiar contraseña. Antes solo
    // la primera limpiaba, y por las otras dos el ticket ajeno se quedaba.
    it('limpia sin importar por dónde se salga', () => {
        for (const _ of [1, 2, 3]) {
            useSessionStore.getState().setSession(cajera(1, 'ana'), 'tok-ana');
            useCartStore.getState().addItem(producto());
            useSessionStore.getState().logout();
            expect(useCartStore.getState().items).toHaveLength(0);
            expect(useSessionStore.getState().token).toBeNull();
        }
    });

    it('suelta el turno para que el siguiente lo vuelva a preguntar a la base', () => {
        useSessionStore.getState().setSession(cajera(1, 'ana'), 'tok-ana');
        useSessionStore.getState().setCashRegisterId(7);

        useSessionStore.getState().logout();

        expect(useSessionStore.getState().cashRegisterId).toBeNull();
    });
});
