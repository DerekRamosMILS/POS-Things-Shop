/**
 * Cuándo se puede reiniciar la aplicación para instalar una actualización.
 *
 * En Windows el instalador mata la app para poder reemplazarla, así que esta
 * decisión es lo único que separa "se actualiza solo" de "se cerró a media
 * venta". Es la pieza que hay que tener amarrada.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));

import { esMomentoSeguro } from '../stores/useActualizacionStore';
import { useCartStore } from '../stores/useCartStore';
import { useSessionStore } from '../stores/useSessionStore';
import type { Product, User } from '../types';

const producto = (): Product => ({
    id: 1, sku: 'CAM-001', barcode: null, name: 'Camisa', description: null,
    category_id: 1, category_name: 'Ropa', supplier_id: null, supplier_name: null,
    purchase_price: 100, sale_price: 249, stock: 10, min_stock: 2,
    image_url: null, is_active: true, has_variants: false,
    created_at: '', updated_at: '',
} as Product);

const cajera = (): User => ({
    id: 1, username: 'ana', full_name: 'Ana', role: 'cashier',
    is_active: true, must_change_password: false, created_at: '', updated_at: '',
} as User);

describe('el momento de instalar', () => {
    beforeEach(() => {
        useCartStore.getState().clear();
        useSessionStore.setState({ user: null, token: null, cashRegisterId: null });
    });

    it('con nadie dentro es el mejor momento', () => {
        // La ventana limpia: la app acaba de abrir y nadie ha entrado.
        expect(esMomentoSeguro()).toBe(true);
    });

    it('nunca con algo en el ticket', () => {
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: null });
        useCartStore.getState().addItem(producto());

        expect(esMomentoSeguro()).toBe(false);
    });

    it('tampoco con un ticket a medias y nadie con sesión', () => {
        // El carrito sobrevive a cerrar la app: puede haber una venta a medias
        // aunque la sesión se haya cerrado. Eso sigue siendo una venta.
        useCartStore.getState().addItem(producto());

        expect(esMomentoSeguro()).toBe(false);
    });

    it('no en medio de un turno abierto', () => {
        // El corte no se pierde —vive en la base— pero reiniciar deja al
        // mostrador mirando una pantalla que se fue, y eso frente a un cliente no.
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });

        expect(esMomentoSeguro()).toBe(false);
    });

    it('sí en cuanto se cierra la caja', () => {
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });
        expect(esMomentoSeguro()).toBe(false);

        // Cerrar el turno pone el id en null: ahí entra la actualización.
        useSessionStore.setState({ cashRegisterId: null });
        expect(esMomentoSeguro()).toBe(true);
    });

    it('vaciar el ticket sin cerrar la caja no alcanza', () => {
        // Justo después de cobrar el carrito queda vacío, pero el turno sigue
        // abierto y va a llegar la siguiente clienta. No es el momento.
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });
        useCartStore.getState().addItem(producto());
        useCartStore.getState().clear();

        expect(esMomentoSeguro()).toBe(false);
    });

    it('salir de la sesión abre la ventana', () => {
        // Cambio de turno: logout vacía el carrito y suelta la caja.
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });
        useCartStore.getState().addItem(producto());

        useSessionStore.getState().logout();

        expect(esMomentoSeguro()).toBe(true);
    });
});
