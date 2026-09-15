import type { ProductImage } from '../types';

/** Lo que hace falta de la API; se inyecta para poder probarlo sin Tauri. */
export interface ApiDeFotos {
    getProductImageList: (productId: number) => Promise<ProductImage[]>;
    addProductImage: (data: { product_id: number; photo: string; thumbnail: string }) => Promise<ProductImage>;
    deleteProductImage: (imageId: number) => Promise<void>;
    reorderProductImages: (productId: number, imageIds: number[]) => Promise<void>;
}

/**
 * Aplica lo que se hizo con la foto en la ficha del producto.
 *
 * La ficha solo maneja la foto principal, pero un producto capturado con el
 * celular trae varias. Antes, cambiar o quitar la principal desde aquí quitaba
 * **todas**: quien cambiaba la foto de una prenda capturada con tres fotos
 * borraba las otras dos sin saberlo. Ahora solo se toca la principal.
 *
 * - `nueva` con foto: entra como principal y las demás se quedan. Solo si ya no
 *   cabe se quita la principal vieja, que es la que se está reemplazando.
 * - `nueva` en null: se quita la principal y la siguiente toma su lugar.
 *
 * Lo que se quita no se pierde: la base lo guarda en su archivo.
 */
export async function aplicarFotoPrincipal(
    api: ApiDeFotos,
    productId: number,
    nueva: { photo: string; thumbnail: string } | null,
    maxFotos: number,
): Promise<void> {
    const previas = [...(await api.getProductImageList(productId))]
        .sort((a, b) => a.position - b.position || a.id - b.id);
    const principal = previas[0];

    if (!nueva) {
        if (principal) await api.deleteProductImage(principal.id);
        return;
    }

    // Primero entra la nueva: si la subida falla, la principal vieja sigue ahí.
    let quedan = previas;
    if (previas.length >= maxFotos && principal) {
        await api.deleteProductImage(principal.id);
        quedan = previas.slice(1);
    }
    const agregada = await api.addProductImage({ product_id: productId, ...nueva });
    await api.reorderProductImages(productId, [agregada.id, ...quedan.map(i => i.id)]);
}
