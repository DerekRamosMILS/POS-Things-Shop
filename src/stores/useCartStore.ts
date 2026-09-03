import { create } from 'zustand';
import type { CartItem, CartVariant, Product } from '../types';

// A cart line is identified by product + variant, so the same product can appear
// as multiple lines (e.g. size M and size L).
export function lineIdOf(productId: number, variantId: number | null | undefined): string {
    return variantId ? `${productId}:${variantId}` : `p${productId}`;
}
export function cartLineId(item: CartItem): string {
    return lineIdOf(item.product.id, item.variant?.id ?? null);
}
export function stockOf(item: CartItem): number {
    return item.variant ? item.variant.stock : item.product.stock;
}

interface CartStore {
    items: CartItem[];
    addItem: (product: Product, variant?: CartVariant | null) => void;
    removeItem: (lineId: string) => void;
    updateQuantity: (lineId: string, quantity: number) => void;
    applyDiscount: (lineId: string, discount: number) => void;
    clear: () => void;
    restoreItems: (cartItems: CartItem[]) => void;
    getSubtotal: () => number;
    getDiscountTotal: () => number;
    getTotal: () => number;
    getItemCount: () => number;
}

export const useCartStore = create<CartStore>((set, get) => ({
    items: [],

    addItem: (product: Product, variant?: CartVariant | null) => {
        set((state) => {
            const v = variant ?? null;
            const lineId = lineIdOf(product.id, v?.id ?? null);
            const stock = v ? v.stock : product.stock;
            const existing = state.items.find((i) => cartLineId(i) === lineId);
            if (existing) {
                if (existing.quantity >= stock) return state;
                return {
                    items: state.items.map((i) =>
                        cartLineId(i) === lineId ? { ...i, quantity: i.quantity + 1 } : i
                    ),
                };
            }
            if (stock <= 0) return state;
            return { items: [...state.items, { product, variant: v, quantity: 1, discount: 0 }] };
        });
    },

    removeItem: (lineId: string) => {
        set((state) => ({ items: state.items.filter((i) => cartLineId(i) !== lineId) }));
    },

    updateQuantity: (lineId: string, quantity: number) => {
        set((state) => {
            if (quantity <= 0) {
                return { items: state.items.filter((i) => cartLineId(i) !== lineId) };
            }
            return {
                items: state.items.map((i) =>
                    cartLineId(i) === lineId ? { ...i, quantity: Math.min(quantity, stockOf(i)) } : i
                ),
            };
        });
    },

    applyDiscount: (lineId: string, discount: number) => {
        set((state) => ({
            items: state.items.map((i) => (cartLineId(i) === lineId ? { ...i, discount } : i)),
        }));
    },

    clear: () => set({ items: [] }),

    restoreItems: (cartItems: CartItem[]) => set({ items: cartItems.map((i) => ({ ...i })) }),

    getSubtotal: () =>
        get().items.reduce((sum, item) => sum + item.product.sale_price * item.quantity, 0),

    getDiscountTotal: () => get().items.reduce((sum, item) => sum + item.discount, 0),

    getTotal: () => get().getSubtotal() - get().getDiscountTotal(),

    getItemCount: () => get().items.reduce((sum, item) => sum + item.quantity, 0),
}));
