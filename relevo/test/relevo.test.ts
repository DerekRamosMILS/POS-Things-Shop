/**
 * El relevo guarda lo que la tienda capturó y todavía no llega a su casa.
 *
 * Lo que más importa aquí no es que funcione el camino feliz, sino que una
 * tienda no pueda ver ni tocar lo de otra: esto está expuesto a internet abierto
 * y lo único que separa una carpeta de otra es el secreto.
 */
import { env, createExecutionContext, waitOnExecutionContext } from 'cloudflare:test';
import { describe, expect, it } from 'vitest';
import worker from '../src/index';

const URL_BASE = 'https://relevo.ejemplo';

/** Un secreto como los que emite el punto de venta: 32 bytes al azar. */
function secreto(): string {
    const bytes = crypto.getRandomValues(new Uint8Array(32));
    return btoa(String.fromCharCode(...bytes)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

async function llamar(ruta: string, opciones: RequestInit & { secreto?: string } = {}) {
    const { secreto: s, ...resto } = opciones;
    const cabeceras = new Headers(resto.headers);
    if (s) cabeceras.set('authorization', `Bearer ${s}`);
    if (resto.body) cabeceras.set('content-type', 'application/json');

    const req = new Request(`${URL_BASE}${ruta}`, { ...resto, headers: cabeceras });
    const ctx = createExecutionContext();
    const res = await worker.fetch(req, env, ctx);
    await waitOnExecutionContext(ctx);
    return res;
}

const subir = (s: string, id: string, datos: unknown = { nombre: 'Vestido' }) =>
    llamar('/api/subir', { method: 'POST', secreto: s, body: JSON.stringify({ captura_id: id, tipo: 'producto', datos }) });

const pendientes = async (s: string) => (await (await llamar('/api/pendientes', { secreto: s })).json()) as
    { pendientes: { captura_id: string }[] };

describe('quién puede entrar', () => {
    it('sin secreto no se ve nada', async () => {
        expect((await llamar('/api/pendientes')).status).toBe(401);
        expect((await llamar('/api/subir', { method: 'POST', body: '{}' })).status).toBe(401);
    });

    it('un secreto corto se rechaza', async () => {
        // En una red local un código de seis dígitos basta porque hay que estar
        // dentro de la tienda. Esto está en internet abierto: no basta.
        expect((await llamar('/api/verificar', { secreto: '123456' })).status).toBe(401);
        expect((await llamar('/api/verificar', { secreto: 'a'.repeat(39) })).status).toBe(401);
        expect((await llamar('/api/verificar', { secreto: 'a'.repeat(40) })).status).toBe(200);
    });

    it('la comprobación de vida no pide secreto', async () => {
        // Para que el punto de venta distinga "el relevo no responde" de "mi
        // secreto no sirve", que son dos problemas con dos arreglos distintos.
        const res = await llamar('/api/salud');
        expect(res.status).toBe(200);
        expect(await res.json()).toMatchObject({ ok: true });
    });
});

describe('una tienda no ve la de otra', () => {
    it('lo subido con un secreto no aparece con otro', async () => {
        const tienda = secreto();
        const intruso = secreto();
        await subir(tienda, 'suya-1');

        expect((await pendientes(tienda)).pendientes).toHaveLength(1);
        expect((await pendientes(intruso)).pendientes).toHaveLength(0);
    });

    it('ni siquiera sabiendo el identificador se puede bajar', async () => {
        // El identificador no es un secreto —viaja en el listado— así que no
        // puede servir de llave por sí mismo.
        const tienda = secreto();
        const intruso = secreto();
        await subir(tienda, 'identificador-conocido');

        expect((await llamar('/api/pendiente/identificador-conocido', { secreto: tienda })).status).toBe(200);
        expect((await llamar('/api/pendiente/identificador-conocido', { secreto: intruso })).status).toBe(404);
    });

    it('ni borrar lo ajeno', async () => {
        const tienda = secreto();
        const intruso = secreto();
        await subir(tienda, 'no-me-borres');

        await llamar('/api/recibido', { method: 'POST', secreto: intruso, body: JSON.stringify({ ids: ['no-me-borres'] }) });

        expect((await llamar('/api/pendiente/no-me-borres', { secreto: tienda })).status).toBe(200);
    });
});

describe('subir', () => {
    it('reintentar con el mismo identificador no duplica', async () => {
        // El teléfono puede mandar dos veces sin saberlo: se apagó la pantalla,
        // se cayó el WiFi, alguien le dio otra vez a sincronizar.
        const s = secreto();
        await subir(s, 'repetida', { nombre: 'Primera' });
        const otra = await subir(s, 'repetida', { nombre: 'Segunda' });

        expect(await otra.json()).toMatchObject({ repetida: true });
        expect((await pendientes(s)).pendientes).toHaveLength(1);
    });

    it('rechaza un identificador que se salga de su carpeta', async () => {
        const s = secreto();
        for (const malo of ['../otra/cosa', 'con/barra', '', ' ', 'a'.repeat(200)]) {
            const res = await llamar('/api/subir', {
                method: 'POST', secreto: s,
                body: JSON.stringify({ captura_id: malo, tipo: 'producto', datos: {} }),
            });
            expect(res.status, `debió rechazar "${malo}"`).toBe(400);
        }
    });

    it('rechaza una captura vacía o que no es JSON', async () => {
        const s = secreto();
        expect((await llamar('/api/subir', { method: 'POST', secreto: s, body: 'esto no es json' })).status).toBe(400);
        expect((await llamar('/api/subir', {
            method: 'POST', secreto: s, body: JSON.stringify({ captura_id: 'sin-datos', tipo: 'producto' }),
        })).status).toBe(400);
    });

    it('rechaza algo demasiado grande antes de leerlo', async () => {
        // Se mira la cabecera para no tragarse ocho megas y luego decir que no.
        const s = secreto();
        const res = await llamar('/api/subir', {
            method: 'POST', secreto: s,
            headers: { 'content-length': String(50 * 1024 * 1024) },
            body: JSON.stringify({ captura_id: 'enorme', tipo: 'producto', datos: {} }),
        });
        expect(res.status).toBe(413);
    });
});

describe('recoger y borrar', () => {
    it('el punto de venta se lo lleva y después se borra', async () => {
        const s = secreto();
        await subir(s, 'para-llevar', { nombre: 'Blusa roja', precio: 249 });

        const bajada = await llamar('/api/pendiente/para-llevar', { secreto: s });
        expect(await bajada.json()).toMatchObject({ tipo: 'producto', datos: { nombre: 'Blusa roja' } });

        const borrado = await llamar('/api/recibido', { method: 'POST', secreto: s, body: JSON.stringify({ ids: ['para-llevar'] }) });
        expect(await borrado.json()).toMatchObject({ ok: true, borradas: 1 });
        expect((await llamar('/api/pendiente/para-llevar', { secreto: s })).status).toBe(404);
    });

    it('pedir lo que ya no está no es un error del que preocuparse', async () => {
        // El listado de KV tarda en ponerse al día, así que el punto de venta
        // puede pedir algo que acaba de borrar. Tiene que poder seguir de largo.
        const s = secreto();
        expect((await llamar('/api/pendiente/fantasma', { secreto: s })).status).toBe(404);
    });

    it('no se puede pedir el borrado de media tienda de un golpe', async () => {
        const s = secreto();
        const muchos = Array.from({ length: 300 }, (_, i) => `id-${i}`);
        expect((await llamar('/api/recibido', { method: 'POST', secreto: s, body: JSON.stringify({ ids: muchos }) })).status).toBe(400);
        expect((await llamar('/api/recibido', { method: 'POST', secreto: s, body: JSON.stringify({ ids: [] }) })).status).toBe(400);
    });
});

describe('rutas', () => {
    it('lo que no existe contesta 404 y no se cuelga', async () => {
        expect((await llamar('/api/inventada', { secreto: secreto() })).status).toBe(404);
    });

    it('nada de esto se guarda en cachés intermedias', async () => {
        const res = await llamar('/api/salud');
        expect(res.headers.get('cache-control')).toBe('no-store');
    });
});
