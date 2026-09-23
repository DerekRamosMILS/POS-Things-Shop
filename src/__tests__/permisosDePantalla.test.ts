/**
 * Ninguna pantalla que la cajera pueda abrir debe pedir datos que el backend le
 * niega.
 *
 * El rol se decide en Rust con `require_admin`, que es lo correcto: la pantalla no
 * es una frontera de seguridad. Pero si el menú ofrece una pantalla y el backend
 * le niega los datos, la cajera se queda mirando un error y una pantalla vacía sin
 * saber por qué. Pasó con el Dashboard, que además es el primer renglón del menú.
 *
 * Una pantalla está bien si el enrutado la marca `adminOnly`, o si ella misma sabe
 * de roles (`isAdmin`) para pedir solo lo que le toca.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

/** Nombre de la función del API → comando de Tauri que invoca. */
function comandoPorFuncion(): Map<string, string> {
    const codigo = readFileSync('src/api/index.ts', 'utf-8');
    const mapa = new Map<string, string>();
    for (const m of codigo.matchAll(
        /export const ([A-Za-z0-9_]+)\s*=[\s\S]{0,200}?invoke<[^>]*>\(\s*'([a-z_0-9]+)'/g,
    )) {
        mapa.set(m[1], m[2]);
    }
    return mapa;
}

/** Comandos que el backend solo deja usar a un administrador. */
function comandosDeAdmin(): Set<string> {
    const fuentes = (dir: string): string[] =>
        readdirSync(dir, { withFileTypes: true }).flatMap(e => {
            const ruta = join(dir, e.name);
            if (e.isDirectory()) return fuentes(ruta);
            return e.name.endsWith('.rs') ? [ruta] : [];
        });

    const out = new Set<string>();
    for (const archivo of fuentes('src-tauri/src')) {
        const codigo = readFileSync(archivo, 'utf-8');
        for (const bloque of codigo.split('#[tauri::command]').slice(1)) {
            const firma = bloque.match(/pub (?:async )?fn ([a-z_0-9]+)/);
            if (!firma) continue;
            // Solo el cuerpo de este comando, hasta el siguiente elemento.
            const cuerpo = bloque.split('\n#[')[0];
            if (cuerpo.includes('require_admin')) out.add(firma[1]);
        }
    }
    return out;
}

/** Rutas de App.tsx: componente → si está marcada como solo de administrador. */
function rutas(): { componente: string; ruta: string; soloAdmin: boolean }[] {
    const app = readFileSync('src/App.tsx', 'utf-8');
    const out: { componente: string; ruta: string; soloAdmin: boolean }[] = [];
    for (const m of app.matchAll(/<Route path="([a-z-]+)" element=\{(.+?)\}\s*\/>/g)) {
        const elemento = m[2];
        const comp = elemento.match(/<([A-Z][A-Za-z0-9]*Page)\s*\/>/);
        if (!comp) continue;
        out.push({ ruta: m[1], componente: comp[1], soloAdmin: elemento.includes('adminOnly') });
    }
    return out;
}

describe('los permisos de cada pantalla', () => {
    it('lo que la cajera puede abrir no pide datos de administrador', () => {
        const comandos = comandoPorFuncion();
        const soloAdmin = comandosDeAdmin();
        const problemas: string[] = [];

        for (const { ruta, componente, soloAdmin: gated } of rutas()) {
            if (gated) continue;
            const archivo = `src/pages/${componente}.tsx`;
            const codigo = readFileSync(archivo, 'utf-8');
            // Si la pantalla sabe de roles, se confía en que pida solo lo suyo.
            if (/isAdmin|role === 'admin'/.test(codigo)) continue;

            const negados = [...codigo.matchAll(/api\.([A-Za-z0-9_]+)\(/g)]
                .map(m => comandos.get(m[1]))
                .filter((c): c is string => !!c && soloAdmin.has(c));

            if (negados.length > 0) {
                problemas.push(`/${ruta} (${componente}) → ${[...new Set(negados)].join(', ')}`);
            }
        }

        expect(problemas, 'estas pantallas se abren sin ser adminOnly y piden comandos de administrador').toEqual([]);
    });

    it('el menú no ofrece una pantalla que el enrutado le niega', () => {
        // Al revés del anterior: una ruta `adminOnly` cuyo enlace no esté detrás de
        // `isAdmin` manda a la cajera de vuelta al punto de venta sin explicación.
        const menu = readFileSync('src/components/layout/MainLayout.tsx', 'utf-8');
        const sinEsconder: string[] = [];

        for (const { ruta, soloAdmin } of rutas()) {
            if (!soloAdmin) continue;
            const i = menu.indexOf(`to="/${ruta}"`);
            if (i === -1) continue; // No está en el menú: nada que revisar.
            // El enlace tiene que venir precedido por su `isAdmin &&`.
            const antes = menu.slice(Math.max(0, i - 200), i);
            if (!antes.includes('isAdmin')) sinEsconder.push(`/${ruta}`);
        }

        expect(sinEsconder, 'estos enlaces se ven pero la ruta los rechaza').toEqual([]);
    });

    it('la lectura encontró de verdad rutas y comandos', () => {
        expect(rutas().length).toBeGreaterThan(8);
        expect(comandosDeAdmin().size).toBeGreaterThan(15);
        expect(comandoPorFuncion().size).toBeGreaterThan(50);
    });
});
