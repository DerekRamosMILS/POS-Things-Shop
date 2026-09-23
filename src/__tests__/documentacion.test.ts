/**
 * Los números que la documentación afirma tienen que ser los del código.
 *
 * El README es lo que va a leer quien vuelva a esto en un año, y el manual es lo que
 * lee quien está en el mostrador. Los dos citan números concretos —cuántas prendas
 * dibuja la rejilla, cuántos caracteres pide una contraseña, cuánto dura una sesión—
 * y esos números viven en prosa: cuando el del código cambia, el del texto se queda,
 * y entonces la documentación no está incompleta, está **mintiendo**. Eso es peor que
 * no tenerla, porque se le cree.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const README = readFileSync('README.md', 'utf-8');
const MANUAL = readFileSync('docs/MANUAL.md', 'utf-8');

/** Los números que el texto escribe en palabras. */
const EN_PALABRAS: Record<string, string> = {
    '6': 'Seis', '8': 'Ocho', '10': 'Diez', '12': 'Doce', '15': 'Quince',
    '100': 'Cien', '150': 'Ciento cincuenta', '200': 'Doscientas', '250': 'Doscientas cincuenta',
    '300': 'Trescientas', '400': 'Cuatrocientas',
};

/** Saca un número de una constante del código. */
function delCodigo(archivo: string, patron: RegExp): string {
    const m = readFileSync(archivo, 'utf-8').match(patron);
    expect(m, `no se encontró ${patron} en ${archivo}`).not.toBeNull();
    return m![1];
}

interface Afirmacion {
    que: string;
    /** Cómo aparece el número en el texto, con el número por `%d`. */
    enElTexto: (n: string) => RegExp;
    archivo: string;
    patron: RegExp;
    texto?: string;
}

const AFIRMACIONES: Afirmacion[] = [
    {
        que: 'las prendas que dibuja la rejilla del mostrador',
        // El texto lo escribe en palabras, así que se traduce: comprobar la frase a
        // secas ignoraba el número del código, y al falsificarla bajando el tope de
        // 200 a 150 la prueba pasaba igual. Una aserción que no mira lo que dice
        // mirar es lo mismo que no tenerla.
        enElTexto: n => new RegExp(`${EN_PALABRAS[n] ?? `__sin palabra para ${n}__`} prendas`),
        archivo: 'src/utils/index.ts', patron: /TOPE_REJILLA_POS = (\d+)/,
    },
    {
        que: 'las prendas que caben en el catálogo del celular',
        enElTexto: n => new RegExp(`${Number(n).toLocaleString('es-MX').replace(',', '[ ,]')} prendas`),
        archivo: 'src-tauri/src/capture/relevo.rs', patron: /TOPE_CATALOGO: i64 = (\d+)/,
    },
    {
        que: 'las combinaciones de talla y color',
        enElTexto: n => new RegExp(`${n} combinaciones`),
        archivo: 'src-tauri/src/capture/producto.rs', patron: /MAX_VARIANTES: usize = (\d+)/,
    },
    {
        que: 'las fotos por prenda',
        enElTexto: n => new RegExp(`${n} fotos`),
        archivo: 'src-tauri/src/commands/product_photos.rs', patron: /MAX_POR_PRODUCTO: i64 = (\d+)/,
    },
    {
        que: 'los minutos que aguanta una restauración preparada',
        enElTexto: n => new RegExp(`${n} minutos`),
        archivo: 'src-tauri/src/db/connection.rs', patron: /VIGENCIA_RESTAURACION_SEGUNDOS: i64 = (\d+) \* 60/,
    },
    {
        que: 'las miniaturas que se recuerdan',
        enElTexto: n => new RegExp(`límite de ${n}`),
        archivo: 'src/hooks/useProductImages.ts', patron: /MAX_EN_MEMORIA = (\d+)/,
    },
    {
        que: 'las ventas que muestra la lista',
        enElTexto: n => new RegExp(`${Number(n).toLocaleString('es-MX').replace(',', '[ ,]')}`),
        archivo: 'src/utils/index.ts', patron: /TOPE_LISTA_VENTAS = (\d+)/,
    },
    {
        que: 'los megas a los que rota la bitácora',
        enElTexto: n => new RegExp(`${n} MB`),
        archivo: 'src-tauri/src/logging.rs', patron: /MAX_LOG_BYTES: u64 = (\d+) \* 1024/,
    },
    {
        que: 'las horas que dura una sesión',
        enElTexto: n => new RegExp(`${n} h`),
        archivo: 'src-tauri/src/session.rs', patron: /DEFAULT_SESSION_HOURS: i64 = (\d+)/,
    },
];

describe('lo que la documentación afirma', () => {
    it('cada número del README es el del código', () => {
        const mentiras: string[] = [];
        for (const a of AFIRMACIONES) {
            const n = delCodigo(a.archivo, a.patron);
            if (!a.enElTexto(n).test(README)) {
                mentiras.push(`${a.que}: el código dice ${n} y el README no lo dice así`);
            }
        }
        expect(mentiras, 'la documentación se separó del código').toEqual([]);
    });

    it('el mínimo de la contraseña se dice en palabras y cuadra', () => {
        const n = delCodigo('src-tauri/src/commands/users.rs', /MIN_PASSWORD_LEN: usize = (\d+)/);
        const enPalabras: Record<string, string> = { '6': 'seis', '8': 'ocho', '10': 'diez', '12': 'doce' };
        expect(README).toMatch(new RegExp(`${enPalabras[n]} caracteres`));
    });

    it('el manual no promete permisos que el backend no da', () => {
        // Lo que el manual le dice a la cajera tiene que ser lo que el backend hace.
        expect(MANUAL, 'el manual dice que el descuento por renglón es de administrador')
            .toMatch(/descuentos por renglón solo los puede dar un administrador/i);
        const ventas = readFileSync('src-tauri/src/commands/sales.rs', 'utf-8');
        expect(ventas, 'y el backend tiene que exigirlo').toMatch(/es_admin/);
    });

    it('las pruebas que el README nombra existen', () => {
        const nombradas = [...README.matchAll(/`([a-zA-Z][a-zA-Z0-9_]*)`/g)]
            .map(m => m[1])
            .filter(n => /^[a-z][a-zA-Z]+$/.test(n) && n.length > 6);
        // De las citadas, las que son archivos de prueba del frontend.
        const { existsSync } = require('node:fs') as typeof import('node:fs');
        const faltan = nombradas.filter(n =>
            README.includes(`prueba \`${n}\``) && !existsSync(`src/__tests__/${n}.test.ts`));
        expect(faltan, 'el README nombra pruebas que no existen').toEqual([]);
    });
});
