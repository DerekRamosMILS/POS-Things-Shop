/**
 * Lo que el teléfono hace cuando el relevo contesta mal.
 *
 * La página se guarda para abrir sin computadora, pero se pide fresca primero.
 * Si el relevo devuelve un 500 —o Cloudflare devuelve su propia página de
 * error—, esa respuesta se entregaba tal cual: el teléfono enseñaba un error en
 * lugar de la captura, y con ella se volvía inalcanzable la cola de lo ya
 * capturado, que vive dentro de la página.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';

type Oyente = (e: unknown) => void;

/**
 * Carga `sw.js` con un entorno de service worker de mentiras.
 *
 * La red se inyecta como función y no como `globalThis.fetch`: el módulo captura
 * lo que se le pasa al cargarlo, así que sustituir el global después no lo toca
 * y todas las pruebas acababan ejercitando la rama de "sin conexión" sin saberlo.
 */
function cargarServiceWorker(guardado: Response | null, red: () => Promise<Response>) {
    const oyentes: Record<string, Oyente> = {};
    const almacen = {
        match: vi.fn(async () => guardado ?? undefined),
        put: vi.fn(async () => undefined),
        add: vi.fn(async () => undefined),
    };
    const caches = {
        open: vi.fn(async () => almacen),
        keys: vi.fn(async () => ['things-shop-captura-v5']),
        match: vi.fn(async () => guardado ?? undefined),
        delete: vi.fn(async () => true),
    };
    const self = {
        addEventListener: (nombre: string, fn: Oyente) => { oyentes[nombre] = fn; },
        skipWaiting: vi.fn(async () => undefined),
        clients: { claim: vi.fn(async () => undefined) },
        caches,
    };

    const codigo = readFileSync('relevo/public/sw.js', 'utf-8');
    // eslint-disable-next-line no-new-func
    new Function('self', 'caches', 'fetch', 'Response', 'URL', 'console', codigo)(
        self, caches, () => red(), Response, URL, console,
    );
    return { oyentes, caches, almacen };
}

/** Dispara el oyente de `fetch` para una navegación y devuelve lo que respondió. */
async function navegar(oyentes: Record<string, Oyente>): Promise<Response> {
    let respuesta: Promise<Response> | null = null;
    const evento = {
        request: { url: 'https://relevo.example/', mode: 'navigate', method: 'GET' },
        respondWith: (r: Promise<Response>) => { respuesta = r; },
    };
    oyentes.fetch(evento);
    if (!respuesta) throw new Error('el service worker no respondió a la navegación');
    return await respuesta;
}

describe('la página guardada en el teléfono', () => {
    it('se usa cuando el relevo contesta con un error del servidor', async () => {
        const guardada = new Response('<html>la captura</html>', {
            status: 200, headers: { 'content-type': 'text/html' },
        });
        const { oyentes } = cargarServiceWorker(
            guardada,
            async () => new Response('Error 1101', { status: 500 }),
        );

        const r = await navegar(oyentes);

        expect(await r.text()).toContain('la captura');
    });

    it('no se guarda una respuesta con error', async () => {
        const { oyentes, almacen } = cargarServiceWorker(
            null,
            async () => new Response('Error 1101', { status: 500 }),
        );

        await navegar(oyentes);

        expect(almacen.put).not.toHaveBeenCalled();
    });

    it('sin nada guardado explica qué hacer, en vez de dejar el error crudo', async () => {
        const { oyentes } = cargarServiceWorker(
            null,
            async () => new Response('Error 1101', { status: 500 }),
        );

        const r = await navegar(oyentes);

        expect(await r.text()).toContain('internet');
    });

    it('la respuesta buena se entrega y se guarda', async () => {
        const { oyentes, almacen } = cargarServiceWorker(
            null,
            async () => new Response('<html>fresca</html>', { status: 200 }),
        );

        const r = await navegar(oyentes);

        expect(await r.text()).toContain('fresca');
        await new Promise(res => setTimeout(res, 10));
        expect(almacen.put).toHaveBeenCalled();
    });

    it('sin red también usa la guardada', async () => {
        const guardada = new Response('<html>la captura</html>', { status: 200 });
        const { oyentes } = cargarServiceWorker(guardada, async () => { throw new Error('sin red'); });

        const r = await navegar(oyentes);

        expect(await r.text()).toContain('la captura');
    });
});
