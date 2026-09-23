/**
 * Escribir en dos tablas es todo o nada.
 *
 * Es la clase de bug que más veces ha aparecido en este proyecto: dos `execute`
 * seguidos, el segundo falla, y queda un gasto editado con el corte sin ajustar, un
 * abono cobrado con el saldo sin bajar, una prenda con existencia y sin el
 * movimiento que la explica. Nada de eso se ve desde fuera hasta que alguien cuenta
 * billetes o revisa un historial.
 *
 * La prueba lee el Rust de producción y busca funciones que escriban en dos o más
 * tablas sin abrir transacción. Las excepciones se enumeran con su motivo: o corren
 * dentro de la transacción de quien las llama, o lo que escriben de más es un
 * rastro que puede fallar sin consecuencia.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { sinPruebas } from './mensajesDelBackend.test';

function fuentesRust(dir: string): string[] {
    return readdirSync(dir, { withFileTypes: true }).flatMap(e => {
        const ruta = join(dir, e.name);
        if (e.isDirectory()) return fuentesRust(ruta);
        return e.name.endsWith('.rs') ? [ruta] : [];
    });
}

/**
 * Funciones que escriben en varias tablas y no abren transacción, con su motivo.
 *
 * Cada una está revisada. Si aparece una nueva, la prueba falla y hay que decidir:
 * o se envuelve, o se agrega aquí explicando por qué no hace falta.
 */
const REVISADAS: Record<string, string> = {
    post_layaway_payment: 'corre dentro de la transacción de registrar_apartado y de abonar_apartado',
    registrar_venta_de_entrega: 'corre dentro de la transacción de entregar_apartado',
    mark_notification_read: 'escribe en una tabla o en la otra, nunca en las dos',
    agregar_foto: 'la segunda escritura solo toca updated_at del producto y va con .ok()',
};

/** `app_logs` es rastro y puede fallar sin consecuencia: no cuenta. */
const NO_CUENTAN = new Set(['app_logs', 'set']);

function sospechosas(): { archivo: string; fn: string; tablas: string[] }[] {
    const out: { archivo: string; fn: string; tablas: string[] }[] = [];
    for (const archivo of fuentesRust('src-tauri/src')) {
        const codigo = sinPruebas(readFileSync(archivo, 'utf-8'));
        const trozos = codigo.split(/\n(?=(?:#\[tauri::command\]\n)?(?:pub(?:\(crate\))? )?(?:async )?fn )/);
        for (const trozo of trozos) {
            const m = trozo.match(/^(?:#\[tauri::command\]\n)?(?:pub(?:\(crate\))? )?(?:async )?fn ([a-z_0-9]+)/);
            if (!m) continue;
            const escrituras = [...trozo.matchAll(/(?:INSERT (?:OR \w+ )?INTO|UPDATE|DELETE FROM)\s+([a-z_]+)/gi)]
                .map(x => x[1].toLowerCase());
            const tablas = [...new Set(escrituras)].filter(t => !NO_CUENTAN.has(t));
            if (tablas.length < 2) continue;
            if (/BEGIN TRANSACTION|en_transaccion/.test(trozo)) continue;
            out.push({ archivo, fn: m[1], tablas: tablas.sort() });
        }
    }
    return out;
}

describe('escribir en varias tablas', () => {
    it('siempre va dentro de una transacción, salvo lo revisado', () => {
        const nuevas = sospechosas()
            .filter(s => !(s.fn in REVISADAS))
            .map(s => `${s.archivo}::${s.fn} → ${s.tablas.join(', ')}`);

        expect(nuevas, 'esto escribe en varias tablas sin transacción: envuélvelo o justifícalo en REVISADAS').toEqual([]);
    });

    it('la lista de excepciones no se queda con nombres muertos', () => {
        // Una excepción que ya no corresponde a nada esconde el siguiente caso.
        const vivas = new Set(sospechosas().map(s => s.fn));
        const muertas = Object.keys(REVISADAS).filter(f => !vivas.has(f));
        expect(muertas, 'estas ya no hacen falta en REVISADAS').toEqual([]);
    });

    it('la lectura del Rust encontró funciones de verdad', () => {
        const todas = fuentesRust('src-tauri/src').length;
        expect(todas).toBeGreaterThan(20);
        // Y que el análisis vea escrituras: si el patrón se rompe, daría cero.
        const conEscrituras = fuentesRust('src-tauri/src')
            .filter(f => /INSERT INTO|UPDATE /i.test(readFileSync(f, 'utf-8'))).length;
        expect(conEscrituras).toBeGreaterThan(10);
    });
});
