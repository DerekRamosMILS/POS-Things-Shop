/**
 * Una sesión vencida a media jornada tiene que sacar a la pantalla de inicio,
 * no dejar la caja fallando con el mismo mensaje en cada acción.
 */
import { readFileSync } from 'node:fs';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const tauriInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => tauriInvoke(...a) }));

import { esSesionVencida, getSales } from '../api';
import { useSessionStore } from '../stores/useSessionStore';
import type { User } from '../types';

const cajera = { id: 1, username: 'ana', full_name: 'Ana', role: 'cashier' } as User;

describe('la sesión vencida', () => {
    beforeEach(() => {
        tauriInvoke.mockReset();
        (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
        useSessionStore.setState({ user: cajera, token: 'viejo', cashRegisterId: 3 });
    });

    it('se reconoce por el mensaje del backend', () => {
        expect(esSesionVencida('Sesión inválida o expirada. Vuelve a iniciar sesión.')).toBe(true);
        expect(esSesionVencida('La sesión expiró. Vuelve a iniciar sesión.')).toBe(true);
        expect(esSesionVencida('Stock insuficiente')).toBe(false);
    });

    it('cierra la sesión en la pantalla', async () => {
        tauriInvoke.mockRejectedValue('Sesión inválida o expirada. Vuelve a iniciar sesión.');

        await expect(getSales()).rejects.toContain('Vuelve a iniciar');

        expect(useSessionStore.getState().user).toBeNull();
    });

    it('cualquier otro error no la toca', async () => {
        tauriInvoke.mockRejectedValue('Stock insuficiente');

        await expect(getSales()).rejects.toBe('Stock insuficiente');

        expect(useSessionStore.getState().user).not.toBeNull();
    });
});

describe('el mensaje que la reconoce', () => {
    it('es el que de verdad manda el backend', () => {
        // `esSesionVencida` busca una frase dentro del texto que devuelve Rust.
        // Nada ataba las dos puntas: reescribir el mensaje en `session.rs` dejaba
        // la pantalla sin sacar a nadie a iniciar sesión, en silencio, y esta misma
        // prueba seguía pasando porque tenía los mensajes copiados a mano.
        const rust = readFileSync('src-tauri/src/session.rs', 'utf-8');
        const resolver = rust.slice(rust.indexOf('fn resolver'));
        const fin = resolver.indexOf('\n}');
        const cuerpo = resolver.slice(0, fin > 0 ? fin : undefined);

        const mensajes = [...cuerpo.matchAll(/Err\("([^"]+)"\.to_string\(\)\)/g)].map(m => m[1]);

        expect(mensajes.length).toBeGreaterThanOrEqual(2);
        for (const mensaje of mensajes) {
            expect(esSesionVencida(mensaje), `el backend dice "${mensaje}" y la pantalla no lo reconoce`).toBe(true);
        }
    });
});
