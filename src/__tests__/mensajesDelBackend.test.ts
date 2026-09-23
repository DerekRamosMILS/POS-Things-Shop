/**
 * Cada frase por la que la pantalla reconoce una respuesta del backend tiene que
 * existir en el backend.
 *
 * Hay lugares donde la pantalla decide algo mirando el **texto** de lo que
 * contestó Rust. No es bonito, y a veces es lo razonable: el aviso de que una
 * venta era de un turno ya cerrado sirve para pintar la notificación de naranja en
 * vez de verde, y ese naranja es lo que hace que alguien lea que tiene que anotar
 * el efectivo como gasto para que el corte cuadre.
 *
 * Lo que no puede pasar es que la frase se reescriba en Rust y nadie se entere: la
 * comparación deja de casar en silencio, el aviso se pinta como un éxito
 * cualquiera y el corte del día queda descuadrado sin que nadie supiera por qué.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

function fuentes(dir: string, ext: string): string[] {
    return readdirSync(dir, { withFileTypes: true }).flatMap(e => {
        const ruta = join(dir, e.name);
        if (e.isDirectory()) return e.name === '__tests__' ? [] : fuentes(ruta, ext);
        return e.name.endsWith(ext) ? [ruta] : [];
    });
}

/**
 * Quita los módulos de prueba de un archivo de Rust.
 *
 * Sin esto la guarda no servía: al reescribir la frase en el código de producción,
 * seguía apareciendo dentro de un `assert!` de las pruebas y la comparación pasaba
 * en verde. Algunos archivos tienen más de un bloque `#[cfg(test)]` y no siempre al
 * final, así que se cuentan llaves en vez de cortar por el primero.
 */
export function sinPruebas(codigo: string): string {
    let out = '';
    let i = 0;
    while (true) {
        const marca = codigo.indexOf('#[cfg(test)]', i);
        if (marca === -1) { out += codigo.slice(i); break; }
        out += codigo.slice(i, marca);
        const abre = codigo.indexOf('{', marca);
        if (abre === -1) { i = codigo.length; break; }
        i = finDelBloque(codigo, abre);
    }
    return out;
}

/**
 * Dónde cierra el bloque que abre en `abre`, contando llaves **fuera** de cadenas
 * y comentarios.
 *
 * Contarlas a secas fue un error con consecuencia: la guarda de los caminos de
 * impresión trae el literal `"};"`, una llave sin pareja, y eso cortó el conteo una
 * llave antes de tiempo. El recorte dejó código de prueba dentro y esta guarda falló
 * por un motivo falso. Peor sería al revés: un literal con `{` haría que el recorte
 * se pasara de largo y se comiera código de producción, y entonces la guarda de las
 * frases dejaría pasar una frase reescrita sin decir nada.
 */
function finDelBloque(codigo: string, abre: number): number {
    let nivel = 0;
    let i = abre;
    while (i < codigo.length) {
        const c = codigo[i];
        const dos = codigo.slice(i, i + 2);

        if (dos === '//') { // comentario de línea
            const fin = codigo.indexOf('\n', i);
            i = fin === -1 ? codigo.length : fin + 1;
            continue;
        }
        if (dos === '/*') { // comentario de bloque
            const fin = codigo.indexOf('*/', i + 2);
            i = fin === -1 ? codigo.length : fin + 2;
            continue;
        }
        if (c === 'r' && /^r#*"/.test(codigo.slice(i))) { // cadena cruda: r"…" o r#"…"#
            const almohadillas = /^r(#*)"/.exec(codigo.slice(i))![1];
            const cierre = `"${almohadillas}`;
            const fin = codigo.indexOf(cierre, i + 1 + almohadillas.length + 1);
            i = fin === -1 ? codigo.length : fin + cierre.length;
            continue;
        }
        if (c === '"') { // cadena, con escapes
            i++;
            while (i < codigo.length && codigo[i] !== '"') {
                i += codigo[i] === '\\' ? 2 : 1;
            }
            i++;
            continue;
        }
        if (c === "'") { // literal de carácter, o una etiqueta de vida ('a)
            const trozo = codigo.slice(i, i + 4);
            const caracter = /^'(\\.|[^'\\])'/.exec(trozo);
            i += caracter ? caracter[0].length : 1;
            continue;
        }

        if (c === '{') nivel++;
        else if (c === '}') {
            nivel--;
            if (nivel === 0) return i + 1;
        }
        i++;
    }
    return i;
}

/** Todo el Rust de producción, junto, para buscar frases dentro. */
function todoElRust(): string {
    return fuentes('src-tauri/src', '.rs')
        .map(f => sinPruebas(readFileSync(f, 'utf-8')))
        .join('\n');
}

