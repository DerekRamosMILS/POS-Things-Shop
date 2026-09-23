import type { CartItem, Product } from '../types';

/**
 * Pone al día un carrito guardado contra el catálogo de ahora.
 *
 * El carrito sobrevive a cerrar la aplicación —un corte de luz, la caja que se
 * quedó con una venta a medias— y lleva dentro una copia del producto: nombre,
 * precio y existencia tal como estaban al agregarlo. El cobro, en cambio, toma
 * el precio de la base, y hace bien: el precio que manda la pantalla se ignora a
 * propósito para que nadie pueda cobrarse de menos. El costo es que un carrito
 * viejo enseñaba un total y cobraba otro, sin una palabra.
 *
 * Aquí se sustituye la copia por la del catálogo y se dice qué cambió, para que
 * la cajera lo vea antes de cobrar y no después.
 */
export interface CambioDeCarrito {
    nombre: string;
    motivo: 'precio' | 'baja' | 'sin-stock';
    antes?: number;
    ahora?: number;
}

export function refrescarCarrito(
    items: CartItem[],
    catalogo: Product[],
): { items: CartItem[]; cambios: CambioDeCarrito[] } {
    const porId = new Map(catalogo.map(p => [p.id, p]));
    const cambios: CambioDeCarrito[] = [];
    const salida: CartItem[] = [];

    for (const item of items) {
        const fresco = porId.get(item.product.id);
        // El catálogo solo trae lo activo: si no está, se dio de baja o ya no existe.
        if (!fresco) {
            cambios.push({ nombre: item.product.name, motivo: 'baja' });
            continue;
        }

        if (fresco.sale_price !== item.product.sale_price) {
            cambios.push({
                nombre: fresco.name,
                motivo: 'precio',
                antes: item.product.sale_price,
                ahora: fresco.sale_price,
            });
        }

        // La existencia de un renglón con talla vive en la talla, no en el
        // producto: ahí el tope no se toca y el cobro rechaza lo que sobre.
        if (!item.variant) {
            if (fresco.stock <= 0) {
                cambios.push({ nombre: fresco.name, motivo: 'sin-stock' });
                continue;
            }
            const cantidad = Math.min(item.quantity, fresco.stock);
            salida.push({
                ...item,
                product: fresco,
                quantity: cantidad,
                // El descuento se capturó contra el precio viejo y la cantidad
                // vieja; nunca puede valer más que el renglón.
                discount: Math.min(item.discount, fresco.sale_price * cantidad),
            });
            continue;
        }

        salida.push({
            ...item,
            product: fresco,
            discount: Math.min(item.discount, fresco.sale_price * item.quantity),
        });
    }

    return { items: salida, cambios };
}

/** Lo que se le dice a la cajera sobre un carrito que se puso al día. */
export function avisoDeCarrito(cambios: CambioDeCarrito[]): string | null {
    if (cambios.length === 0) return null;
    const partes = cambios.map(c => {
        if (c.motivo === 'baja') return `"${c.nombre}" ya no está disponible y salió del ticket`;
        if (c.motivo === 'sin-stock') return `"${c.nombre}" se quedó sin existencia y salió del ticket`;
        return `"${c.nombre}" cambió de precio`;
    });
    return `El ticket que estaba guardado se puso al día: ${partes.join('; ')}.`;
}
