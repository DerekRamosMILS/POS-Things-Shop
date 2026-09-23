/**
 * La aritmética de versiones del script de publicación.
 *
 * Equivocarse aquí no da un error: da una tienda que se reinstala en un bucle
 * infinito, porque la app compara la versión que trae horneada contra la que
 * anuncia el manifiesto. Es barato de probar y caro de descubrir en producción.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
// @ts-expect-error -- script de Node sin tipos; se prueba su lógica pura.
import { resolverVersion, esMayor } from '../../scripts/publicar.mjs';

describe('resolver la versión pedida', () => {
    it('acepta un número explícito', () => {
        expect(resolverVersion('1.2.3', '0.1.0')).toBe('1.2.3');
    });

    it('traduce patch, minor y major', () => {
        expect(resolverVersion('patch', '0.1.0')).toBe('0.1.1');
        expect(resolverVersion('minor', '0.1.4')).toBe('0.2.0');
        expect(resolverVersion('major', '0.3.7')).toBe('1.0.0');
    });

    it('pone en cero lo que va a la derecha del salto', () => {
        // Un minor que dejara el patch en su valor anterior daría 1.3.9, que se
        // lee como si fuera posterior a versiones que nunca existieron.
        expect(resolverVersion('minor', '1.2.9')).toBe('1.3.0');
        expect(resolverVersion('major', '1.2.9')).toBe('2.0.0');
    });

    it('rechaza lo que no es una versión', () => {
        expect(() => resolverVersion('0.2', '0.1.0')).toThrow();
        expect(() => resolverVersion('v0.2.0', '0.1.0')).toThrow();
        expect(() => resolverVersion('ultima', '0.1.0')).toThrow();
    });
});

describe('comparar versiones', () => {
    it('compara por número y no como texto', () => {
        // Como texto, "0.10.0" < "0.9.0" y la tienda se quedaría sin recibir la
        // actualización, callada.
        expect(esMayor('0.10.0', '0.9.0')).toBe(true);
        expect(esMayor('0.9.0', '0.10.0')).toBe(false);
        expect(esMayor('1.0.0', '0.99.99')).toBe(true);
        expect(esMayor('0.2.10', '0.2.9')).toBe(true);
    });

    it('la misma versión no es mayor', () => {
        // Publicar el mismo número no le llega a nadie: el actualizador solo
        // avanza. Mejor negarse que dejar creer que se publicó algo.
        expect(esMayor('0.2.0', '0.2.0')).toBe(false);
    });

    it('no deja retroceder', () => {
        expect(esMayor('0.1.0', '0.2.0')).toBe(false);
    });
});

describe('cómo sube los cambios', () => {
    /**
     * La rama y la etiqueta viajan juntas o no viajan.
     *
     * Empujarlas por separado dejaba un estado intermedio malo: si fallaba el
     * segundo `push` —un corte de red a la mitad—, `main` se quedaba con el commit
     * de release y sin etiqueta. Nada se construye, la tienda no recibe nada, y
     * reintentar choca con la etiqueta que ya existe en local. Quien publica lo hace
     * cada varias semanas y no tiene por qué saber salir de eso a mano.
     */
    const GUION = readFileSync('scripts/publicar.mjs', 'utf-8');

    it('empuja la rama y la etiqueta en una sola orden atómica', () => {
        expect(GUION).toMatch(/'push',\s*'--atomic',\s*'origin',\s*'main'/);
    });

    it('no quedan empujes sueltos', () => {
        const empujes = [...GUION.matchAll(/sh\('git',\s*\['push'/g)].length;
        expect(empujes, 'un solo push: la rama y la etiqueta juntas').toBe(1);
    });

    it('se niega a publicar con cosas sin commitear o fuera de main', () => {
        // Las dos rejas que evitan publicar algo que nadie más tiene.
        expect(GUION).toMatch(/status', '--porcelain/);
        expect(GUION).toMatch(/Las versiones se publican desde main/);
        expect(GUION, 'y que main esté al día con origin').toMatch(/rev-parse', 'origin\/main/);
    });

    it('si la etiqueta ya existe, dice cómo salir de ahí', () => {
        // Pasa cuando el CI falló después de subir la etiqueta: la versión quedó
        // marcada y sin release. "Ya existe" a secas deja sin salida a quien publica
        // cada varias semanas.
        const bloque = GUION.slice(GUION.indexOf('etiquetas.includes'));
        const hasta = bloque.indexOf('\n}');
        const cuerpo = bloque.slice(0, hasta > 0 ? hasta : undefined);

        expect(cuerpo, 'tiene que decir si está en origin o solo en local').toMatch(/ls-remote/);
        expect(cuerpo, 'y qué hacer').toMatch(/publica la siguiente|pnpm publicar patch/);
        expect(cuerpo, 'sin sugerir reutilizar el número').toMatch(/el actualizador solo avanza/);
    });
});
