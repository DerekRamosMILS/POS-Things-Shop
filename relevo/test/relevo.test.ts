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

describe('el reloj del teléfono', () => {
    it('se guarda junto a la captura, para que la caja corrija el desfase', async () => {
        const s = secreto();
        await llamar('/api/subir', {
            method: 'POST', secreto: s,
            body: JSON.stringify({
                captura_id: 'con-reloj', tipo: 'conteo',
                datos: { conteo_id: 'con-reloj', sku: 'TS-1', contado: 3, contado_en: '2026-01-01 10:00:00' },
                reloj: '2026-01-01 10:05:00',
            }),
        });

        const guardada = await (await llamar('/api/pendiente/con-reloj', { secreto: s })).json() as { reloj: string };
        expect(guardada.reloj).toBe('2026-01-01 10:05:00');
    });

    it('un reloj con mala forma se descarta en vez de viajar', async () => {
        const s = secreto();
        await llamar('/api/subir', {
            method: 'POST', secreto: s,
            body: JSON.stringify({ captura_id: 'reloj-raro', tipo: 'producto', datos: { nombre: 'X' }, reloj: 'ayer' }),
        });

        const guardada = await (await llamar('/api/pendiente/reloj-raro', { secreto: s })).json() as { reloj: string | null };
        expect(guardada.reloj).toBeNull();
    });
});

