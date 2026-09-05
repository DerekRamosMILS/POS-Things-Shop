/**
 * La página que se abre en el celular vive dentro de una cadena de Rust, así
 * que su JavaScript nunca se ejecutaba en ninguna prueba. Aquí se carga tal
 * cual, se llena el formulario y se revisa lo que sale hacia la caja.
 */
import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const fuente = readFileSync('src-tauri/src/capture/page.rs', 'utf-8');
const HTML = fuente.slice(
    fuente.indexOf('r####"') + 'r####"'.length,
    fuente.lastIndexOf('"####;'),
);

type Enviado = { piezas: { talla: string | null; color: string | null; cantidad: number }[]; existencia: number };

/** Abre la página, deja hacer algo con ella y devuelve lo que se mandó al guardar. */
async function capturar(llenar: (d: Document) => void): Promise<Enviado> {
    const enviados: Enviado[] = [];
    const fetchFalso = vi.fn(async (_url: string, init?: { body?: string }) => {
        if (init?.body) enviados.push(JSON.parse(init.body));
        return {
            ok: true,
            json: async () => ({ ok: true, sku: 'TS-000001' }),
        };
    });

    const dom = new JSDOM(HTML, {
        runScripts: 'dangerously',
        url: 'http://192.168.0.10:7423/?c=123456',
        beforeParse(win) {
            (win as unknown as { fetch: unknown }).fetch = fetchFalso;
        },
    });

    const doc = dom.window.document;
    llenar(doc);
    doc.getElementById('guardar')!.dispatchEvent(new dom.window.Event('click'));
    await new Promise(r => setTimeout(r, 0));

    const guardado = enviados.find(e => 'piezas' in e);
    expect(guardado, 'no se mandó ningún producto').toBeDefined();
    return guardado!;
}

const escribir = (doc: Document, id: string, valor: string) => {
    const el = doc.getElementById(id) as HTMLInputElement;
    el.value = valor;
};

const agregarChip = (doc: Document, input: string, boton: string, valor: string) => {
    escribir(doc, input, valor);
    doc.getElementById(boton)!.dispatchEvent(new doc.defaultView!.Event('click'));
};

describe('captura desde el celular', () => {
    beforeEach(() => vi.restoreAllMocks());

    it('con una sola talla no manda reparto y deja que valgan las piezas de arriba', async () => {
        const enviado = await capturar(doc => {
            escribir(doc, 'nombre', 'Vestido amarillo');
            escribir(doc, 'existencia', '9');
            agregarChip(doc, 'tallaInput', 'addTalla', 'Unitalla');
        });

        expect(enviado.existencia).toBe(9);
        expect(enviado.piezas).toEqual([]);
    });

    it('sin tallas ni colores tampoco manda reparto', async () => {
        const enviado = await capturar(doc => {
            escribir(doc, 'nombre', 'Bufanda');
            escribir(doc, 'existencia', '4');
        });

        expect(enviado.existencia).toBe(4);
        expect(enviado.piezas).toEqual([]);
    });

    it('con varias tallas manda las piezas de cada una, no la cantidad repetida', async () => {
        const enviado = await capturar(doc => {
            escribir(doc, 'nombre', 'Vestido amarillo');
            escribir(doc, 'existencia', '9');
            agregarChip(doc, 'tallaInput', 'addTalla', 'CH');
            agregarChip(doc, 'tallaInput', 'addTalla', 'M');
            agregarChip(doc, 'tallaInput', 'addTalla', 'G');

            const casillas = doc.querySelectorAll<HTMLInputElement>('#lineasReparto input');
            expect(casillas.length).toBe(3);
            [2, 4, 3].forEach((n, i) => {
                casillas[i].value = String(n);
                casillas[i].dispatchEvent(new doc.defaultView!.Event('input', { bubbles: true }));
            });
        });

        expect(enviado.piezas.map(p => p.cantidad)).toEqual([2, 4, 3]);
        expect(enviado.piezas.reduce((a, p) => a + p.cantidad, 0)).toBe(9);
    });

    it('el total que ve quien captura es la suma, no un múltiplo', async () => {
        const dom = new JSDOM(HTML, { runScripts: 'dangerously', url: 'http://x/?c=1', beforeParse(w) {
            (w as unknown as { fetch: unknown }).fetch = vi.fn(async () => ({ ok: true, json: async () => ({}) }));
        } });
        const doc = dom.window.document;

        agregarChip(doc, 'tallaInput', 'addTalla', 'CH');
        agregarChip(doc, 'tallaInput', 'addTalla', 'G');
        agregarChip(doc, 'colorInput', 'addColor', 'Rojo');
        agregarChip(doc, 'colorInput', 'addColor', 'Azul');

        const casillas = doc.querySelectorAll<HTMLInputElement>('#lineasReparto input');
        expect(casillas.length).toBe(4);
        casillas.forEach(c => {
            c.value = '3';
            c.dispatchEvent(new dom.window.Event('input', { bubbles: true }));
        });

        expect(doc.getElementById('totalPiezas')!.textContent).toBe('12 piezas');
    });
});
