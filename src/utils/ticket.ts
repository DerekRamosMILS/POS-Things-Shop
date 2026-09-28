/**
 * Las cuentas del ticket, hechas igual que las hace el backend (`registrar_venta`
 * y `promotion_discount` en `sales.rs`): en centavos enteros y con el mismo
 * redondeo en cada paso.
 *
 * El cobro manda el total que vio el cajero y el backend se niega si el suyo
 * difiere en más de un centavo. Por eso no basta con parecerse: la pantalla
 * multiplicaba el precio tal cual y redondeaba al final, y con un precio de
 * 10.005 por diez piezas mostraba $100.05 donde el backend cobra $100.10.
 * `cuentasDelTicket.test.ts` y una prueba de `sales.rs` comparten los casos.
 */

export interface LineaDeTicket {
    precio: number;
    cantidad: number;
    descuento: number;
    /** Si la promoción alcanza esta línea. */
    enPromo: boolean;
}

export interface PromoDeTicket {
    tipo: 'percentage' | 'fixed';
    valor: number;
}

export interface CuentasDelTicket {
    subtotal: number;
    descuentoDeLineas: number;
    descuentoDePromo: number;
    impuesto: number;
    total: number;
}

/** `Cents::from_pesos`: al centavo más cercano, las mitades lejos del cero. */
export function aCentavos(pesos: number): number {
    if (!Number.isFinite(pesos)) return 0;
    const x = pesos * 100;
    return Math.sign(x) * Math.round(Math.abs(x));
}

/** `Cents::percent`: el porcentaje en centésimas y el redondeo en enteros. */
export function porcentajeDe(centavos: number, porcentaje: number): number {
    if (!Number.isFinite(porcentaje) || porcentaje <= 0) return 0;
    const base = aCentavos(porcentaje);
    const producto = centavos * base;
    return Math.trunc((producto + 5000 * Math.sign(producto)) / 10000);
}

/** Lo que vale una línea antes de descuentos, en centavos. */
export function brutoDeLinea(precio: number, cantidad: number): number {
    return aCentavos(precio) * cantidad;
}

export function cuentasDelTicket(lineas: LineaDeTicket[], promo: PromoDeTicket | null, tasa: number): CuentasDelTicket {
    let subtotal = 0;
    let descuentoDeLineas = 0;
    let basePromo = 0;
    for (const l of lineas) {
        const bruto = brutoDeLinea(l.precio, l.cantidad);
        const descuento = Math.min(Math.max(aCentavos(l.descuento), 0), bruto);
        subtotal += bruto;
        descuentoDeLineas += descuento;
        if (l.enPromo) basePromo += bruto - descuento;
    }

    let descuentoDePromo = 0;
    if (promo && promo.valor > 0 && basePromo > 0) {
        descuentoDePromo = promo.tipo === 'percentage'
            ? porcentajeDe(basePromo, Math.min(promo.valor, 100))
            : Math.min(aCentavos(promo.valor), basePromo);
    }

    const descuento = Math.min(descuentoDeLineas + descuentoDePromo, subtotal);
    const gravable = Math.max(subtotal - descuento, 0);
    const impuesto = porcentajeDe(gravable, tasa);
    const pesos = (c: number) => c / 100;
    return {
        subtotal: pesos(subtotal),
        descuentoDeLineas: pesos(descuentoDeLineas),
        descuentoDePromo: pesos(descuentoDePromo),
        impuesto: pesos(impuesto),
        total: pesos(gravable + impuesto),
    };
}
