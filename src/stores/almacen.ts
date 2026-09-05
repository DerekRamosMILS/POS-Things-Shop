import { createJSONStorage } from 'zustand/middleware';

/**
 * Dónde se guarda lo que tiene que sobrevivir a cerrar la aplicación.
 *
 * `localStorage` normalmente está ahí, pero no siempre: un navegador con los
 * datos de sitio bloqueados, una ventana privada o el entorno de las pruebas
 * lo dejan sin definir, y el acceso puede incluso lanzar. Que no se pueda
 * recordar el carrito es un inconveniente; que la caja no abra por eso, no.
 * Cuando falta se usa memoria: se pierde al cerrar, que es justo lo que pasaba
 * antes de todos modos.
 */
export const almacenSeguro = createJSONStorage(() => {
    try {
        if (typeof localStorage !== 'undefined' && localStorage) {
            // Una lectura de prueba: algunos navegadores lo exponen y lanzan al usarlo.
            localStorage.getItem('__prueba__');
            return localStorage;
        }
    } catch {
        // Sigue al respaldo en memoria.
    }

    const memoria = new Map<string, string>();
    return {
        getItem: (k: string) => memoria.get(k) ?? null,
        setItem: (k: string, v: string) => { memoria.set(k, v); },
        removeItem: (k: string) => { memoria.delete(k); },
    };
});
