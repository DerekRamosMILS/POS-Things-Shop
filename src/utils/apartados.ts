import { formatDate } from './index';

export interface Vencimiento {
    texto: string;
    vencido: boolean;
}

/**
 * Cuándo vence un apartado y si ya venció.
 *
 * La fecha límite se pedía al crearlo y no se enseñaba en ningún lado. Vence al
 * terminar su último día, y sólo mientras siga activo: uno entregado o
 * cancelado ya no tiene nada que vencer. `hoy` va como "AAAA-MM-DD" local.
 */
export function vencimientoDeApartado(dueDate: string | null, status: string, hoy: string): Vencimiento {
    if (!dueDate) return { texto: '—', vencido: false };
    const dia = dueDate.slice(0, 10);
    return { texto: formatDate(dia), vencido: status === 'active' && dia < hoy };
}
