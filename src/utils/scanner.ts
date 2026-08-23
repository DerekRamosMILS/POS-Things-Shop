/**
 * Captura de lecturas de un lector de código de barras.
 *
 * Prácticamente todos los lectores del mercado se presentan al sistema como un
 * teclado: "teclean" el código y terminan con Enter, Tab o nada. Lo que cambia
 * entre modelos es solo la velocidad, el sufijo y a veces un prefijo — por eso
 * todo eso es configurable en lugar de estar fijo en el código.
 *
 * Lo que distingue una lectura de alguien escribiendo no es el contenido sino el
 * ritmo: un lector manda todos los caracteres con huecos de milisegundos, cosa
 * que una persona no puede reproducir.
 */

export type ScannerSuffix = 'enter' | 'tab' | 'none';

export interface ScannerConfig {
    enabled: boolean;
    suffix: ScannerSuffix;
    prefix: string;
    /** Máximo de milisegundos entre teclas para seguir considerándolo una lectura. */
    maxGapMs: number;
    minLength: number;
}

export const DEFAULT_SCANNER_CONFIG: ScannerConfig = {
    enabled: true,
    suffix: 'enter',
    prefix: '',
    maxGapMs: 60,
    minLength: 4,
};

export function configFromSettings(values: Record<string, string>): ScannerConfig {
    const suffix = values.scanner_suffix as ScannerSuffix;
    const gap = Number(values.scanner_max_gap_ms);
    const min = Number(values.scanner_min_length);

    return {
        enabled: values.scanner_enabled !== '0',
        suffix: suffix === 'tab' || suffix === 'none' ? suffix : 'enter',
        prefix: values.scanner_prefix ?? '',
        maxGapMs: Number.isFinite(gap) && gap >= 10 && gap <= 500 ? gap : DEFAULT_SCANNER_CONFIG.maxGapMs,
        minLength: Number.isFinite(min) && min >= 1 && min <= 64 ? min : DEFAULT_SCANNER_CONFIG.minLength,
    };
}

/** Quita el prefijo que algunos lectores anteponen a cada lectura. */
export function stripPrefix(code: string, prefix: string): string {
    if (prefix && code.startsWith(prefix)) return code.slice(prefix.length);
    return code;
}

interface Burst {
    chars: string[];
    startedAt: number;
    lastAt: number;
    /** Campo enfocado al iniciar la ráfaga y su valor, para poder deshacer. */
    field: HTMLInputElement | HTMLTextAreaElement | null;
    fieldValue: string;
}

export interface ScannerEvents {
    onScan: (code: string) => void;
    getConfig: () => ScannerConfig;
}

/**
 * Devuelve un manejador de `keydown` que reconoce lecturas del escáner.
 *
 * A diferencia de ignorar los eventos cuando hay un campo enfocado —que deja el
 * caso más común del mostrador sin funcionar, porque el buscador casi siempre
 * tiene el foco—, aquí se captura siempre y, si resultó ser una lectura, se
 * restaura el campo a como estaba.
 */
