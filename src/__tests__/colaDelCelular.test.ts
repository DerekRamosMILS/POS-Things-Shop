/**
 * La cola del teléfono: lo capturado vive ahí hasta que la caja lo recibe.
 *
 * Es la pieza donde perder algo sería peor, porque cuando se usa la caja está
 * apagada y no hay a quién preguntarle. El JavaScript vive dentro de una cadena
 * de Rust, así que aquí se carga la página tal cual y se maneja como la
 * manejaría alguien en el mostrador.
 */
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IDBFactory, IDBKeyRange } from 'fake-indexeddb';

const fuente = readFileSync('src-tauri/src/capture/page.rs', 'utf-8');
const HTML = fuente.slice(
    fuente.indexOf('r####"') + 'r####"'.length,
    fuente.lastIndexOf('"####;'),
);

type Envio = { captura_id: string; nombre: string };

interface Caja {
    /** Lo que la caja recibió, en orden. */
    recibidos: Envio[];
    /** Cuando es false, la caja está apagada: todo intento falla. */
    encendida: boolean;
    /** Cuando se pone, la caja contesta que no con este mensaje. */
    rechazaCon: string | null;
}

/** Abre la página con una caja simulada y devuelve con qué jugar. */
async function abrirCaptura() {
    const caja: Caja = { recibidos: [], encendida: true, rechazaCon: null };
    const skus = { n: 0 };

    const fetchFalso = vi.fn(async (url: string, init?: { body?: string }) => {
        if (!caja.encendida) throw new TypeError('Failed to fetch');
        if (String(url).includes('/api/verificar')) {
            return { ok: true, json: async () => ({ ok: true }) };
        }
        const enviado = JSON.parse(init?.body ?? '{}') as Envio;
        if (caja.rechazaCon) {
            return { ok: false, json: async () => ({ ok: false, mensaje: caja.rechazaCon }) };
        }
        caja.recibidos.push(enviado);
        // La caja no da de alta dos veces lo que ya conoce: devuelve el mismo
        // código, igual que el servidor de verdad.
        const previo = caja.recibidos.find(
            (r, i) => r.captura_id === enviado.captura_id && i < caja.recibidos.length - 1,
        );
        if (previo) {
            return { ok: true, json: async () => ({ ok: true, sku: 'TS-repetido' }) };
        }
        skus.n += 1;
        return { ok: true, json: async () => ({ ok: true, sku: `TS-${String(skus.n).padStart(6, '0')}` }) };
    });

    const dom = new JSDOM(HTML, {
        runScripts: 'dangerously',
        url: 'https://192.168.0.10:7423/?c=123456',
        beforeParse(win) {
            const w = win as unknown as Record<string, unknown>;
            w.fetch = fetchFalso;
            // Cada prueba con su propia base, para que no se pisen.
            w.indexedDB = new IDBFactory();
            w.IDBKeyRange = IDBKeyRange;
        },
    });

    const doc = dom.window.document;
    await reposar();
    return { dom, doc, caja, fetchFalso };
}

/** Deja correr las promesas pendientes de IndexedDB y de la red simulada. */
const reposar = (ms = 40) => new Promise(r => setTimeout(r, ms));

/**
 * Espera a que algo se cumpla, en vez de a que pasen N milisegundos.
 *
 * Con un plazo fijo la prueba pasa en una máquina descansada y falla en una
 * ocupada, que es la peor clase de prueba: la que se ignora por costumbre.
 */
async function esperarA(condicion: () => boolean, queEsperaba: string, limiteMs = 3000) {
    const hasta = Date.now() + limiteMs;
    while (Date.now() < hasta) {
        if (condicion()) return;
        await reposar(10);
    }
    throw new Error(`Nunca ocurrió: ${queEsperaba}`);
}

function escribir(doc: Document, id: string, valor: string) {
    (doc.getElementById(id) as HTMLInputElement).value = valor;
}

