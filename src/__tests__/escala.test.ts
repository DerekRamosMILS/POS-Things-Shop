import { describe, expect, it } from 'vitest';
import {
    ESCALA_MAXIMA, ESCALA_MINIMA, ESCALA_POR_DEFECTO, normalizarEscala,
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
