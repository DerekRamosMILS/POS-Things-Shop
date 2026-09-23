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
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { ETIQUETAS_DE_AJUSTES, HARDWARE_KEYS, esAjusteVisible } from '../pages/ajustesVisibles';

/**
 * Las claves que **cualquier** migración siembra: los ajustes de la tienda.
 *
 * Mirar solo la migración de arranque era el hueco de esta guarda, y dejó pasar una
 * regresión: la 015 siembra los datos fiscales del negocio y la 013 los del
 * hardware, así que tres ajustes se quedaron sin etiqueta y desaparecieron de la
 * pantalla sin que nada lo dijera. Un ajuste sembrado es un ajuste que alguien tiene
 * que poder ver, venga de la migración que venga.
 */
function clavesSembradas(): string[] {
    const claves: string[] = [];
    for (const archivo of readdirSync('src-tauri/migrations').sort()) {
        const sql = readFileSync(join('src-tauri/migrations', archivo), 'utf-8');
        let desde = 0;
        while (true) {
            const i = sql.indexOf('INTO system_config', desde);
            if (i === -1) break;
            const hasta = sql.indexOf(';', i);
            const bloque = sql.slice(i, hasta === -1 ? undefined : hasta);
            // Solo los INSERT con lista de valores; los UPDATE no siembran nada.
            if (/VALUES/i.test(bloque)) {
                for (const m of bloque.matchAll(/\('([a-z_]+)',\s*'/g)) claves.push(m[1]);
            }
            desde = hasta === -1 ? sql.length : hasta + 1;
        }
    }
    return [...new Set(claves)];
}

/**
 * Ajustes que una migración siembra y que aun así **no** se editan a mano, con su
 * motivo. Si alguno deja de corresponder a algo, la prueba de abajo lo dice: una
 * entrada muerta aquí esconde el siguiente ajuste que se caiga de la pantalla.
 */
const SEMBRADAS_PERO_INTERNAS: Record<string, string> = {
    update_endpoint: 'de dónde se bajan las actualizaciones; cambiarlo a mano rompe el actualizador',
    terminal_id: 'identifica la caja y entra en los folios; cambiarlo parte la numeración',
};

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
        const sinEtiqueta = clavesSembradas().filter(k =>
            !esAjusteVisible(k) && !HARDWARE_KEYS.includes(k) && !(k in SEMBRADAS_PERO_INTERNAS));
        expect(sinEtiqueta, 'estos ajustes existen pero ya no se pueden ver ni cambiar').toEqual([]);
    });

    it('los de hardware van en su propia tarjeta, no en la reja', () => {
        for (const k of HARDWARE_KEYS) {
            expect(esAjusteVisible(k), `${k} es de hardware`).toBe(false);
        }
    });

    it('la lectura de las migraciones encontró los ajustes', () => {
        const sembradas = clavesSembradas();
        expect(sembradas.length).toBeGreaterThan(15);
        // Las de la 015, que son las que se habían caído de la pantalla.
        expect(sembradas).toContain('rfc_emisor');
        expect(sembradas).toContain('printer_name');
        expect(Object.keys(ETIQUETAS_DE_AJUSTES).length).toBeGreaterThan(5);
    });

    it('la pantalla usa el filtro y no una lista negra', () => {
        const pantalla = readFileSync('src/pages/SettingsPage.tsx', 'utf-8');
        expect(pantalla).toMatch(/esAjusteVisible/);
        expect(pantalla, 'la lista negra se va: siempre le faltaba algo').not.toMatch(/HIDDEN_KEYS/);
    });

    it('la lista de sembradas-pero-internas no se queda con nombres muertos', () => {
        const sembradas = new Set(clavesSembradas());
        const muertas = Object.keys(SEMBRADAS_PERO_INTERNAS).filter(k => !sembradas.has(k));
        expect(muertas, 'ya no las siembra ninguna migración').toEqual([]);
    });

    it('los datos fiscales del negocio se pueden capturar', () => {
        // La regresión concreta: la 015 los siembra vacíos y la tienda tiene que
        // poder poner su RFC. Se quedaron sin etiqueta y desaparecieron.
        for (const k of ['rfc_emisor', 'regimen_emisor', 'cp_emisor']) {
            expect(esAjusteVisible(k), `${k} tiene que poder capturarse`).toBe(true);
        }
    });
});
