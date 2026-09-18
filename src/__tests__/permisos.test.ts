/**
 * Los permisos de Tauri se enumeran a mano, y un comando sin permiso falla solo
 * en la aplicación empaquetada: la copia a USB quedó meses sin funcionar porque
 * faltaba `dialog:allow-open`. Esta prueba cruza lo que usa la pantalla contra
 * lo que está concedido.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { TOPE_LISTA_VENTAS } from '../utils';

/** Lo que hay que pedir para cada función de plugin que se use en el frontend. */
const PERMISO_DE: Record<string, string> = {
    open: 'dialog:allow-open',
    save: 'dialog:allow-save',
    message: 'dialog:allow-message',
    ask: 'dialog:allow-ask',
    confirm: 'dialog:allow-confirm',
    relaunch: 'process:allow-restart',
    exit: 'process:allow-exit',
    check: 'updater:default',
};

function fuentes(dir: string): string[] {
    return readdirSync(dir, { withFileTypes: true }).flatMap(e => {
        const ruta = join(dir, e.name);
        if (e.isDirectory()) return e.name === '__tests__' ? [] : fuentes(ruta);
        return /\.tsx?$/.test(e.name) ? [ruta] : [];
    });
}

describe('permisos de Tauri', () => {
    it('todo plugin que usa la pantalla tiene su permiso concedido', () => {
        const concedidos: string[] = JSON.parse(
            readFileSync('src-tauri/capabilities/default.json', 'utf-8'),
        ).permissions;

        const faltantes = new Set<string>();
        for (const archivo of fuentes('src')) {
            const codigo = readFileSync(archivo, 'utf-8');
            for (const m of codigo.matchAll(/import\s*\{([^}]+)\}\s*from\s*'@tauri-apps\/plugin-[a-z-]+'/g)) {
                for (const bruto of m[1].split(',')) {
                    const nombre = bruto.replace(/\btype\b/, '').split(' as ')[0].trim();
                    const permiso = PERMISO_DE[nombre];
                    if (permiso && !concedidos.includes(permiso)) faltantes.add(`${nombre} → ${permiso} (${archivo})`);
                }
            }
        }

        expect([...faltantes]).toEqual([]);
    });
});

describe('el tope de la lista de ventas', () => {
    it('la pantalla usa el mismo número que el backend', () => {
        // Si el LIMIT de `get_sales` cambia y la constante no, el aviso aparece
        // cuando no toca o no aparece cuando sí.
        const rust = readFileSync('src-tauri/src/commands/sales.rs', 'utf-8');
        const limite = rust.match(/ORDER BY s\.created_at DESC LIMIT (\d+)/);
        expect(limite).not.toBeNull();
        expect(Number(limite![1])).toBe(TOPE_LISTA_VENTAS);
    });
});
