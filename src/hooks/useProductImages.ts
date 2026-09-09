import { useEffect, useState } from 'react';
import * as api from '../api';

/**
 * Carga las fotos de producto bajo demanda.
 *
 * Las fotos son archivos en disco y los listados no las devuelven: traer 500
 * productos con foto movería decenas de megabytes cada vez que se abre el punto
 * de venta. Aquí se piden solo las de los productos visibles, por lote, y se
 * recuerdan mientras la aplicación siga abierta.
 */

const cache = new Map<number, string | null>();
/// Tope de miniaturas recordadas. Cada una son unos 16 KB de texto; sin tope,
/// pasear por un catálogo grande las va acumulando todas hasta cerrar la app.
/// Se olvidan las más viejas, que es exactamente lo que ya no se está viendo.
const MAX_EN_MEMORIA = 400;
let pending = new Set<number>();
let flushTimer: ReturnType<typeof setTimeout> | null = null;
const listeners = new Set<() => void>();

/// Cuántas miniaturas de cada producto están puestas en pantalla ahora mismo.
///
/// El desalojo miraba solo la antigüedad, y la rejilla del punto de venta no
/// está paginada: con más de cuatrocientos productos con foto se montan todos de
/// golpe, los últimos desalojaban a los primeros y esos se quedaban con las
/// iniciales hasta recargar, porque solo se vuelven a pedir al montarse. Lo que
/// está a la vista no se olvida; el tope sigue acotando lo que ya no se ve.
const montadas = new Map<number, number>();

/**
 * Apunta que una miniatura está en pantalla, y devuelve cómo dejar de apuntarlo.
 *
 * Vive aparte del hook para poder probar el desalojo sin montar componentes:
 * lo que importa es que lo visible nunca se olvide.
 */
export function marcarEnPantalla(id: number): () => void {
    montadas.set(id, (montadas.get(id) ?? 0) + 1);
    return () => {
        const quedan = (montadas.get(id) ?? 1) - 1;
        if (quedan > 0) montadas.set(id, quedan);
        else montadas.delete(id);
    };
}

/** Solo para pruebas: cuántas miniaturas se están recordando. */
export function recordadas(): number {
    return cache.size;
}

/** Solo para pruebas: si una miniatura sigue en memoria. */
export function estaRecordada(id: number): boolean {
    return cache.has(id);
}

/** Tope alineado con el que impone el backend. */
const BATCH_LIMIT = 60;

/** Guarda una miniatura y olvida las más viejas que ya nadie esté mirando. */
export function recordar(id: number, url: string | null) {
    cache.delete(id);
    cache.set(id, url);
    if (cache.size <= MAX_EN_MEMORIA) return;
    for (const candidata of [...cache.keys()]) {
        if (cache.size <= MAX_EN_MEMORIA) break;
        if (montadas.has(candidata)) continue;
        cache.delete(candidata);
    }
}

async function flush() {
    flushTimer = null;
    const ids = [...pending].slice(0, BATCH_LIMIT);
    pending = new Set([...pending].slice(BATCH_LIMIT));

    if (ids.length === 0) return;

    try {
        const rows = await api.getProductImages(ids);
        const found = new Map(rows);
        // Recordar también los que no tienen foto evita volver a preguntarlas.
        for (const id of ids) recordar(id, found.get(id) ?? null);
    } catch {
        // Una foto que no carga no debe romper la venta: se queda con iniciales.
        for (const id of ids) recordar(id, null);
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

/**
 * Olvida la foto de un producto tras editarla y la vuelve a pedir.
 *
 * Sin la petición la miniatura se quedaba en blanco: el efecto que la pide solo
 * corre cuando cambia el producto o su `hasImage`, y al reemplazar una foto no
 * cambia ninguno de los dos.
 */
export function invalidateProductImage(id: number) {
    cache.delete(id);
    request(id);
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
        const soltar = marcarEnPantalla(productId);
        request(productId);
        return () => {
            listeners.delete(notify);
            soltar();
        };
    }, [productId, hasImage]);

    if (!hasImage) return null;
    return cache.get(productId) ?? null;
}
