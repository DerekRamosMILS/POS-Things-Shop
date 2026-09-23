/**
 * El relevo tiene que comprobarse de tipos antes de desplegarse.
 *
 * Es el único componente que corre en producción y no se puede inspeccionar desde
 * aquí. `vitest` quita los tipos con esbuild **sin mirarlos**, así que durante mucho
 * tiempo un error de tipos en el worker pasaba el CI y se publicaba a Cloudflare: de
 * hecho había uno, en el `cursor` del listado de KV, que solo existe cuando el
 * listado no está completo.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const PAQUETE = JSON.parse(readFileSync('relevo/package.json', 'utf-8')) as {
    scripts: Record<string, string>;
    devDependencies: Record<string, string>;
};
const CI = readFileSync('.github/workflows/ci.yml', 'utf-8');

describe('los tipos del relevo', () => {
    it('tiene TypeScript y con qué comprobarse', () => {
        expect(PAQUETE.devDependencies.typescript, 'el relevo necesita TypeScript').toBeTruthy();
        expect(PAQUETE.scripts.typecheck).toBe('tsc --noEmit');
    });

    it('sus pruebas no pueden pasar sin comprobar los tipos', () => {
        // Correr `pnpm test` a secas en el relevo tiene que comprobar también.
        expect(PAQUETE.scripts.test).toMatch(/tsc --noEmit/);
    });

    it('el CI lo comprueba antes de publicar', () => {
        const trabajo = CI.slice(CI.indexOf('  relevo:'));
        const hasta = trabajo.indexOf('\n  windows:');
        const cuerpo = trabajo.slice(0, hasta > 0 ? hasta : undefined);
        expect(cuerpo).toMatch(/pnpm typecheck/);
    });

    it('la configuración de tipos existe y usa los tipos generados', () => {
        const tsconfig = readFileSync('relevo/tsconfig.json', 'utf-8');
        expect(tsconfig).toMatch(/"strict": true/);
        expect(tsconfig, 'los bindings salen de wrangler.jsonc, no de una copia a mano')
            .toMatch(/worker-configuration\.d\.ts/);
    });

    it('el worker no vuelve a declarar sus bindings a mano', () => {
        // Declararlos aquí y en la configuración los deja separarse en silencio.
        const worker = readFileSync('relevo/src/index.ts', 'utf-8');
        expect(worker).not.toMatch(/interface Env \{/);
    });
});
