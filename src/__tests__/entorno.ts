/**
 * Preparación del entorno de pruebas: un `localStorage` que de verdad guarde.
 *
 * El entorno de jsdom que usa vitest no expone `localStorage` —y el de Node 22 no
 * está disponible sin una bandera—, así que quedaba `undefined`. El efecto era
 * silencioso y grande: el carrito, la sesión y las órdenes en espera se guardan con
 * un almacén que cae a memoria cuando el navegador no deja, de modo que **todas**
 * las pruebas de persistencia venían ejercitando ese respaldo en vez del
 * almacenamiento real. Pasaban en verde sin comprobar lo que dicen comprobar. Y
 * `escala.ts`, que lo usa directo, no tenía forma de probarse.
 *
 * En la tienda esto no hace falta: WebView2 trae el suyo. Aquí se pone uno mínimo
 * con el mismo comportamiento observable, incluida la excepción cuando no se puede
 * escribir, que es lo que hay que saber manejar.
 */
class AlmacenDePrueba implements Storage {
    private datos = new Map<string, string>();

    get length(): number { return this.datos.size; }
    clear(): void { this.datos.clear(); }
    getItem(clave: string): string | null { return this.datos.get(String(clave)) ?? null; }
    key(i: number): string | null { return [...this.datos.keys()][i] ?? null; }
    removeItem(clave: string): void { this.datos.delete(String(clave)); }
    setItem(clave: string, valor: string): void { this.datos.set(String(clave), String(valor)); }
}

function poner(destino: object, almacen: Storage) {
    Object.defineProperty(destino, 'localStorage', {
        value: almacen, configurable: true, writable: true,
    });
}

const almacen = new AlmacenDePrueba();
poner(globalThis, almacen);
if (typeof window !== 'undefined' && window !== (globalThis as unknown as Window)) {
    poner(window, almacen);
}
