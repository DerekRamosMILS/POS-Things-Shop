/**
 * Un error al dibujar la pantalla tiene que dejar rastro.
 *
 * El `ErrorBoundary` evita que la cajera se quede mirando una ventana en blanco,
 * y eso ya funcionaba. Lo que no dejaba era huella: `console.error` solo se ve
 * con las herramientas del navegador abiertas, y en la compilación de producción
 * de la tienda no va a ninguna parte. La caja se recuperaba y nadie se enteraba
 * nunca — ni la bitácora, ni el reporte de diagnóstico que se manda cuando algo
 * anda raro.
 */
import { readFileSync } from 'node:fs';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const tauriInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => tauriInvoke(...a) }));

import { registrarErrorDeInterfaz } from '../api';

describe('el aviso de un error de pantalla', () => {
    beforeEach(() => {
        tauriInvoke.mockReset();
        tauriInvoke.mockResolvedValue(undefined);
        (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    });

    it('llega al backend con el mensaje', async () => {
        await registrarErrorDeInterfaz("Cannot read properties of undefined (reading 'map')");

        expect(tauriInvoke).toHaveBeenCalledWith(
            'registrar_error_de_interfaz',
            expect.objectContaining({ mensaje: expect.stringContaining('undefined') }),
        );
    });

    it('el ErrorBoundary lo manda, además de al console', () => {
        // El cableado vive en un método de ciclo de vida de React que no se puede
        // invocar desde aquí sin montar el árbol; se comprueba que `componentDidCatch`
        // lo llama, que es lo único que se rompió alguna vez.
        const fuente = readFileSync('src/components/ErrorBoundary.tsx', 'utf-8');
        const metodo = fuente.slice(fuente.indexOf('componentDidCatch'));
        const cuerpo = metodo.slice(0, metodo.indexOf('\n    }'));

        expect(cuerpo, 'tiene que anotarlo en la bitácora de la tienda')
            .toMatch(/registrarErrorDeInterfaz/);
        expect(cuerpo, 'y un fallo al anotarlo no puede volver a romper la pantalla')
            .toMatch(/\.catch\(/);
    });

    it('la pantalla de error sigue ofreciendo una salida', () => {
        const fuente = readFileSync('src/components/ErrorBoundary.tsx', 'utf-8');
        expect(fuente).toMatch(/Volver al inicio/);
        expect(fuente).toMatch(/Reintentar/);
    });
});
