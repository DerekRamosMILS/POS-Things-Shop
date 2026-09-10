import { create } from 'zustand';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { useCartStore } from './useCartStore';
import { useSessionStore } from './useSessionStore';
import * as api from '../api';

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

/** Cuándo arrancó esta ejecución de la aplicación. */
const ARRANQUE = Date.now();

/**
 * Cuánto dura la ventana de arranque.
 *
 * Tiene que aguantar lo que tarda buscar y descargar el instalador —unos cuatro
 * megas por el WiFi de una tienda—, o la ventana se cierra antes de que haya
 * algo que instalar.
 */
const VENTANA_ARRANQUE_MS = 3 * 60 * 1000;

export type Fase = 'inactivo' | 'buscando' | 'descargando' | 'lista' | 'instalando';

/** Deja el rastro en la bitácora sin dejar que un fallo ahí rompa nada. */
function anotar(mensaje: string) {
    api.registrarEventoActualizacion(mensaje).catch(() => { /* la bitácora no manda */ });
}

interface ActualizacionStore {
    fase: Fase;
    /** Versión disponible, cuando hay una. */
    version: string | null;
    /** Avance de la descarga, 0 a 1. Es -1 cuando no se conoce el tamaño. */
    progreso: number;
    /** Último fallo, solo para poder decirlo si alguien pregunta. */
    error: string | null;
    /** Qué pasó la última vez que se buscó, en palabras, para Ajustes. */
    ultimaRevision: string | null;

    /** Busca y, si hay algo, la deja descargada y lista. */
    buscar: () => Promise<boolean>;
    /** Instala lo que ya está listo. La aplicación se cierra sola. */
    instalar: () => Promise<void>;
    /**
     * Busca, descarga e instala ahora, sin consultar la política de momentos.
     *
     * Es la orden de una persona que está mirando la pantalla, así que la
     * política —pensada para decidir sola— no tiene nada que opinar. Existe
     * porque la automática puede quedarse esperando por un motivo que no
     * previmos, y entonces hace falta una salida que no dependa de que nosotros
     * hayamos acertado.
     */
    forzar: () => Promise<string>;
}

/** El handle vive fuera del estado: no es serializable ni hay que redibujarlo. */
let pendiente: Update | null = null;

/**
 * Por qué no se puede instalar ahora, o null si sí se puede.
 *
 * Devuelve el motivo en vez de un sí o no para poder decirlo en Ajustes: una
 * actualización que se queda esperando sin explicar por qué es indistinguible de
 * una que no llegó, y eso a distancia no se puede depurar.
 */
export function motivoDeEspera(): string | null {
    // Un carrito con algo dentro es una venta a medias, siempre y en todo caso.
    if (useCartStore.getState().items.length > 0) {
        return 'Hay un ticket a medias';
    }

    // Recién arrancada nadie está a media operación, aunque haya sesión y turno.
    //
    // Antes esta ventana pedía además que no hubiera caja abierta, y era un error
    // de bulto: la sesión y el turno sobreviven a cerrar la aplicación, así que al
    // reabrirla el usuario ya está dentro y la caja sigue abierta. La condición
    // "nadie ha entrado todavía" no se cumplía nunca. Una tienda que deja la caja
    // abierta de la mañana a la noche —o sea, cualquier tienda— no se actualizaba
    // jamás: la versión se descargaba y se quedaba esperando un momento que no
    // llegaba, en silencio.
    if (Date.now() - ARRANQUE < VENTANA_ARRANQUE_MS) return null;

    const { cashRegisterId, user } = useSessionStore.getState();
    if (!user) return null;
    if (cashRegisterId !== null) {
        return 'Hay un turno abierto; se instala al cerrar la caja';
    }
    return null;
}

/** Si este es un momento en el que se puede reiniciar la aplicación. */
export function esMomentoSeguro(): boolean {
    return motivoDeEspera() === null;
}

export const useActualizacionStore = create<ActualizacionStore>((set, get) => ({
    fase: 'inactivo',
    version: null,
    progreso: 0,
    error: null,
    ultimaRevision: null,

    buscar: async () => {
        // Ya hay una lista o una en curso: no se pisa.
        if (get().fase !== 'inactivo') return get().fase === 'lista';

        set({ fase: 'buscando', error: null });
        let update: Update | null = null;
        try {
            update = await check({ timeout: TIMEOUT_MS });
        } catch (err) {
            // Sin internet, GitHub caído, o todavía no hay ningún release: el
            // endpoint contesta 404 y esto lanza. Ninguno es motivo de alarma,
            // pero sí queda anotado: desde lejos, "no llegó" y "no se pudo
            // buscar" se ven igual y hay que poder distinguirlos.
            const motivo = `No se pudo buscar actualizaciones: ${err}`;
            set({ fase: 'inactivo', error: String(err), ultimaRevision: motivo });
            anotar(motivo);
            return false;
        }

        if (!update) {
            set({ fase: 'inactivo', version: null, ultimaRevision: 'Ya tiene la versión más reciente' });
            return false;
        }

        anotar(`Encontrada la versión ${update.version}; descargando`);

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
            const motivo = `No se pudo descargar la versión ${update.version}: ${err}`;
            set({ fase: 'inactivo', error: String(err), progreso: 0, ultimaRevision: motivo });
            anotar(motivo);
            return false;
        }

        pendiente = update;
        const espera = motivoDeEspera();
        set({
            fase: 'lista',
            ultimaRevision: espera
                ? `Versión ${update.version} lista, esperando: ${espera.toLowerCase()}`
                : `Versión ${update.version} lista para instalar`,
        });
        anotar(espera
            ? `Versión ${update.version} descargada; esperando porque ${espera.toLowerCase()}`
            : `Versión ${update.version} descargada; instalando`);
        return true;
    },

    forzar: async () => {
        // Si ya está descargada no se vuelve a bajar.
        if (get().fase !== 'lista') {
            if (get().fase !== 'inactivo') return 'Ya hay una actualización en curso.';
            const hay = await get().buscar();
            if (!hay) {
                return get().error
                    ? `No se pudo buscar: ${get().error}`
                    : 'Ya tienes la versión más reciente.';
            }
        }
        anotar('Actualización forzada a mano desde Ajustes');
        await get().instalar();
        return get().error ?? 'Instalando; la aplicación se va a reiniciar.';
    },

    instalar: async () => {
        if (!pendiente || get().fase !== 'lista') return;
        set({ fase: 'instalando' });
        anotar(`Instalando la versión ${get().version}`);
        try {
            await pendiente.install();
            // En Windows el instalador cierra la aplicación por su cuenta, así
            // que esto casi nunca alcanza a correr. Se llama igual porque cuando
            // no lo hace, sin esto la app se quedaría cerrada.
            await relaunch();
        } catch (err) {
            const motivo = `Falló la instalación: ${err}`;
            set({ fase: 'lista', error: String(err), ultimaRevision: motivo });
            anotar(motivo);
        }
    },
}));
