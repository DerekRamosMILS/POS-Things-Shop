/**
 * La caché de miniaturas del listado. El punto delicado es qué pasa después de
 * reemplazar la foto de un producto: la miniatura vieja ya no sirve y hay que
 * volver a pedirla sin esperar a que el listado se recargue.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const getProductImages = vi.fn();
vi.mock('../api', () => ({ getProductImages: (ids: number[]) => getProductImages(ids) }));

/** Las peticiones se agrupan con un temporizador corto; esto lo deja correr. */
const esperarLote = () => new Promise(r => setTimeout(r, 80));

describe('miniaturas del listado', () => {
    beforeEach(() => {
        vi.resetModules();
        getProductImages.mockReset();
        getProductImages.mockResolvedValue([]);
    });

    it('al reemplazar una foto la vuelve a pedir', async () => {
        // Sin esto la miniatura se quedaba en blanco hasta salir de la pantalla:
        // el efecto que la pide solo corre cuando cambia el producto.
        getProductImages.mockResolvedValueOnce([[1, 'data:vieja']]);
        const mod = await import('../hooks/useProductImages');

        mod.invalidateProductImage(1);
        await esperarLote();

        expect(getProductImages).toHaveBeenCalledWith([1]);
    });

    it('no vuelve a preguntar por una foto que ya trajo', async () => {
        getProductImages.mockResolvedValueOnce([[1, 'data:foto']]);
        const mod = await import('../hooks/useProductImages');

        mod.invalidateProductImage(1);
        await esperarLote();
        expect(getProductImages).toHaveBeenCalledTimes(1);

        mod.invalidateProductImage(2);
        await esperarLote();

        // La segunda tanda pide la 2, no vuelve por la 1 que ya está en caché.
        expect(getProductImages).toHaveBeenLastCalledWith([2]);
    });

    it('recuerda también que un producto no tiene foto', async () => {
        getProductImages.mockResolvedValueOnce([]);
        const mod = await import('../hooks/useProductImages');

        mod.invalidateProductImage(9);
        await esperarLote();
        mod.invalidateProductImage(9);
        await esperarLote();

        // La segunda invalidación sí vuelve a pedirla: es lo que se pide al
        // invalidar. Lo que no debe pasar es que se pida sola una y otra vez.
        expect(getProductImages).toHaveBeenCalledTimes(2);
    });
});
