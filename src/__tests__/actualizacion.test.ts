/**
 * Cuándo se puede reiniciar la aplicación para instalar una actualización.
 *
 * En Windows el instalador mata la app para poder reemplazarla, así que esta
 * decisión es lo único que separa "se actualiza solo" de "se cerró a media
 * venta". Es la pieza que hay que tener amarrada.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const check = vi.fn();
vi.mock('@tauri-apps/plugin-updater', () => ({ check: (...a: unknown[]) => check(...a) }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));
vi.mock('../api', () => ({ registrarEventoActualizacion: vi.fn(async () => {}) }));

import { esMomentoSeguro, motivoDeEspera, useActualizacionStore } from '../stores/useActualizacionStore';
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
        vi.useFakeTimers({ shouldAdvanceTime: true });
        useCartStore.getState().clear();
        useSessionStore.setState({ user: null, token: null, cashRegisterId: null });
    });

    afterEach(() => { vi.useRealTimers(); });

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

    it('recién arrancada instala aunque el turno siga abierto', () => {
        // EL BUG QUE SE REPORTÓ. La sesión y el turno sobreviven a cerrar la
        // aplicación: al reabrirla el usuario ya está dentro y la caja sigue
        // abierta. Pedir "que no haya turno abierto" en el arranque era pedir algo
        // que no pasa nunca en una tienda que deja la caja abierta todo el día, y
        // la actualización se quedaba esperando un momento que no llegaba.
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });

        expect(motivoDeEspera()).toBeNull();
        expect(esMomentoSeguro()).toBe(true);
    });

    it('pasada la ventana de arranque sí respeta el turno abierto', () => {
        // Ya en plena jornada, con gente trabajando, se vuelve conservador.
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });
        vi.setSystemTime(Date.now() + 10 * 60 * 1000);

        expect(motivoDeEspera()).toContain('turno abierto');
        expect(esMomentoSeguro()).toBe(false);
    });

    it('un ticket a medias manda incluso recién arrancada', () => {
        // Lo único que nunca se puede interrumpir.
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });
        useCartStore.getState().addItem(producto());

        expect(motivoDeEspera()).toContain('ticket a medias');
        expect(esMomentoSeguro()).toBe(false);
    });

    it('el motivo se puede leer, para poder decirlo en Ajustes', () => {
        // Una actualización que espera sin explicar por qué es indistinguible de
        // una que no llegó, y eso a distancia no se depura.
        vi.setSystemTime(Date.now() + 10 * 60 * 1000);
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });
        expect(motivoDeEspera()).toBe('Hay un turno abierto; se instala al cerrar la caja');

        useCartStore.getState().addItem(producto());
        expect(motivoDeEspera()).toBe('Hay un ticket a medias');
    });

    it('sí en cuanto se cierra la caja', () => {
        vi.setSystemTime(Date.now() + 10 * 60 * 1000);
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });
        expect(esMomentoSeguro()).toBe(false);

        // Cerrar el turno pone el id en null: ahí entra la actualización.
        useSessionStore.setState({ cashRegisterId: null });
        expect(esMomentoSeguro()).toBe(true);
    });

    it('en plena jornada, vaciar el ticket sin cerrar la caja no alcanza', () => {
        // Justo después de cobrar el carrito queda vacío, pero el turno sigue
        // abierto y va a llegar la siguiente clienta. No es el momento.
        vi.setSystemTime(Date.now() + 10 * 60 * 1000);
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

describe('el botón de actualizar a mano', () => {
    beforeEach(() => {
        check.mockReset();
        useCartStore.getState().clear();
        useSessionStore.setState({ user: null, token: null, cashRegisterId: null });
        useActualizacionStore.setState({ fase: 'inactivo', version: null, error: null, ultimaRevision: null });
    });

    /** Una actualización de mentira que anota lo que le piden. */
    function actualizacionFalsa() {
        const hecho = { descargada: false, instalada: false };
        check.mockResolvedValue({
            version: '9.9.9',
            download: async () => { hecho.descargada = true; },
            install: async () => { hecho.instalada = true; },
        });
        return hecho;
    }

    it('instala aunque la política diga que hay que esperar', async () => {
        // Lo pide una persona que está mirando la pantalla: la política, que
        // existe para decidir sola, no tiene nada que opinar. Es la salida para
        // cuando la automática se queda esperando por algo que no previmos.
        const hecho = actualizacionFalsa();
        useSessionStore.setState({ user: cajera(), token: 't', cashRegisterId: 7 });
        useCartStore.getState().addItem(producto());
        expect(esMomentoSeguro()).toBe(false);

        await useActualizacionStore.getState().forzar();

        expect(hecho.descargada).toBe(true);
        expect(hecho.instalada).toBe(true);
    });

    it('cuando ya está al día lo dice y no instala nada', async () => {
        check.mockResolvedValue(null);

        const mensaje = await useActualizacionStore.getState().forzar();

        expect(mensaje).toContain('más reciente');
    });

    it('cuando falla la búsqueda devuelve el motivo, no un silencio', async () => {
        // "No se actualizó" sin explicación es lo que nos costó dos rondas.
        check.mockRejectedValue(new Error('Network unreachable'));

        const mensaje = await useActualizacionStore.getState().forzar();

        expect(mensaje).toContain('Network unreachable');
    });
});
