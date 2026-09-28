import type { Supplier } from '../types';

export interface OpcionDeProveedor {
    id: number;
    etiqueta: string;
}

/**
 * Proveedores que ofrece la ficha de un producto.
 *
 * Sólo los activos, más el que el producto ya tiene aunque se haya dado de
 * baja. Filtrando sólo activos, el selector no encontraba ese proveedor y
 * enseñaba "Sin proveedor" mientras la tabla de productos decía el nombre.
 */
export function proveedoresParaLaFicha(proveedores: Supplier[], actual: number | null): OpcionDeProveedor[] {
    return proveedores
        .filter(p => p.is_active || p.id === actual)
        .map(p => ({ id: p.id, etiqueta: p.is_active ? p.name : `${p.name} (dado de baja)` }));
}
