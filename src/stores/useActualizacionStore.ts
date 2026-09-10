import { create } from 'zustand';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { useCartStore } from './useCartStore';
import { useSessionStore } from './useSessionStore';

/**
 * Actualizaciones automáticas.
 *
 * La tienda está lejos y nadie de allá va a instalar nada: los arreglos tienen
 * que llegar solos. Lo delicado no es bajar el instalador, es *cuándo* dejarlo
 * correr, porque en Windows el instalador mata la aplicación para poder
 * reemplazarla. Reiniciar a media venta es lo único que no puede pasar.
 *
 * De ahí las dos ventanas:
 *
 * - **Al arrancar, antes de que alguien entre.** Nadie está vendiendo, así que
 *   se instala sin preguntar. Es la ventana limpia y la que va a atrapar casi
 *   todas las actualizaciones, porque la app se abre todos los días.
 * - **Durante el día, solo si no hay nada en curso.** Se baja en silencio y se
 *   espera a que el carrito esté vacío y la caja cerrada. Si están vendiendo, no
 *   pasa nada: se queda lista y entra cuando se desocupen o al día siguiente.
 *
 * Cuando no hay internet, `check` falla y aquí no pasa nada. Que no se pueda
 * actualizar es un inconveniente; que la caja no abra por eso, no.
 */

/** Cuánto se espera a GitHub antes de rendirse. La tienda no puede esperar. */
const TIMEOUT_MS = 15_000;

/** Cada cuánto se vuelve a mirar si hay algo nuevo. */
export const INTERVALO_MS = 4 * 60 * 60 * 1000;

export type Fase = 'inactivo' | 'buscando' | 'descargando' | 'lista' | 'instalando';

interface ActualizacionStore {
    fase: Fase;
    /** Versión disponible, cuando hay una. */
    version: string | null;
    /** Avance de la descarga, 0 a 1. Es -1 cuando no se conoce el tamaño. */
    progreso: number;
    /** Último fallo, solo para poder decirlo si alguien pregunta. */
    error: string | null;

    /** Busca y, si hay algo, la deja descargada y lista. */
    buscar: () => Promise<boolean>;
    /** Instala lo que ya está listo. La aplicación se cierra sola. */
    instalar: () => Promise<void>;
}

/** El handle vive fuera del estado: no es serializable ni hay que redibujarlo. */
let pendiente: Update | null = null;

/**
 * Si este es un momento en el que se puede reiniciar la aplicación.
 *
 * Un carrito con algo dentro es una venta a medias. Una caja abierta es un turno
 * en curso: reiniciar no perdería datos —el corte vive en la base— pero deja al
 * mostrador mirando una pantalla que se fue, y eso frente a un cliente no.
 */
export function esMomentoSeguro(): boolean {
    const { items } = useCartStore.getState();
    const { cashRegisterId, user } = useSessionStore.getState();
    if (items.length > 0) return false;
    // Nadie dentro: la ventana más limpia que hay.
    if (!user) return true;
    return cashRegisterId === null;
}

export const useActualizacionStore = create<ActualizacionStore>((set, get) => ({
    fase: 'inactivo',
    version: null,
    progreso: 0,
    error: null,

    buscar: async () => {
        // Ya hay una lista o una en curso: no se pisa.
        if (get().fase !== 'inactivo') return get().fase === 'lista';

        set({ fase: 'buscando', error: null });
        let update: Update | null = null;
        try {
            update = await check({ timeout: TIMEOUT_MS });
        } catch (err) {
            // Sin internet, GitHub caído, o todavía no hay ningún release: el
            // endpoint contesta 404 y esto lanza. Ninguno es motivo de alarma.
            set({ fase: 'inactivo', error: String(err) });
            return false;
        }

        if (!update) {
            set({ fase: 'inactivo', version: null });
            return false;
        }

        set({ fase: 'descargando', version: update.version, progreso: 0 });
        try {
            let total = 0;
            let bajado = 0;
            await update.download((evento) => {
                if (evento.event === 'Started') {
                    total = evento.data.contentLength ?? 0;
                    set({ progreso: total > 0 ? 0 : -1 });
                } else if (evento.event === 'Progress') {
                    bajado += evento.data.chunkLength;
                    if (total > 0) set({ progreso: Math.min(1, bajado / total) });
                } else if (evento.event === 'Finished') {
                    set({ progreso: 1 });
                }
            });
        } catch (err) {
            // Se descarta el handle: reintentar con uno a medias no funciona.
            pendiente = null;
            set({ fase: 'inactivo', error: String(err), progreso: 0 });
            return false;
        }

        pendiente = update;
        set({ fase: 'lista' });
        return true;
    },

    instalar: async () => {
        if (!pendiente || get().fase !== 'lista') return;
        set({ fase: 'instalando' });
        try {
            await pendiente.install();
            // En Windows el instalador cierra la aplicación por su cuenta, así
            // que esto casi nunca alcanza a correr. Se llama igual porque cuando
            // no lo hace, sin esto la app se quedaría cerrada.
            await relaunch();
        } catch (err) {
            set({ fase: 'lista', error: String(err) });
        }
    },
}));
