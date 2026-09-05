import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { almacenSeguro } from './almacen';
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

            logout: () => {
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
