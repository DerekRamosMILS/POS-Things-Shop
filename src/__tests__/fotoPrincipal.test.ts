/**
 * Cambiar la foto principal desde la ficha del producto.
 *
 * La ficha maneja una sola foto, pero lo capturado con el celular trae varias.
 * Cambiar o quitar la principal quitaba todas: las demás fotos se perdían sin
 * que nadie lo pidiera.
 */
import { describe, expect, it } from 'vitest';
import { aplicarFotoPrincipal, type ApiDeFotos } from '../utils/fotoPrincipal';
import type { ProductImage } from '../types';

/** Un producto con N fotos, en memoria, que se comporta como el backend. */
function productoConFotos(n: number) {
    let siguiente = 100;
    let fotos: ProductImage[] = Array.from({ length: n }, (_, i) => ({
        id: i + 1, product_id: 1, position: i, created_at: '',
    }));
    const quitadas: number[] = [];
    const api: ApiDeFotos = {
        getProductImageList: async () => fotos.map(f => ({ ...f })),
        addProductImage: async () => {
            const f = { id: siguiente++, product_id: 1, position: fotos.length, created_at: '' };
            fotos.push(f);
            return { ...f };
        },
        deleteProductImage: async (id) => {
            quitadas.push(id);
            fotos = fotos.filter(f => f.id !== id);
        },
        reorderProductImages: async (_p, ids) => {
            fotos = ids.map((id, position) => ({ ...fotos.find(f => f.id === id)!, position }));
        },
    };
    const orden = () => [...fotos].sort((a, b) => a.position - b.position).map(f => f.id);
    return { api, quitadas, orden };
}

const nueva = { photo: 'data:grande', thumbnail: 'data:chica' };

describe('la foto principal de la ficha', () => {
    it('cambiarla no se lleva las demás fotos', async () => {
        const p = productoConFotos(3);

        await aplicarFotoPrincipal(p.api, 1, nueva, 8);

        expect(p.quitadas).toEqual([]);
        expect(p.orden()).toEqual([100, 1, 2, 3]);
    });

    it('quitarla quita solo la principal, y la siguiente toma su lugar', async () => {
        const p = productoConFotos(3);

        await aplicarFotoPrincipal(p.api, 1, null, 8);

        expect(p.quitadas).toEqual([1]);
        expect(p.orden()).toEqual([2, 3]);
    });

    it('con el cupo lleno reemplaza la principal y deja las demás', async () => {
        const p = productoConFotos(8);

        await aplicarFotoPrincipal(p.api, 1, nueva, 8);

        expect(p.quitadas).toEqual([1]);
        expect(p.orden()).toEqual([100, 2, 3, 4, 5, 6, 7, 8]);
    });

    it('un producto sin fotos recibe la primera', async () => {
        const p = productoConFotos(0);

        await aplicarFotoPrincipal(p.api, 1, nueva, 8);

        expect(p.orden()).toEqual([100]);
    });

    it('si la subida falla, la principal vieja sigue ahí', async () => {
        const p = productoConFotos(2);
        p.api.addProductImage = async () => { throw new Error('sin espacio'); };

        await expect(aplicarFotoPrincipal(p.api, 1, nueva, 8)).rejects.toThrow('sin espacio');

        expect(p.quitadas).toEqual([]);
        expect(p.orden()).toEqual([1, 2]);
    });
});
