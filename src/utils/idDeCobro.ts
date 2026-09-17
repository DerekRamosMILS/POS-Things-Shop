import type { CartItem } from '../types';

/**
 * Qué se está cobrando, reducido a texto para poder compararlo.
 *
 * Solo entra lo que define la compra —qué, cuánto, con qué descuento, para
 * quién—, no cómo se paga: si el primer intento sí se cobró con tarjeta y la
 * respuesta se perdió, reintentar en efectivo tiene que encontrar esa venta, no
 * cobrarla otra vez.
 */
export function firmaDelCobro(
    items: CartItem[],
    promotionId: number | null,
    customerId: number | null,
    requiereFactura: boolean,
): string {
    const renglones = items
        .map(i => [i.product.id, i.variant?.id ?? null, i.quantity, Math.round(i.discount * 100)])
        .sort((a, b) => String(a).localeCompare(String(b)));
    return JSON.stringify({ renglones, promotionId, customerId, requiereFactura });
}

export interface IdentidadDeCobro {
    id: string;
    firma: string;
}

/**
 * El identificador para este intento de cobro.
 *
 * Se conserva mientras se reintente lo mismo —así un cobro que sí entró pero
 * cuya respuesta se perdió no se duplica— y se renueva en cuanto cambia lo que
 * se cobra. Antes se conservaba siempre hasta un cobro exitoso: agregar un
 * artículo tras un error y volver a cobrar devolvía la venta anterior, lo nuevo
 * no se cobraba y la pantalla decía que todo salió bien.
 */
export function identidadParaCobrar(
    anterior: IdentidadDeCobro | null,
    firma: string,
    nuevoId: () => string,
): IdentidadDeCobro {
    if (anterior && anterior.firma === firma) return anterior;
    return { id: nuevoId(), firma };
}
