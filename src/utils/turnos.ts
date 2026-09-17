/**
 * Numera pedidos que compiten entre sí para quedarse solo con el último.
 *
 * Al teclear rápido en el buscador salen varias búsquedas; la de "ves" puede
 * contestar después que la de "vestido", y sin esto sus resultados pisaban a
 * los buenos.
 */
export function crearTurnos() {
    let actual = 0;
    return {
        siguiente: () => ++actual,
        vigente: (turno: number) => turno === actual,
    };
}
