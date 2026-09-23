/**
 * Un `if:` de paso que mire una variable que nunca existe nunca corre.
 *
 * El `if:` de un paso se evalúa **antes** de armar el `env:` de ese mismo paso, así
 * que `if: env.X` con `X` declarada solo ahí es siempre falso: el paso no corre y
 * nada lo dice. Pasó con el certificado de firma de Windows, que existía completo y
 * no se ejecutaba nunca. Quien configurara el secreto seguiría viendo "editor
 * desconocido" en SmartScreen sin ninguna pista.
 *
 * Es el peor sitio para un fallo callado: estos flujos son lo que construye y firma
 * lo que se instala solo en la tienda.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const DIR = '.github/workflows';
const FLUJOS = readdirSync(DIR).filter(f => f.endsWith('.yml') || f.endsWith('.yaml'));

/** Variables que un `if:` de paso puede ver de verdad en este flujo. */
function visiblesEn(texto: string): Set<string> {
    const fuera = new Set<string>();

    // Escritas por un paso anterior: `echo "X=..." >> $env:GITHUB_ENV` o `$GITHUB_ENV`.
    for (const m of texto.matchAll(/"?([A-Z_][A-Z0-9_]*)=[^"]*"?\s*>>\s*\$(?:env:)?GITHUB_ENV/g)) {
        fuera.add(m[1]);
    }

    // Declaradas en un `env:` de flujo o de trabajo: el bloque va a 0 o 4 espacios,
    // nunca a 8 (que es el de un paso).
    const lineas = texto.split('\n');
    for (let i = 0; i < lineas.length; i++) {
        const cabecera = /^(\s*)env:\s*$/.exec(lineas[i]);
        if (!cabecera || cabecera[1].length > 4) continue;
        for (let j = i + 1; j < lineas.length; j++) {
            const clave = /^(\s+)([A-Za-z_][A-Za-z0-9_]*):/.exec(lineas[j]);
            if (!clave || clave[1].length <= cabecera[1].length) break;
            fuera.add(clave[2]);
        }
    }
    return fuera;
}

describe('los flujos de CI y de publicación', () => {
    it('encontró los flujos', () => {
        expect(FLUJOS).toContain('ci.yml');
        expect(FLUJOS).toContain('release.yml');
    });

    it('ningún `if:` mira una variable que ese paso define para sí mismo', () => {
        const problemas: string[] = [];
        for (const archivo of FLUJOS) {
            const texto = readFileSync(join(DIR, archivo), 'utf-8');
            const visibles = visiblesEn(texto);
            for (const m of texto.matchAll(/^\s*if:\s*.*?env\.([A-Za-z_][A-Za-z0-9_]*)/gm)) {
                if (!visibles.has(m[1])) {
                    problemas.push(`${archivo}: if: env.${m[1]} — nunca va a ser verdad`);
                }
            }
        }
        expect(problemas, 'estos pasos no se ejecutan nunca').toEqual([]);
    });

    it('nada silencia un fallo', () => {
        // Un paso que no pueda fallar deja pasar un instalador sin probar.
        for (const archivo of FLUJOS) {
            const texto = readFileSync(join(DIR, archivo), 'utf-8');
            expect(texto, `${archivo} no puede tener continue-on-error`).not.toMatch(/continue-on-error:\s*true/);
            expect(texto, `${archivo} no puede ignorar el código de salida`).not.toMatch(/\|\|\s*true/);
        }
    });

    it('publicar exige que las pruebas pasen y que la firma exista', () => {
        const release = readFileSync(join(DIR, 'release.yml'), 'utf-8');
        expect(release, 'las pruebas antes de construir').toMatch(/cargo test/);
        expect(release, 'y las del frontend').toMatch(/pnpm test/);
        expect(release, 'sin .sig la app rechaza la versión').toMatch(/Falta el \.sig/);
        expect(release, 'y hay que comprobar que la tienda puede bajarlo')
            .toMatch(/releases\/latest\/download\/latest\.json/);
    });
});
