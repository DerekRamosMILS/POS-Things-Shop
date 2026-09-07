/**
 * Escala de la interfaz: qué tan grandes se ven letras e iconos.
 *
 * Está hecho con `zoom` sobre la raíz del documento y no con tamaños relativos
 * por una razón práctica: media aplicación tiene medidas en píxeles escritas
 * directamente en cada elemento, y pasarlas todas a unidades relativas serían
 * cientos de ediciones a mano, cada una una oportunidad de romper algo. `zoom`
 * agranda todo junto —texto, iconos, márgenes— y mantiene las proporciones.
 * Los dos motores a los que se apunta lo soportan: WebKit en macOS y WebView2
 * en Windows.
 *
 * Se guarda en el equipo y no en la base: es de quien mira esta pantalla, no de
 * la tienda, y tiene que aplicarse antes de que nadie inicie sesión.
 */

export const ESCALA_MINIMA = 80;
export const ESCALA_MAXIMA = 160;
export const ESCALA_POR_DEFECTO = 100;

const CLAVE = 'things-shop-escala';

/** Deja la escala en un valor usable aunque venga un ajuste corrupto o vacío. */
export function normalizarEscala(valor: string | number | null | undefined): number {
    const n = typeof valor === 'number' ? valor : Number(valor);
    if (!Number.isFinite(n) || n === 0) return ESCALA_POR_DEFECTO;
    return Math.min(ESCALA_MAXIMA, Math.max(ESCALA_MINIMA, Math.round(n)));
}

/** La escala guardada, o la de fábrica si no hay ninguna o el equipo no deja leer. */
export function escalaGuardada(): number {
    try {
        return normalizarEscala(localStorage.getItem(CLAVE));
    } catch {
        return ESCALA_POR_DEFECTO;
    }
}

/** Aplica la escala a la ventana. No la guarda: sirve para la vista previa. */
export function aplicarEscala(porcentaje: number) {
    const escala = normalizarEscala(porcentaje);
    // `zoom` no aparece en los tipos de CSSStyleDeclaration de todos los motores.
    (document.documentElement.style as unknown as Record<string, string>).zoom =
        escala === ESCALA_POR_DEFECTO ? '' : String(escala / 100);
}

/** Aplica la escala y la recuerda para los siguientes arranques. */
export function guardarEscala(porcentaje: number) {
    const escala = normalizarEscala(porcentaje);
    aplicarEscala(escala);
    try {
        localStorage.setItem(CLAVE, String(escala));
    } catch {
        // Que no se pueda recordar no debe impedir usarla ahora.
    }
}
