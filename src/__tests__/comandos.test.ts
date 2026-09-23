/**
 * Un comando de Tauri existe en tres lugares a la vez: la función en Rust, la
 * lista de `generate_handler!` y la llamada desde la pantalla. Si falta el del
 * medio, la función compila, la pantalla la llama y el usuario recibe un error
 * de "comando no encontrado" que solo aparece en la función que lo usa —puede
 * pasar meses sin que nadie la toque, como pasó con el permiso del diálogo.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

function fuentesRust(dir: string): string[] {
    return readdirSync(dir, { withFileTypes: true }).flatMap(e => {
        const ruta = join(dir, e.name);
        if (e.isDirectory()) return fuentesRust(ruta);
        return e.name.endsWith('.rs') ? [ruta] : [];
    });
}

/** Los `#[tauri::command]` definidos en el backend. */
function definidos(): Set<string> {
    const out = new Set<string>();
    for (const archivo of fuentesRust('src-tauri/src')) {
        const codigo = readFileSync(archivo, 'utf-8');
        for (const m of codigo.matchAll(/#\[tauri::command\]\s*\n\s*pub (?:async )?fn ([a-z_0-9]+)/g)) {
            out.add(m[1]);
        }
    }
    return out;
}

/** Los que `generate_handler!` registra. */
function registrados(): Set<string> {
    const lib = readFileSync('src-tauri/src/lib.rs', 'utf-8');
    const desde = lib.indexOf('generate_handler!');
    const lista = lib.slice(desde, lib.indexOf('])', desde));
    const out = new Set<string>();
    for (const m of lista.matchAll(/([a-z_0-9]+)\s*(?:,|\])/g)) out.add(m[1]);
    return out;
}

/** Los que las pantallas invocan. */
function invocados(): Set<string> {
    const out = new Set<string>();
    for (const archivo of readdirSync('src/api')) {
        if (!archivo.endsWith('.ts')) continue;
        const codigo = readFileSync(join('src/api', archivo), 'utf-8');
        for (const m of codigo.matchAll(/invoke<[^>]*>\(\s*'([a-z_0-9]+)'/g)) out.add(m[1]);
    }
    return out;
}

describe('los comandos de Tauri', () => {
    it('todos los definidos están registrados en generate_handler', () => {
        const reg = registrados();
        const faltantes = [...definidos()].filter(c => !reg.has(c)).sort();
        expect(faltantes).toEqual([]);
    });

    it('todo lo que la pantalla invoca existe en el backend', () => {
        const def = definidos();
        const inventados = [...invocados()].filter(c => !def.has(c)).sort();
        expect(inventados).toEqual([]);
    });

    it('la lectura de las fuentes de verdad encontró algo', () => {
        // Si un cambio de formato rompe los patrones, los dos casos de arriba
        // pasarían con listas vacías sin comprobar nada.
        expect(definidos().size).toBeGreaterThan(80);
        expect(registrados().size).toBeGreaterThan(80);
        expect(invocados().size).toBeGreaterThan(80);
    });
});