async function capturar(doc: Document, nombre: string) {
    escribir(doc, 'nombre', nombre);
    escribir(doc, 'precio', '499');
    doc.getElementById('guardar')!.dispatchEvent(new doc.defaultView!.Event('click'));
    // El formulario se limpia cuando la captura ya está guardada en el teléfono.
    await esperarA(
        () => (doc.getElementById('nombre') as HTMLInputElement).value === '',
        `que se guardara "${nombre}"`,
    );
}

describe('capturar con la caja apagada', () => {
    beforeEach(() => vi.restoreAllMocks());

    it('lo capturado no se pierde y se manda al prenderla', async () => {
        const { doc, caja } = await abrirCaptura();
        caja.encendida = false;

        await capturar(doc, 'Vestido amarillo');
        await capturar(doc, 'Blusa roja');

        expect(caja.recibidos).toEqual([]);
        await esperarA(
            () => doc.getElementById('pendientesTexto')!.textContent!.includes('2 productos esperando'),
            'que avisara de los dos pendientes',
        );

        // Llega la mañana y alguien prende la computadora.
        caja.encendida = true;
        doc.getElementById('sincronizar')!.dispatchEvent(new doc.defaultView!.Event('click'));
        await esperarA(() => caja.recibidos.length === 2, 'que se mandaran los dos');

        // En el orden en que se capturaron: así los códigos de producto siguen
        // el orden en que se fotografió la mercancía.
        expect(caja.recibidos.map(r => r.nombre)).toEqual(['Vestido amarillo', 'Blusa roja']);
        expect(doc.getElementById('pendientes')!.style.display).toBe('none');
    });

    it('mandar dos veces no manda dos veces lo mismo', async () => {
        // Quien esté esperando le va a dar al botón otra vez. La cola ya está
        // vacía, así que no hay nada que reenviar.
        const { doc, caja } = await abrirCaptura();
        await capturar(doc, 'Falda');
        await esperarA(() => caja.recibidos.length === 1, 'que se mandara la falda');

        doc.getElementById('sincronizar')!.dispatchEvent(new doc.defaultView!.Event('click'));
        await reposar(150);

        expect(caja.recibidos).toHaveLength(1);
    });

    it('cada captura lleva su propio identificador', async () => {
        const { doc, caja } = await abrirCaptura();
        await capturar(doc, 'Playera');
        await capturar(doc, 'Playera');
        await esperarA(() => caja.recibidos.length === 2, 'que se mandaran las dos');

        const ids = caja.recibidos.map(r => r.captura_id);
        expect(ids[0]).toBeTruthy();
        expect(ids[0]).not.toBe(ids[1]);
    });

    it('si la caja dice que no, no se pierde: se queda en la cola', async () => {
        // Un código vencido, por ejemplo. Reintentar no ayudaría, pero tirar lo
        // capturado sería mucho peor.
        const { doc, caja } = await abrirCaptura();
        caja.encendida = false;
        await capturar(doc, 'Chamarra');

        caja.encendida = true;
        caja.rechazaCon = 'Código incorrecto';
        doc.getElementById('sincronizar')!.dispatchEvent(new doc.defaultView!.Event('click'));
        await esperarA(
            () => doc.getElementById('aviso')!.textContent!.includes('Código incorrecto'),
            'que avisara del código',
        );

        expect(doc.getElementById('pendientesTexto')!.textContent).toContain('1 producto');

        // Y cuando el código se arregla, sale.
        caja.rechazaCon = null;
        doc.getElementById('sincronizar')!.dispatchEvent(new doc.defaultView!.Event('click'));
        await esperarA(() => caja.recibidos.length === 1, 'que se mandara al arreglar el código');
        expect(caja.recibidos.map(r => r.nombre)).toEqual(['Chamarra']);
    });

    it('capturar con la caja apagada deja el formulario listo para el siguiente', async () => {
        const { doc, caja } = await abrirCaptura();
        caja.encendida = false;
        await capturar(doc, 'Suéter');

        expect((doc.getElementById('nombre') as HTMLInputElement).value).toBe('');
    });
});
