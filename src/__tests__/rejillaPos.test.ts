/**
 * La rejilla del mostrador no dibuja el catálogo entero.
 *
 * No está paginada, así que dibujaba todo: con dos mil prendas con foto son dos mil
 * nodos y unos 44 MB de miniaturas retenidas, porque la caché de miniaturas no
 * desaloja lo que está montado —su tope de 400 queda anulado cuando todo está a la
 * vista—. En la computadora del mostrador eso se siente al abrir la pantalla y no se
 * suelta mientras esté abierta.
 *
 * Y nadie encuentra una prenda entre dos mil bajando con el dedo: se busca por nombre
 * o se filtra por categoría. Lo que no puede pasar es que el resto desaparezca sin
 * decirlo, que es el error que se corrigió antes en la lista de ventas.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { TOPE_REJILLA_POS } from '../utils';
import { marcarEnPantalla, recordar, recordadas, estaRecordada } from '../hooks/useProductImages';

const PANTALLA = readFileSync('src/pages/POSPage.tsx', 'utf-8');

describe('la rejilla del punto de venta', () => {
    it('dibuja un tramo, no el catálogo entero', () => {
        expect(PANTALLA).toMatch(/visibleProducts\.slice\(0, TOPE_REJILLA_POS\)/);
    });

    it('dice cuántas quedaron fuera y cómo llegar a ellas', () => {
        expect(PANTALLA).toMatch(/visibleProducts\.length > TOPE_REJILLA_POS/);
        expect(PANTALLA, 'hay que decir qué hacer, no solo que hay más')
            .toMatch(/Busca por nombre o elige una categoría/);
    });

    it('el tope deja trabajar sin buscar en una tienda normal', () => {
        // Ni tan chico que estorbe ni tan grande que no sirva de nada.
        expect(TOPE_REJILLA_POS).toBeGreaterThanOrEqual(100);
        expect(TOPE_REJILLA_POS).toBeLessThanOrEqual(400);
    });

    it('el tope de la caché de miniaturas se anula si todo está montado', () => {
        // Es la razón por la que el tope de la rejilla hace falta: mientras una
        // miniatura está en pantalla no se olvida, así que con todo montado la caché
        // crece sin límite. Se comprueba para que nadie confíe en ese tope.
        const soltar: (() => void)[] = [];
        for (let id = 1; id <= 500; id++) {
            soltar.push(marcarEnPantalla(id));
            recordar(id, `data:image/jpeg;base64,${'A'.repeat(64)}`);
        }

        expect(recordadas(), 'con todo montado el tope no puede desalojar nada')
            .toBeGreaterThan(400);
        expect(estaRecordada(1), 'ni siquiera la más vieja').toBe(true);

        // Al soltarlas, la siguiente entrada sí puede desalojar.
        soltar.forEach(f => f());
        recordar(9999, 'data:image/jpeg;base64,AAAA');
        expect(recordadas()).toBeLessThanOrEqual(401);
    });
});
