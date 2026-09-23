/**
 * El largo mínimo de una contraseña, dicho una sola vez.
 *
 * Estaba escrito tres veces: 8 en Rust, 8 en la pantalla de cambio obligatorio y
 * **6** en la de usuarios. Un administrador creaba una cajera con una contraseña
 * de seis, la pantalla la aceptaba diciendo que con seis bastaba, y el backend la
 * rechazaba pidiendo ocho. La usuaria no se creaba y el mensaje contradecía lo que
 * la misma pantalla acababa de prometer.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { MIN_CONTRASENA } from '../utils';

describe('el largo mínimo de la contraseña', () => {
    it('es el mismo que exige el backend', () => {
        const rust = readFileSync('src-tauri/src/commands/users.rs', 'utf-8');
        const m = rust.match(/const MIN_PASSWORD_LEN: usize = (\d+);/);
        expect(m, 'no se encontró MIN_PASSWORD_LEN en users.rs').not.toBeNull();
        expect(MIN_CONTRASENA).toBe(Number(m![1]));
    });

    it('ninguna pantalla lo vuelve a escribir a mano', () => {
        // Es lo que dejó una de las tres copias en seis mientras las otras decían
        // ocho: cada lugar que lo repite es una oportunidad de que se desincronice.
        const paginas = readdirSync('src/pages')
            .filter(f => f.endsWith('.tsx'))
            .map(f => join('src/pages', f));

        const sospechosas: string[] = [];
        for (const archivo of paginas) {
            const codigo = readFileSync(archivo, 'utf-8');
            if (!/contraseña/i.test(codigo)) continue;
            // Un número suelto comparado contra el largo de algo que se llama
            // contraseña o password.
            if (/(password|contrasena|contraseña)[A-Za-z]*\.length\s*<\s*\d/i.test(codigo)) {
                sospechosas.push(archivo);
            }
            if (/const MIN_LENGTH\s*=\s*\d/.test(codigo)) sospechosas.push(archivo);
        }

        expect(sospechosas, 'tienen que usar MIN_CONTRASENA').toEqual([]);
    });
});
