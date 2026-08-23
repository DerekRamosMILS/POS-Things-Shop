import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { CartItem, Promotion } from '../types';

export type ServiceType = 'direct' | 'layaway';

export interface HeldCart {
    items: CartItem[];
    customerName: string;
    orderNotes: string;
    serviceType: ServiceType;
    orderNo: number;
    promo: Promotion | null;
}

interface HoldsStore {
    holds: (HeldCart | null)[];
    setHolds: (holds: (HeldCart | null)[]) => void;
}

// Persisted so held tickets survive navigating away from the POS screen.
export const useHoldsStore = create<HoldsStore>()(
    persist(
        (set) => ({
            holds: [null, null, null],
            setHolds: (holds) => set({ holds }),
        }),
        { name: 'things-shop-holds' }
    )
);
