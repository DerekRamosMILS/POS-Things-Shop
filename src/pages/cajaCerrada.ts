/**
 * Lo que el backend NO deja hacer con el turno cerrado.
 *
 * El letrero de la pantalla de Caja decía "las ventas pueden seguir operando":
 * falso. `registrar_venta` corta en seco sin turno abierto. Un cajero que leyera
 * eso a las nueve de la mañana se enteraría de lo contrario con el cliente
 * enfrente. El texto se arma desde esta lista y `cajaCerrada.test.ts` la ata a
 * los candados de Rust, para que nadie mueva uno sin mover el otro.
 */
export const BLOQUEADO_SIN_CAJA = [
    'cobrar ventas',
    'registrar gastos',
    'recibir abonos en efectivo',
    'devolver en efectivo',
] as const;

export const AVISO_CAJA_CERRADA =
    `Mientras el turno esté cerrado no se puede ${BLOQUEADO_SIN_CAJA.slice(0, -1).join(', ')} ` +
    `ni ${BLOQUEADO_SIN_CAJA[BLOQUEADO_SIN_CAJA.length - 1]}. ` +
    'Los abonos y devoluciones con tarjeta o transferencia sí pasan, porque no tocan el cajón.';
