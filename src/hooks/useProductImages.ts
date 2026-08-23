import { useEffect, useState } from 'react';
import * as api from '../api';

/**
 * Carga las fotos de producto bajo demanda.
 *
 * Las fotos viven como data URL dentro de la fila del producto (~54 KB cada
 * una), así que los listados ya no las devuelven: traer 500 productos con foto
 * movería decenas de megabytes cada vez que se abre el punto de venta. Aquí se
 * piden solo las de los productos visibles, por lote, y se recuerdan mientras la
 * aplicación siga abierta.
 */

const cache = new Map<number, string | null>();
let pending = new Set<number>();
let flushTimer: ReturnType<typeof setTimeout> | null = null;
const listeners = new Set<() => void>();

/** Tope alineado con el que impone el backend. */
const BATCH_LIMIT = 60;

async function flush() {
    flushTimer = null;
    const ids = [...pending].slice(0, BATCH_LIMIT);
    pending = new Set([...pending].slice(BATCH_LIMIT));

    if (ids.length === 0) return;

    try {
        const rows = await api.getProductImages(ids);
        const found = new Map(rows);
        // Recordar también los que no tienen foto evita volver a preguntarlas.
        for (const id of ids) cache.set(id, found.get(id) ?? null);
    } catch {
        // Una foto que no carga no debe romper la venta: se queda con iniciales.
        for (const id of ids) cache.set(id, null);
    }

    listeners.forEach(fn => fn());
    if (pending.size > 0) schedule();
}

function schedule() {
    if (flushTimer) return;
    // Agrupa las peticiones de un mismo render en un solo viaje.
    flushTimer = setTimeout(flush, 40);
}

function request(id: number) {
    if (cache.has(id) || pending.has(id)) return;
    pending.add(id);
    schedule();
}

/** Olvida la foto de un producto tras editarla, para que se vuelva a pedir. */
export function invalidateProductImage(id: number) {
    cache.delete(id);
    listeners.forEach(fn => fn());
}

/**
 * Devuelve la foto de un producto, pidiéndola si hace falta.
 * `hasImage` viene del listado y evita preguntar por productos sin foto.
 */
export function useProductImage(productId: number, hasImage: boolean): string | null {
    const [, force] = useState(0);

    useEffect(() => {
        if (!hasImage) return;
        const notify = () => force(n => n + 1);
        listeners.add(notify);
        request(productId);
        return () => { listeners.delete(notify); };
    }, [productId, hasImage]);

    if (!hasImage) return null;
    return cache.get(productId) ?? null;
}
