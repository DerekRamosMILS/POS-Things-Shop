/**
 * La pantalla calcula el ticket igual que el backend, centavo por centavo.
 *
 * El cobro manda el total que vio el cajero y el backend se niega si el suyo
 * difiere en más de un centavo. La pantalla multiplicaba el precio tal cual y
 * redondeaba al final; el backend redondea el precio unitario y luego
 * multiplica. Con un precio de 10.005 y diez piezas: $100.05 contra $100.00, y
 * la venta se frenaba. Promoción e impuesto también redondeaban distinto.
 *
 * Los casos viven en un archivo que lee también la prueba del backend
 * (`sales.rs`): si alguno de los dos cambia su forma de redondear, falla.
 */
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { cuentasDelTicket, type PromoDeTicket } from '../utils/ticket';

interface Caso {
    nombre: string;
    lineas: { precio: number; cantidad: number; descuento: number }[];
    promo: PromoDeTicket | null;
    tasa: number;
    total_centavos?: number;
}

const casos: Caso[] = JSON.parse(readFileSync(resolve(__dirname, '../../src-tauri/src/commands/casos_de_ticket.json'), 'utf8'));
const esperado = JSON.parse(readFileSync(resolve(__dirname, '../../src-tauri/src/commands/totales_de_ticket.json'), 'utf8')) as Record<string, number>;

describe('las cuentas del ticket', () => {
    it.each(casos)('$nombre', (caso) => {
        const c = cuentasDelTicket(caso.lineas.map(l => ({ ...l, enPromo: true })), caso.promo, caso.tasa);
        expect(Math.round(c.total * 100)).toBe(esperado[caso.nombre]);
    });

    it('cada caso tiene su total', () => {
        expect(Object.keys(esperado).sort()).toEqual(casos.map(c => c.nombre).sort());
    });

    it('el punto de venta usa estas cuentas', () => {
        const pos = readFileSync(resolve(__dirname, '../pages/POSPage.tsx'), 'utf8');
        expect(pos).toContain('cuentasDelTicket(');
    });
});