/** Frases que la pantalla busca dentro de una respuesta del backend. */
function frasesQueLaPantallaBusca(): { archivo: string; frase: string }[] {
    const out: { archivo: string; frase: string }[] = [];
    for (const archivo of [...fuentes('src/pages', '.tsx'), ...fuentes('src/components', '.tsx'), ...fuentes('src/utils', '.ts')]) {
        const codigo = readFileSync(archivo, 'utf-8');
        // `algo.includes('frase')` y `/frase/.test(...)`
        for (const m of codigo.matchAll(/\.includes\((['"])([^'"]{4,})\1\)/g)) {
            out.push({ archivo, frase: m[2] });
        }
    }
    return out;
}

describe('las frases con las que la pantalla lee al backend', () => {
    it('todas existen tal cual en el backend', () => {
        const rust = todoElRust();
        // Lo que compara contra datos propios de la pantalla, no contra Rust.
        const ajenas = new Set(['warning', 'success', 'error', 'cash', 'card', 'transfer']);

        const perdidas = frasesQueLaPantallaBusca()
            .filter(({ frase }) => !ajenas.has(frase))
            // Solo las que parecen mensajes del backend: en español con espacios,
            // o un centinela en mayúsculas.
            .filter(({ frase }) => / /.test(frase) || /^[A-Z_]{6,}$/.test(frase))
            .filter(({ frase }) => !rust.includes(frase))
            .map(({ archivo, frase }) => `${archivo}: "${frase}"`);

        expect(perdidas, 'la pantalla busca frases que el backend ya no dice').toEqual([]);
    });

    it('la lectura encontró frases de verdad', () => {
        const frases = frasesQueLaPantallaBusca().map(f => f.frase);
        expect(frases).toContain('SIN_IMPRESORA');
        expect(frases).toContain('turno ya cerrado');
    });

    it('el recorte no se pierde con llaves dentro de cadenas', () => {
        // El caso que lo rompió de verdad: la guarda de los caminos de impresión trae
        // el literal `"};"`, una llave sin pareja. Contando a secas, el recorte
        // terminaba una llave antes y dejaba código de prueba dentro. Al revés sería
        // peor: un literal con `{` haría que se comiera código de producción y esta
        // guarda dejaría pasar una frase reescrita.
        const conLiteral = [
            'fn produccion() { let x = "antes"; }',
            '#[cfg(test)]',
            'mod tests {',
            '    #[test]',
            '    fn a() { let cierre = "};"; let abre = "{"; assert!(cierre != abre); }',
            '    #[test]',
            '    fn b() { let s = r#"un {"raro"} con almohadilla"#; assert!(!s.is_empty()); }',
            '    #[test]',
            "    fn c() { let llave = '{'; let fin = '}'; assert_ne!(llave, fin); }",
            '}',
            'fn despues() { let y = "despues"; }',
        ].join('\n');

        const limpio = sinPruebas(conLiteral);

        expect(limpio).toContain('antes');
        expect(limpio, 'todo lo de producción de después tiene que sobrevivir').toContain('despues');
        expect(limpio, 'y nada de prueba puede quedarse').not.toContain('#[test]');
    });

    it('el recorte sobrevive a un comentario con una llave suelta', () => {
        const conComentario = [
            '#[cfg(test)]',
            'mod tests {',
            '    // aquí va una llave suelta } a propósito',
            '    /* y otra en bloque { */',
            '    #[test]',
            '    fn a() { assert!(true); }',
            '}',
            'fn despues() {}',
        ].join('\n');

        const limpio = sinPruebas(conComentario);
        expect(limpio).toContain('despues');
        expect(limpio).not.toContain('#[test]');
    });

    it('no se cuentan las frases que solo viven en las pruebas de Rust', () => {
        // Es lo que dejaba pasar la falsificación: la frase reescrita en
        // producción seguía dentro de un `assert!` y la guarda no se enteraba.
        const conPrueba = [
            'fn produccion() { let x = "frase de produccion"; }',
            '#[cfg(test)]',
            'mod tests {',
            '    #[test]',
            '    fn algo() { assert!(v.contains("frase de prueba")); }',
            '}',
            'fn despues() { let y = "frase de despues"; }',
        ].join('\n');

        const limpio = sinPruebas(conPrueba);

        expect(limpio).toContain('frase de produccion');
        expect(limpio).toContain('frase de despues');
        expect(limpio, 'lo de las pruebas no cuenta').not.toContain('frase de prueba');
    });

    it('el Rust de producción sigue teniendo cuerpo después de quitar las pruebas', () => {
        const rust = todoElRust();
        expect(rust).toContain('pub fn');
        expect(rust, 'quedó código de prueba dentro').not.toContain('#[test]');
    });
});