describe('un código retirado', () => {
    // Aquí no hay registro de secretos: cualquiera tiene su carpeta. Antes de
    // poder retirar uno, un teléfono sin re-emparejar seguía subiendo con el
    // código viejo, el relevo decía "ok", el teléfono borraba su copia, y la
    // tienda ya nunca miraba esa carpeta. Se perdía en silencio.
    it('ya no acepta capturas, para que el teléfono se quede con ellas', async () => {
        const viejo = secreto();
        expect((await llamar('/api/retirar', { method: 'POST', secreto: viejo })).status).toBe(200);

        const res = await subir(viejo, 'despues-de-retirar');
        expect(res.status).toBe(401);
        expect((await llamar('/api/verificar', { secreto: viejo })).status).toBe(401);
        expect((await pendientes(viejo)).pendientes).toHaveLength(0);
    });

    it('deja que la tienda recoja lo que quedó antes de retirarlo', async () => {
        const viejo = secreto();
        await subir(viejo, 'antes-de-retirar');
        await llamar('/api/retirar', { method: 'POST', secreto: viejo });

        expect((await pendientes(viejo)).pendientes.map((p) => p.captura_id)).toEqual(['antes-de-retirar']);
        expect((await llamar('/api/pendiente/antes-de-retirar', { secreto: viejo })).status).toBe(200);
        const recibido = await llamar('/api/recibido', {
            method: 'POST', secreto: viejo, body: JSON.stringify({ ids: ['antes-de-retirar'] }),
        });
        expect(recibido.status).toBe(200);
    });

    it('no afecta a ningún otro código', async () => {
        const viejo = secreto();
        const otro = secreto();
        await llamar('/api/retirar', { method: 'POST', secreto: viejo });
        expect((await subir(otro, 'de-otro')).status).toBe(200);
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

describe('el catálogo para contar', () => {
    const catalogo = [
        { sku: 'TS-000001', nombre: 'Vestido amarillo', variantes: [] },
        { sku: 'TS-000002', nombre: 'Blusa roja', variantes: [{ id: 5, etiqueta: 'M' }] },
    ];
    const publicar = (s: string, productos: unknown) =>
        llamar('/api/catalogo', { method: 'PUT', secreto: s, body: JSON.stringify({ productos }) });
    const leer = async (s: string) =>
        (await (await llamar('/api/catalogo', { secreto: s })).json()) as { productos: unknown[]; cuando: string | null };

    it('la tienda lo publica y el teléfono lo baja', async () => {
        const s = secreto();
        expect((await publicar(s, catalogo)).status).toBe(200);

        const bajado = await leer(s);
        expect(bajado.productos).toEqual(catalogo);
        expect(bajado.cuando).toBeTruthy();
    });

    it('sin catálogo publicado se contesta vacío, no con error', async () => {
        // El teléfono de una tienda recién emparejada tiene que poder decir
        // "todavía no hay catálogo" en vez de fallar.
        const vacio = await leer(secreto());
        expect(vacio.productos).toEqual([]);
        expect(vacio.cuando).toBeNull();
    });

    it('una tienda no ve el catálogo de otra', async () => {
        const tienda = secreto();
        await publicar(tienda, catalogo);
        expect((await leer(secreto())).productos).toEqual([]);
    });

    it('no se mezcla con lo pendiente de recoger', async () => {
        // Viven en prefijos distintos. Si el catálogo apareciera en el listado,
        // el punto de venta intentaría darlo de alta como si fuera un producto.
        const s = secreto();
        await publicar(s, catalogo);
        expect((await pendientes(s)).pendientes).toHaveLength(0);
    });

    it('rechaza algo que no es una lista de productos', async () => {
        const s = secreto();
        expect((await publicar(s, 'no es una lista')).status).toBe(400);
        expect((await llamar('/api/catalogo', { method: 'PUT', secreto: s, body: 'basura' })).status).toBe(400);
    });

    it('sin secreto no se publica ni se lee', async () => {
        expect((await llamar('/api/catalogo')).status).toBe(401);
        expect((await llamar('/api/catalogo', { method: 'PUT', body: JSON.stringify({ productos: [] }) })).status).toBe(401);
    });
});

describe('cuando hay más de una página de pendientes', () => {
    /**
     * Un teléfono que capturó días sin conexión sincroniza de golpe. El listado va
     * de 200 en 200 y el punto de venta sigue el cursor hasta agotarlo; si el
     * cursor no viaja bien, la tienda se queda con las primeras 200 y las demás no
     * aparecen nunca, sin un error que lo diga. Nunca se había probado con más de
     * una página.
     */
    it('el cursor lleva a la siguiente y al final se acaba', async () => {
        const s = secreto();
        const cuantas = 205;
        for (let i = 0; i < cuantas; i++) {
            await subir(s, `c-${String(i).padStart(4, '0')}`);
        }

        const primera = await (await llamar('/api/pendientes', { secreto: s })).json() as
            { pendientes: { captura_id: string }[]; completo: boolean; cursor: string | null };

        expect(primera.pendientes).toHaveLength(200);
        expect(primera.completo, 'con 205 no puede decir que ya terminó').toBe(false);
        expect(primera.cursor, 'tiene que dar por dónde seguir').toBeTruthy();

        const segunda = await (await llamar(
            `/api/pendientes?cursor=${encodeURIComponent(primera.cursor!)}`, { secreto: s },
        )).json() as { pendientes: { captura_id: string }[]; completo: boolean; cursor: string | null };

        expect(segunda.pendientes).toHaveLength(cuantas - 200);
        expect(segunda.completo, 'la última página tiene que decir que ya está').toBe(true);
        expect(segunda.cursor).toBeNull();

        // Y entre las dos páginas están todas, sin repetir ni faltar ninguna.
        const vistas = new Set([
            ...primera.pendientes.map(p => p.captura_id),
            ...segunda.pendientes.map(p => p.captura_id),
        ]);
        expect(vistas.size).toBe(cuantas);
    });

    it('un cursor de otra tienda no abre su carpeta', async () => {
        // El cursor es un dato opaco que viaja por la URL. Si sirviera para leer la
        // carpeta de otro secreto, todo el aislamiento se cae por ahí.
        const a = secreto();
        const b = secreto();
        for (let i = 0; i < 205; i++) await subir(a, `a-${String(i).padStart(4, '0')}`);
        await subir(b, 'solo-de-b');

        const deA = await (await llamar('/api/pendientes', { secreto: a })).json() as { cursor: string | null };
        expect(deA.cursor).toBeTruthy();

        const conCursorAjeno = await (await llamar(
            `/api/pendientes?cursor=${encodeURIComponent(deA.cursor!)}`, { secreto: b },
        )).json() as { pendientes: { captura_id: string }[] };

        for (const p of conCursorAjeno.pendientes) {
            expect(p.captura_id.startsWith('a-'), `se colaron cosas de la otra tienda: ${p.captura_id}`).toBe(false);
        }
    });
});

describe('los límites que quedaban sin probar', () => {
    it('un catálogo demasiado grande se rechaza por la cabecera', async () => {
        // Igual que una captura: se mira la cabecera para no tragarse los megas y
        // después decir que no. Este camino no estaba cubierto.
        const s = secreto();
        const res = await llamar('/api/catalogo', {
            method: 'PUT', secreto: s,
            headers: { 'content-length': String(50 * 1024 * 1024) },
            body: JSON.stringify({ productos: [] }),
        });
        expect(res.status).toBe(413);
    });

    it('pedir el borrado de una lista que no es lista se rechaza', async () => {
        const s = secreto();
        for (const ids of [undefined, null, 'c-1', 42, {}]) {
            const res = await llamar('/api/recibido', {
                method: 'POST', secreto: s, body: JSON.stringify({ ids }),
            });
            expect(res.status, `con ids=${JSON.stringify(ids)}`).toBe(400);
        }
    });

    it('un identificador con barra no borra fuera de su carpeta', async () => {
        const a = secreto();
        const b = secreto();
        await subir(a, 'de-a');
        await subir(b, 'de-b');

        // El de B intenta borrar el de A escapándose de su carpeta.
        const huellaFalsa = '../t/cualquiera/de-a';
        const res = await llamar('/api/recibido', {
            method: 'POST', secreto: b, body: JSON.stringify({ ids: [huellaFalsa] }),
        });
        expect(res.status).toBe(200);
        expect((await res.json() as { borradas: number }).borradas, 'no puede contar como borrado').toBe(0);

        // Y lo de A sigue ahí.
        expect((await pendientes(a)).pendientes.map(p => p.captura_id)).toContain('de-a');
    });

    it('el punto de venta puede confirmar justo doscientas, no más', async () => {
        // El tope del relevo y el tamaño de tanda del punto de venta tienen que
        // coincidir: si el relevo aceptara menos de lo que la tienda manda, las
        // confirmaciones fallarían y todo se recogería una y otra vez.
        const s = secreto();
        const doscientas = Array.from({ length: 200 }, (_, i) => `c-${i}`);
        const justas = await llamar('/api/recibido', {
            method: 'POST', secreto: s, body: JSON.stringify({ ids: doscientas }),
        });
        expect(justas.status, 'doscientas es lo que manda la tienda por tanda').toBe(200);

        const una_mas = await llamar('/api/recibido', {
            method: 'POST', secreto: s, body: JSON.stringify({ ids: [...doscientas, 'c-200'] }),
        });
        expect(una_mas.status).toBe(400);
    });
});
