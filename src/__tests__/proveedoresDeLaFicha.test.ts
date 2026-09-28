import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { proveedoresParaLaFicha } from '../utils/proveedores';
import type { Supplier } from '../types';

const prov = (id: number, name: string, is_active: boolean): Supplier => ({
    id, name, is_active, contact_name: null, phone: null, email: null, address: null, notes: null,
    created_at: '', updated_at: '',
});

const lista = [prov(1, 'Textiles Norte', true), prov(2, 'Calzado Sur', false), prov(3, 'Gorras MX', true)];

describe('el selector de proveedor de la ficha', () => {
    it('ofrece sólo los activos a un producto nuevo', () => {
        expect(proveedoresParaLaFicha(lista, null).map(o => o.id)).toEqual([1, 3]);
    });

    it('conserva el proveedor dado de baja que el producto ya tiene', () => {
        // Sólo listaba activos: el selector no encontraba el proveedor del
        // producto y enseñaba "Sin proveedor", mientras la tabla de al lado
        // decía "Calzado Sur".
        const opciones = proveedoresParaLaFicha(lista, 2);
        expect(opciones.map(o => o.id)).toEqual([1, 2, 3]);
        expect(opciones.find(o => o.id === 2)?.etiqueta).toBe('Calzado Sur (dado de baja)');
    });

    it('la ficha de productos usa el ayudante', () => {
        const pagina = readFileSync(resolve(__dirname, '../pages/ProductsPage.tsx'), 'utf8');
        expect(pagina).toContain('proveedoresParaLaFicha(');
        expect(pagina).not.toContain('suppliers.filter(s => s.is_active).map');
    });
});
