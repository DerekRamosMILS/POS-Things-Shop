/**
 * Ajustes solo puede ofrecer lo que de verdad es un ajuste.
 *
 * La reja genérica dibujaba **toda** clave de `system_config` que no fuera de
 * hardware, con el nombre técnico por etiqueta cuando no tenía una. Lo que se
 * colaba no eran ajustes: `version_instalada` —que se escribe en cada arranque y
 * es el rastro de si la actualización llegó—, `ultima_copia_externa` —de la que
 * depende el aviso de que hace mucho no sale una copia del equipo— y la huella
 * del catálogo del relevo. Todas editables, en la tienda, ahora mismo.
 *
 * Se invierte la regla: en vez de una lista negra que siempre le falta algo, solo
 * se dibuja lo que tiene etiqueta. Y esta prueba cuida el otro lado: que ningún
 * ajuste de verdad se quede sin etiqueta y desaparezca de la pantalla.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { ETIQUETAS_DE_AJUSTES, HARDWARE_KEYS, esAjusteVisible } from '../pages/ajustesVisibles';

/** Las claves que la migración de arranque siembra: los ajustes de la tienda. */
function clavesSembradas(): string[] {
    const sql = readFileSync('src-tauri/migrations/002_seed_data.sql', 'utf-8');
    const bloque = sql.slice(sql.indexOf('INSERT OR IGNORE INTO system_config'));
    const hasta = bloque.indexOf(';');
    return [...bloque.slice(0, hasta).matchAll(/\('([a-z_]+)',/g)].map(m => m[1]);
}

/** Claves que Rust escribe por su cuenta: rastro interno, no ajustes. */
const INTERNAS = [
    'version_instalada',
    'ultima_copia_externa',
    'relevo_catalogo_publicado',
    'relevo_url',
    'sku_counter',
    'terminal_id',
    'demo_seeded',
];

describe('qué se puede editar en Ajustes', () => {
    it('ningún rastro interno se ofrece como ajuste', () => {
        const colados = INTERNAS.filter(k => esAjusteVisible(k));
        expect(colados, 'esto no son ajustes y no se pueden editar a mano').toEqual([]);
    });

    it('todo ajuste sembrado al arrancar sigue siendo editable', () => {
        // El otro lado del filtro: si alguien agrega un ajuste al arranque y no le
        // pone etiqueta, desaparece de la pantalla sin que nadie lo note.
        const sinEtiqueta = clavesSembradas().filter(k => !esAjusteVisible(k) && !HARDWARE_KEYS.includes(k));
        expect(sinEtiqueta, 'estos ajustes existen pero ya no se pueden ver ni cambiar').toEqual([]);
    });

    it('los de hardware van en su propia tarjeta, no en la reja', () => {
        for (const k of HARDWARE_KEYS) {
            expect(esAjusteVisible(k), `${k} es de hardware`).toBe(false);
        }
    });

    it('la lectura del arranque encontró los ajustes', () => {
        expect(clavesSembradas().length).toBeGreaterThan(5);
        expect(Object.keys(ETIQUETAS_DE_AJUSTES).length).toBeGreaterThan(5);
    });

    it('la pantalla usa el filtro y no una lista negra', () => {
        const pantalla = readFileSync('src/pages/SettingsPage.tsx', 'utf-8');
        expect(pantalla).toMatch(/esAjusteVisible/);
        expect(pantalla, 'la lista negra se va: siempre le faltaba algo').not.toMatch(/HIDDEN_KEYS/);
    });
});
