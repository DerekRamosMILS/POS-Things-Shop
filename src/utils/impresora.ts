/**
 * Lo que se le dice a la tienda después de una prueba de impresión.
 *
 * La prueba sale por la impresora **elegida en pantalla**, y así debe ser: probar
 * antes de guardar es justo para lo que sirve. Pero el ticket de verdad lo manda
 * el backend a la que está guardada en la base. Sin decirlo, alguien elige una
 * impresora, prueba, sale el ticket, cierra Ajustes convencido de que quedó — y
 * los tickets reales siguen saliendo por la de antes, o por ninguna.
 */
export function mensajeDePrueba({ conCajon, sinGuardar }: { conCajon: boolean; sinGuardar: boolean }): string {
    const base = conCajon ? 'Ticket enviado y cajón accionado' : 'Ticket de prueba enviado';
    if (!sinGuardar) return base;
    return `${base}. Salió por la impresora que elegiste, que todavía no está guardada: guarda los cambios para que los tickets de verdad salgan por ahí.`;
}
