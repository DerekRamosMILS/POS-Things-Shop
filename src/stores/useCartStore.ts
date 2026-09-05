import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { almacenSeguro } from './almacen';
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

// El carrito se guarda igual que las órdenes en espera. Si la aplicación se
// cierra —un corte de luz, un cierre por error— con la venta a medias, volver a
// abrirla la encuentra tal cual en vez de obligar a rearmarla con la fila
// esperando.
export const useCartStore = create<CartStore>()(persist((set, get) => ({
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
                items: state.items.map((i) => {
                    if (cartLineId(i) !== lineId) return i;
                    const q = Math.min(quantity, stockOf(i));
                    // El descuento se capturó contra la cantidad anterior. Al
                    // bajarla podía acabar valiendo más que la línea entera y
                    // el total en pantalla se iba a negativo, aunque el
                    // servidor luego lo recortara al cobrar.
                    return { ...i, quantity: q, discount: Math.min(i.discount, i.product.sale_price * q) };
                }),
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
}), { name: 'things-shop-cart', storage: almacenSeguro }));
