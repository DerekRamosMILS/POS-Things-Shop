import { create } from 'zustand';
import type { CartItem, Product } from '../types';

interface CartStore {
    items: CartItem[];
    addItem: (product: Product) => void;
    removeItem: (productId: number) => void;
    updateQuantity: (productId: number, quantity: number) => void;
    applyDiscount: (productId: number, discount: number) => void;
    clear: () => void;
    getSubtotal: () => number;
    getDiscountTotal: () => number;
    getTotal: () => number;
    getItemCount: () => number;
}

export const useCartStore = create<CartStore>((set, get) => ({
    items: [],

    addItem: (product: Product) => {
        set((state) => {
            const existing = state.items.find((i) => i.product.id === product.id);
            if (existing) {
                if (existing.quantity >= product.stock) return state;
                return {
                    items: state.items.map((i) =>
                        i.product.id === product.id
                            ? { ...i, quantity: i.quantity + 1 }
                            : i
                    ),
                };
            }
            if (product.stock <= 0) return state;
            return {
                items: [...state.items, { product, quantity: 1, discount: 0 }],
            };
        });
    },

    removeItem: (productId: number) => {
        set((state) => ({
            items: state.items.filter((i) => i.product.id !== productId),
        }));
    },

    updateQuantity: (productId: number, quantity: number) => {
        set((state) => {
            if (quantity <= 0) {
                return { items: state.items.filter((i) => i.product.id !== productId) };
            }
            return {
                items: state.items.map((i) =>
                    i.product.id === productId
                        ? { ...i, quantity: Math.min(quantity, i.product.stock) }
                        : i
                ),
            };
        });
    },

    applyDiscount: (productId: number, discount: number) => {
        set((state) => ({
            items: state.items.map((i) =>
                i.product.id === productId ? { ...i, discount } : i
            ),
        }));
    },

    clear: () => set({ items: [] }),

    getSubtotal: () => {
        return get().items.reduce(
            (sum, item) => sum + item.product.sale_price * item.quantity,
            0
        );
    },

    getDiscountTotal: () => {
        return get().items.reduce((sum, item) => sum + item.discount, 0);
    },

    getTotal: () => {
        return get().getSubtotal() - get().getDiscountTotal();
    },

    getItemCount: () => {
        return get().items.reduce((sum, item) => sum + item.quantity, 0);
    },
}));
