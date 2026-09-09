import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { almacenSeguro } from './almacen';
import { useCartStore } from './useCartStore';
import { useHoldsStore } from './useHoldsStore';
import type { User } from '../types';

interface SessionStore {
    user: User | null;
    token: string | null;
    cashRegisterId: number | null;
    setSession: (user: User, token: string) => void;
    setCashRegisterId: (id: number | null) => void;
    logout: () => void;
    isAdmin: () => boolean;
    isLoggedIn: () => boolean;
}

export const useSessionStore = create<SessionStore>()(
    persist(
        (set, get) => ({
            user: null,
            token: null,
            cashRegisterId: null,

            setSession: (user: User, token: string) => {
                set({ user, token });
            },

            setCashRegisterId: (id: number | null) => {
                set({ cashRegisterId: id });
            },

            // El carrito y las órdenes en espera sobreviven a cerrar la
            // aplicación, que es lo que se quiere para un corte de luz. Pero son
            // de quien las armó: al cambiar de turno, la siguiente persona no
            // debe encontrarse el ticket a medias de la anterior y cobrarlo sin
            // darse cuenta.
            //
            // Se limpian aquí y no en el botón de salir porque hay más de una
            // salida: la sesión que venció al arrancar y el botón de la pantalla
            // de cambio de contraseña también terminan aquí, y por esas dos el
            // ticket ajeno se quedaba en pantalla.
            logout: () => {
                useCartStore.getState().clear();
                useHoldsStore.getState().setHolds([null, null, null]);
                set({ user: null, token: null, cashRegisterId: null });
            },

            isAdmin: () => get().user?.role === 'admin',

            isLoggedIn: () => get().user !== null && get().token !== null,
        }),
        {
            name: 'things-shop-session',
            storage: almacenSeguro,
        }
    )
);
