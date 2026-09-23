import { readFileSync } from 'node:fs';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
    ESCALA_MAXIMA, ESCALA_MINIMA, ESCALA_POR_DEFECTO, creaGuardadoDiferido, normalizarEscala,
} from '../utils/escala';

describe('escala de la interfaz', () => {
    it('un ajuste corrupto o vacío no deja la pantalla inservible', () => {
        // Es lo único que puede pasar aquí y dejar la caja sin poder usarse:
        // una escala de cero o de mil.
        for (const malo of [null, undefined, '', 'abc', '0', 0, NaN, Infinity]) {
            expect(normalizarEscala(malo)).toBe(ESCALA_POR_DEFECTO);
        }
    });

    it('se queda dentro de lo legible', () => {
        expect(normalizarEscala(10)).toBe(ESCALA_MINIMA);
        expect(normalizarEscala(1000)).toBe(ESCALA_MAXIMA);
        expect(normalizarEscala(-50)).toBe(ESCALA_MINIMA);
    });

    it('respeta lo que se elige dentro del rango', () => {
        expect(normalizarEscala(125)).toBe(125);
        expect(normalizarEscala('90')).toBe(90);
        expect(normalizarEscala(117.4)).toBe(117);
    });
});

describe('cuándo se recuerda el tamaño elegido', () => {
    afterEach(() => vi.useRealTimers());

    it('se guarda aunque se suelte el ratón fuera del deslizador', () => {
        // Arrastrar un deslizador y soltar pasándose del borde es lo normal, y
        // entonces el `mouseup` le llega al documento y no al control: el tamaño
        // se veía aplicado y no se guardaba nunca. Al siguiente arranque volvía
        // al de antes y parecía que el ajuste no servía.
        vi.useFakeTimers();
        const guardado: number[] = [];
        const g = creaGuardadoDiferido(n => guardado.push(n));

        g.programar(120);
        g.programar(135);
        g.programar(140);
        expect(guardado, 'todavía está arrastrando').toEqual([]);

        vi.advanceTimersByTime(1000);
        expect(guardado, 'solo el último valor').toEqual([140]);
    });

    it('soltar sobre el control lo guarda en el momento, sin repetirlo', () => {
        vi.useFakeTimers();
        const guardado: number[] = [];
        const g = creaGuardadoDiferido(n => guardado.push(n));

        g.programar(120);
        g.ahora(120);
        expect(guardado).toEqual([120]);

        vi.advanceTimersByTime(1000);
        expect(guardado, 'el diferido pendiente se cancela').toEqual([120]);
    });

    it('la pantalla programa el guardado en cada cambio, no solo al soltar', () => {
        // El arreglo vive en el cableado de los eventos, que no se puede llamar
        // desde aquí. Comprobar que aparece el nombre de la fábrica no bastaba:
        // quitando la llamada de la vista previa, la prueba seguía pasando.
        // Lo que importa es que el diferido se programe donde se previsualiza.
        const pantalla = readFileSync('src/components/TamanoSettings.tsx', 'utf-8');
        const previsualizar = pantalla.slice(pantalla.indexOf('const previsualizar'));
        const cuerpo = previsualizar.slice(0, previsualizar.indexOf('};'));

        expect(cuerpo, 'la vista previa tiene que programar el guardado').toMatch(/\.programar\(/);
        expect(pantalla, 'y soltar encima lo guarda en el momento').toMatch(/\.ahora\(/);
    });
});