export function createScannerHandler({ onScan, getConfig }: ScannerEvents) {
    let burst: Burst | null = null;
    let idleTimer: ReturnType<typeof setTimeout> | null = null;

    const reset = () => {
        burst = null;
        if (idleTimer) { clearTimeout(idleTimer); idleTimer = null; }
    };

    const commit = (config: ScannerConfig) => {
        if (!burst) return;
        const raw = burst.chars.join('');
        const field = burst.field;
        const fieldValue = burst.fieldValue;
        const code = stripPrefix(raw, config.prefix);
        reset();

        if (code.length < config.minLength) return;

        // El lector escribió dentro de un campo: devuélvelo a como estaba para
        // que el código no quede pegado en el buscador o en las notas.
        if (field) {
            field.value = fieldValue;
            field.dispatchEvent(new Event('input', { bubbles: true }));
        }
        onScan(code);
    };

    return function handleKeyDown(e: KeyboardEvent) {
        const config = getConfig();
        if (!config.enabled) return;
        if (e.ctrlKey || e.metaKey || e.altKey) { reset(); return; }

        const now = performance.now();
        const gapExceeded = burst !== null && now - burst.lastAt > config.maxGapMs;
        if (gapExceeded) reset();

        const terminator =
            (config.suffix === 'enter' && e.key === 'Enter') ||
            (config.suffix === 'tab' && e.key === 'Tab');

        if (terminator) {
            if (burst && burst.chars.length >= config.minLength) {
                e.preventDefault();
                commit(config);
            } else {
                reset();
            }
            return;
        }

        if (e.key.length !== 1) {
            // Teclas de control ajenas a la lectura (flechas, F2, Escape…).
            if (e.key !== 'Shift') reset();
            return;
        }

        if (!burst) {
            const target = e.target as HTMLElement | null;
            const isField =
                target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
            burst = {
                chars: [],
                startedAt: now,
                lastAt: now,
                field: isField ? (target as HTMLInputElement) : null,
                fieldValue: isField ? (target as HTMLInputElement).value : '',
            };
        }

        burst.chars.push(e.key);
        burst.lastAt = now;

        // Sin sufijo, la lectura se cierra cuando el lector deja de teclear.
        if (config.suffix === 'none') {
            if (idleTimer) clearTimeout(idleTimer);
            idleTimer = setTimeout(() => commit(config), config.maxGapMs * 3);
        }
    };
}

// ─── Calibración ─────────────────────────────────────────────────────────────

export interface CalibrationSample {
    /** Caracteres leídos, sin el sufijo. */
    code: string;
    /** Mayor hueco observado entre teclas, en milisegundos. */
    maxGapMs: number;
    /** Duración total de la ráfaga. */
    totalMs: number;
    detectedSuffix: ScannerSuffix;
}

/**
 * Deduce la configuración a partir de una lectura real.
 *
 * El margen sobre el hueco observado evita que el lector se corte a media
 * lectura cuando la computadora está ocupada, sin llegar a confundir el tecleo
 * de una persona (que rara vez baja de 80-100 ms entre teclas).
 */
export function configFromSample(sample: CalibrationSample, base = DEFAULT_SCANNER_CONFIG): ScannerConfig {
    const suggested = Math.ceil(Math.max(sample.maxGapMs * 2.5, 25));
    return {
        ...base,
        enabled: true,
        suffix: sample.detectedSuffix,
        maxGapMs: Math.min(suggested, 250),
        minLength: Math.max(3, Math.min(sample.code.length - 2, 8)),
    };
}

/** Recolector de una lectura de prueba para la pantalla de calibración. */
export function createCalibrator(onSample: (sample: CalibrationSample) => void) {
    let chars: string[] = [];
    let times: number[] = [];
    let idleTimer: ReturnType<typeof setTimeout> | null = null;

    const emit = (detectedSuffix: ScannerSuffix) => {
        if (chars.length === 0) return;
        const gaps = times.slice(1).map((t, i) => t - times[i]);
        onSample({
            code: chars.join(''),
            maxGapMs: gaps.length > 0 ? Math.max(...gaps) : 0,
            totalMs: times.length > 1 ? times[times.length - 1] - times[0] : 0,
            detectedSuffix,
        });
        chars = [];
        times = [];
    };

    return function handleKeyDown(e: KeyboardEvent) {
        if (e.key === 'Enter' || e.key === 'Tab') {
            e.preventDefault();
            if (idleTimer) { clearTimeout(idleTimer); idleTimer = null; }
            emit(e.key === 'Enter' ? 'enter' : 'tab');
            return;
        }
        if (e.key.length !== 1) return;

        e.preventDefault();
        chars.push(e.key);
        times.push(performance.now());

        // Si el lector no manda sufijo, la lectura termina por silencio.
        if (idleTimer) clearTimeout(idleTimer);
        idleTimer = setTimeout(() => emit('none'), 300);
    };
}
