/**
 * Lo que el teléfono dice sobre si va a funcionar con la computadora apagada.
 *
 * Antes el fallo del guardado se iba a `console.warn`, que en un celular no ve
 * nadie: la encargada instalaba todo, se llevaba el teléfono y descubría que no
 * servía cuando ya no había forma de arreglarlo. Estas pruebas son sobre eso —
 * que la página lo diga a la cara, mientras todavía se está a tiempo.
 */
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { describe, expect, it, vi } from 'vitest';

const fuente = readFileSync('src-tauri/src/capture/page.rs', 'utf-8');
const HTML = fuente.slice(
    fuente.indexOf('r####"') + 'r####"'.length,
    fuente.lastIndexOf('"####;'),
);

const reposar = (ms = 60) => new Promise(r => setTimeout(r, ms));

async function esperarA(condicion: () => boolean, queEsperaba: string, limiteMs = 3000) {
    const hasta = Date.now() + limiteMs;
    while (Date.now() < hasta) {
        if (condicion()) return;
        await reposar(10);
    }
    throw new Error(`Nunca ocurrió: ${queEsperaba}`);
}

interface Opciones {
    /** Si el navegador sabe de service workers. */
    conServiceWorker?: boolean;
    /** Si el registro del service worker falla, y con qué mensaje. */
    fallaElRegistro?: string;
    /** Si la página ya quedó guardada en algún almacén. */
    yaGuardada?: boolean;
}

/** Abre la página con el guardado simulado según las opciones. */
async function abrir(op: Opciones = {}) {
    const { conServiceWorker = true, fallaElRegistro, yaGuardada = false } = op;

    const dom = new JSDOM(HTML, {
        runScripts: 'dangerously',
        url: 'https://192.168.0.10:7423/?c=123456',
        beforeParse(win) {
            const w = win as unknown as Record<string, unknown>;
            w.fetch = vi.fn(async () => ({ ok: true, status: 200, json: async () => ({ ok: true }) }));
            w.indexedDB = undefined;

            // El almacén de la caché: solo lo que estas pruebas necesitan.
            w.caches = {
                keys: async () => (yaGuardada ? ['things-shop-captura-v4'] : []),
                open: async () => ({ match: async () => (yaGuardada ? { ok: true } : undefined) }),
            };

            if (conServiceWorker) {
                (win.navigator as unknown as Record<string, unknown>).serviceWorker = {
                    register: fallaElRegistro
                        ? async () => { throw new Error(fallaElRegistro); }
                        : async () => ({ installing: null }),
                    ready: Promise.resolve({}),
                };
            }
        },
    });

    const doc = dom.window.document;
    await reposar();
    return { dom, doc };
}

const titulo = (doc: Document) => doc.getElementById('estadoSinConexion')!.textContent || '';
const detalle = (doc: Document) => doc.getElementById('detalleSinConexion')!.textContent || '';
const visible = (doc: Document) => doc.getElementById('tarjetaSinConexion')!.style.display !== 'none';

describe('el aviso de "funciona sin la computadora"', () => {
    it('dice que sí cuando la página ya quedó guardada', async () => {
        const { doc } = await abrir({ yaGuardada: true });

        await esperarA(() => titulo(doc).includes('Lista'), 'que confirmara que está lista');
        expect(visible(doc)).toBe(true);
        expect(titulo(doc)).toContain('Lista para trabajar sin la computadora');
    });

    it('avisa cuando NO quedó guardada, en vez de callarse', async () => {
        // El caso que se descubría demasiado tarde.
        const { doc } = await abrir({ yaGuardada: false });

        await esperarA(() => titulo(doc).includes('NO funciona'), 'que avisara del problema');
        expect(titulo(doc)).toContain('Todavía NO funciona sin la computadora');
    });

    it('enseña el error de verdad cuando el registro falla', async () => {
        // Sin esto el motivo se iba a la consola del teléfono, donde no lo lee
        // nadie, y desde aquí era imposible saber qué había pasado.
        const { doc } = await abrir({ fallaElRegistro: 'SSL certificate error', yaGuardada: false });

        await esperarA(() => detalle(doc).includes('SSL'), 'que mostrara el motivo real');
        expect(detalle(doc)).toContain('no se pudo guardar');
        expect(detalle(doc)).toContain('SSL certificate error');
    });

    it('en un navegador sin service worker lo dice y manda a Chrome', async () => {
        const { doc } = await abrir({ conServiceWorker: false });

        await esperarA(() => titulo(doc).includes('No va a funcionar'), 'que lo dijera');
        expect(detalle(doc)).toContain('Chrome');
    });

    it('estando ya guardada pero en el navegador, pide instalarla', async () => {
        const { doc } = await abrir({ yaGuardada: true });
        await esperarA(() => titulo(doc).includes('Lista'), 'el estado listo');

        // No corre como aplicación instalada: falta el último paso.
        expect(detalle(doc)).toContain('Instálala en la pantalla de inicio');
    });
});
