/**
 * Los topes del teléfono y los de la caja tienen que decir lo mismo.
 *
 * El teléfono revisa lo que puede antes de guardar la captura en su cola, y eso es lo
 * que hace que la persona frente al perchero se entere a tiempo: si el número del
 * teléfono es más grande que el de la caja, alguien llena ochenta casillas de talla y
 * color y la captura se rechaza al llegar, con el trabajo hecho y perdido. Si es más
 * chico, el teléfono estorba sin motivo.
 *
 * Los dos lados están escritos a mano en lenguajes distintos, que es exactamente la
 * forma de bug que este proyecto ha tenido cuatro veces: el tope de la lista de
 * ventas, el mínimo de la contraseña, la frase de la sesión vencida y la fórmula del
 * efectivo.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const PWA = readFileSync('relevo/public/index.html', 'utf-8');
const PRODUCTO_RS = readFileSync('src-tauri/src/capture/producto.rs', 'utf-8');
const FOTOS_RS = readFileSync('src-tauri/src/commands/product_photos.rs', 'utf-8');
const WORKER = readFileSync('relevo/src/index.ts', 'utf-8');

function numeroEnPwa(nombre: string): number {
    const m = PWA.match(new RegExp(`var ${nombre} = ([^;]+);`));
    expect(m, `no se encontró ${nombre} en la página del celular`).not.toBeNull();
    // Puede venir como una expresión: `7.5 * 1024 * 1024`.
    return Number(new Function(`return (${m![1]})`)());
}

describe('los topes del celular contra los de la caja', () => {
    it('las combinaciones de talla y color son el mismo número', () => {
        const enRust = PRODUCTO_RS.match(/const MAX_VARIANTES: usize = (\d+);/);
        expect(enRust, 'no se encontró MAX_VARIANTES en producto.rs').not.toBeNull();
        expect(numeroEnPwa('MAX_VARIANTES')).toBe(Number(enRust![1]));
    });

    it('las fotos por producto son el mismo número', () => {
        const enRust = FOTOS_RS.match(/pub const MAX_POR_PRODUCTO: i64 = (\d+);/);
        expect(enRust, 'no se encontró MAX_POR_PRODUCTO').not.toBeNull();
        expect(numeroEnPwa('MAX_FOTOS')).toBe(Number(enRust![1]));
    });

    it('el peso que el teléfono se permite cabe en lo que acepta el relevo', () => {
        // Aquí no es igualdad: el teléfono se deja margen para el sobre JSON, y el
        // relevo mira la cabecera antes de leer el cuerpo. Lo que no puede pasar es
        // que el teléfono se permita más de lo que el relevo va a aceptar.
        const enWorker = WORKER.match(/const MAX_BYTES = ([^;]+);/);
        expect(enWorker, 'no se encontró MAX_BYTES en el relevo').not.toBeNull();
        const delRelevo = Number(new Function(`return (${enWorker![1]})`)());
        const delTelefono = numeroEnPwa('MAX_BYTES_CAPTURA');

        expect(delTelefono).toBeLessThan(delRelevo);
        expect(delTelefono, 'y con margen de verdad, no de un byte')
            .toBeLessThanOrEqual(delRelevo * 0.95);
    });

    it('el catálogo que el relevo acepta cabe en lo que la tienda publica', () => {
        const topeRelevo = WORKER.match(/const MAX_BYTES_CATALOGO = ([^;]+);/);
        expect(topeRelevo, 'no se encontró MAX_BYTES_CATALOGO').not.toBeNull();
        const bytes = Number(new Function(`return (${topeRelevo![1]})`)());
        const relevoRs = readFileSync('src-tauri/src/capture/relevo.rs', 'utf-8');
        const prendas = relevoRs.match(/pub\(crate\) const TOPE_CATALOGO: i64 = (\d+);/);
        expect(prendas, 'no se encontró TOPE_CATALOGO').not.toBeNull();

        // Sin fotos, una prenda con sus tallas ronda los 200 bytes de JSON. El tope de
        // prendas tiene que caber con holgura en el de bytes, o el catálogo se
        // rechazaría entero y el celular se quedaría sin con qué contar.
        const estimado = Number(prendas![1]) * 200;
        expect(estimado, `${prendas![1]} prendas estimadas en ${estimado} bytes`).toBeLessThan(bytes);
    });
});
