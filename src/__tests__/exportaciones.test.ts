/**
 * Las dos exportaciones de Reportes tienen que comportarse igual.
 *
 * La de ventas por facturar preguntaba dónde guardar y confirmaba cuántas ventas
 * escribió. La del reporte de ventas bajaba un blob del navegador: nadie elegía
 * dónde, nadie sabía dónde quedaba, y el aviso decía "exportado exitosamente" sin
 * haber comprobado nada — ni que el archivo se escribiera ni que alguien no
 * hubiera cancelado el diálogo.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const PANTALLA = readFileSync('src/pages/ReportsPage.tsx', 'utf-8');

describe('exportar desde Reportes', () => {
    it('ninguna exportación baja un blob del navegador', () => {
        expect(PANTALLA, 'el archivo se escribe desde Rust, con ruta elegida')
            .not.toMatch(/createObjectURL|createElement\('a'\)/);
    });

    it('las dos piden dónde guardar', () => {
        const veces = [...PANTALLA.matchAll(/await save\(\{/g)].length;
        expect(veces, 'una por exportación').toBe(2);
    });

    it('ninguna canta victoria sin haber escrito', () => {
        // El aviso tiene que salir después de la llamada que escribe, y contar lo
        // que esa llamada devolvió.
        for (const fn of ['exportarFacturas', 'exportCSV']) {
            const i = PANTALLA.indexOf(`const ${fn} =`);
            expect(i, `no se encontró ${fn}`).toBeGreaterThan(-1);
            const cuerpo = PANTALLA.slice(i, PANTALLA.indexOf('\n    };', i));
            expect(cuerpo, `${fn} tiene que salirse si se cancela el diálogo`).toMatch(/if \(!target\) return;/);
            expect(cuerpo, `${fn} tiene que avisar con lo que devolvió el backend`).toMatch(/count|dias/);
        }
    });

    it('el comando existe en el backend y está registrado', () => {
        const reports = readFileSync('src-tauri/src/commands/reports.rs', 'utf-8');
        expect(reports).toMatch(/pub fn exportar_reporte_diario/);
        const lib = readFileSync('src-tauri/src/lib.rs', 'utf-8');
        expect(lib).toMatch(/reports::exportar_reporte_diario/);
    });
});
