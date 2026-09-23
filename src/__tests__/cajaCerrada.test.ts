import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { BLOQUEADO_SIN_CAJA, AVISO_CAJA_CERRADA } from '../pages/cajaCerrada';
import { sinPruebas } from './mensajesDelBackend.test';

const raiz = resolve(__dirname, '../..');
const rust = (archivo: string) =>
    sinPruebas(readFileSync(resolve(raiz, 'src-tauri/src/commands', archivo), 'utf8'));

/**
 * Cada candado del backend, con la frase que le corresponde en el letrero.
 * Si alguien quita un candado, o agrega uno nuevo, esta tabla deja de cuadrar y
 * CI lo dice antes de que la tienda lea un letrero falso.
 */
const CANDADOS = [
    { archivo: 'sales.rs', mensaje: 'La caja no está abierta. Ábrela antes de cobrar.', frase: 'cobrar ventas' },
    { archivo: 'expenses.rs', mensaje: 'Abre la caja antes de registrar un gasto.', frase: 'registrar gastos' },
    { archivo: 'layaways.rs', mensaje: 'Abre la caja antes de recibir un abono en efectivo.', frase: 'recibir abonos en efectivo' },
    { archivo: 'returns.rs', mensaje: 'Abre la caja antes de devolver en efectivo.', frase: 'devolver en efectivo' },
] as const;

describe('el letrero de caja cerrada dice lo que el backend hace', () => {
    it.each(CANDADOS)('$archivo sigue negando: $frase', ({ archivo, mensaje }) => {
        expect(rust(archivo)).toContain(mensaje);
    });

    it('no hay candados de caja cerrada fuera de la tabla', () => {
        const archivos = ['sales.rs', 'expenses.rs', 'layaways.rs', 'returns.rs', 'cash_register.rs'];
        const hallados: string[] = [];
        for (const archivo of archivos) {
            for (const m of rust(archivo).matchAll(/"((?:Abre la caja|La caja no está abierta)[^"]*)"/g)) {
                hallados.push(`${archivo}: ${m[1]}`);
            }
        }
        expect(hallados.sort()).toEqual(CANDADOS.map(c => `${c.archivo}: ${c.mensaje}`).sort());
    });

    it('el aviso nombra cada operación bloqueada', () => {
        for (const { frase } of CANDADOS) {
            expect(AVISO_CAJA_CERRADA).toContain(frase);
            expect(BLOQUEADO_SIN_CAJA as readonly string[]).toContain(frase);
        }
    });

    it('la pantalla de Caja usa ese aviso y no promete ventas', () => {
        const pagina = readFileSync(resolve(raiz, 'src/pages/CashRegisterPage.tsx'), 'utf8');
        expect(pagina).toContain('AVISO_CAJA_CERRADA');
        expect(pagina).not.toMatch(/ventas pueden seguir/i);
    });
});
