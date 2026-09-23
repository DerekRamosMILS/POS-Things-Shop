import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import {
    createScannerHandler,
    createCalibrator,
    configFromSettings,
    configFromSample,
    stripPrefix,
    DEFAULT_SCANNER_CONFIG,
    MS_POR_CARACTER_HUMANO,
    type CalibrationSample,
    type ScannerConfig,
} from '../utils/scanner';

/** Simula una ráfaga del lector: teclas con huecos de `gap` milisegundos. */
function scan(
    handler: (e: KeyboardEvent) => void,
    code: string,
    { gap = 8, suffix = 'Enter' } = {},
) {
    let clock = 1000;
    vi.spyOn(performance, 'now').mockImplementation(() => clock);

    for (const ch of code) {
        handler(new KeyboardEvent('keydown', { key: ch, bubbles: true, cancelable: true }));
        clock += gap;
    }
    if (suffix) {
        handler(new KeyboardEvent('keydown', { key: suffix, bubbles: true, cancelable: true }));
    }
}

/** Igual que `scan`, pero con el evento apuntando a un campo de texto real. */
function scanInto(
    handler: (e: KeyboardEvent) => void,
    input: HTMLInputElement,
    code: string,
    gap = 8,
) {
    let clock = 1000;
    vi.spyOn(performance, 'now').mockImplementation(() => clock);

    for (const ch of code) {
        // keydown ocurre ANTES de que el navegador inserte el carácter.
        const e = new KeyboardEvent('keydown', { key: ch, bubbles: true, cancelable: true });
        Object.defineProperty(e, 'target', { value: input });
        handler(e);
        if (!e.defaultPrevented) input.value += ch;
        clock += gap;
    }
    const enter = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true });
    Object.defineProperty(enter, 'target', { value: input });
    handler(enter);
}

const makeHandler = (over: Partial<ScannerConfig> = {}) => {
    const onScan = vi.fn();
    const config = { ...DEFAULT_SCANNER_CONFIG, ...over };
    return { onScan, handler: createScannerHandler({ onScan, getConfig: () => config }) };
};

beforeEach(() => {
    vi.useFakeTimers();
});
afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
});

describe('captura del lector', () => {
    it('reconoce una lectura EAN-13 completa', () => {
        const { onScan, handler } = makeHandler();
        scan(handler, '7501234567890');
        expect(onScan).toHaveBeenCalledWith('7501234567890');
    });

    it('ignora a una persona tecleando: los huecos son demasiado largos', () => {
        const { onScan, handler } = makeHandler();
        scan(handler, '7501234567890', { gap: 150 });
        expect(onScan).not.toHaveBeenCalled();
    });

    it('descarta lecturas más cortas que el mínimo', () => {
        const { onScan, handler } = makeHandler({ minLength: 6 });
        scan(handler, '123');
        expect(onScan).not.toHaveBeenCalled();
    });

    it('acepta lectores que terminan con Tab', () => {
        const { onScan, handler } = makeHandler({ suffix: 'tab' });
        scan(handler, '7501234567890', { suffix: 'Tab' });
        expect(onScan).toHaveBeenCalledWith('7501234567890');
    });

    it('no confunde Enter con Tab cuando el sufijo está configurado', () => {
        const { onScan, handler } = makeHandler({ suffix: 'tab' });
        scan(handler, '7501234567890', { suffix: 'Enter' });
        expect(onScan).not.toHaveBeenCalled();
    });

    it('acepta lectores sin sufijo, cerrando por silencio', () => {
        const { onScan, handler } = makeHandler({ suffix: 'none' });
        scan(handler, '7501234567890', { suffix: '' });
        vi.advanceTimersByTime(500);
        expect(onScan).toHaveBeenCalledWith('7501234567890');
    });

    it('quita el prefijo que algunos lectores anteponen', () => {
        const { onScan, handler } = makeHandler({ prefix: '*' });
        scan(handler, '*7501234567890');
        expect(onScan).toHaveBeenCalledWith('7501234567890');
    });

    it('no captura nada cuando el escáner está apagado', () => {
        const { onScan, handler } = makeHandler({ enabled: false });
        scan(handler, '7501234567890');
        expect(onScan).not.toHaveBeenCalled();
    });

    it('un atajo de teclado corta la ráfaga en vez de ensuciarla', () => {
        const { onScan, handler } = makeHandler();
        vi.spyOn(performance, 'now').mockReturnValue(1000);
        handler(new KeyboardEvent('keydown', { key: '7' }));
        handler(new KeyboardEvent('keydown', { key: 'c', ctrlKey: true }));
        scan(handler, '501234567890');
        expect(onScan).toHaveBeenCalledWith('501234567890');
    });
});

describe('lectura con el foco dentro de un campo', () => {
    it('dispara la lectura aunque el cursor esté en el buscador', () => {
        const input = document.createElement('input');
        document.body.appendChild(input);
        const { onScan, handler } = makeHandler();

        scanInto(handler, input, '7501234567890');

        expect(onScan).toHaveBeenCalledWith('7501234567890');
        input.remove();
    });

    it('deja el campo como estaba, sin el código pegado', () => {
        const input = document.createElement('input');
        input.value = 'camisa';
        document.body.appendChild(input);
        const { handler } = makeHandler();

        scanInto(handler, input, '7501234567890');

        expect(input.value).toBe('camisa');
        input.remove();
    });
});

describe('configuración', () => {
    it('lee los ajustes guardados', () => {
        const c = configFromSettings({
            scanner_enabled: '1', scanner_suffix: 'tab', scanner_prefix: '*',
            scanner_max_gap_ms: '90', scanner_min_length: '6',
        });
        expect(c).toEqual({ enabled: true, suffix: 'tab', prefix: '*', maxGapMs: 90, minLength: 6 });
    });

    it('cae en valores seguros si los ajustes están corruptos', () => {
        const c = configFromSettings({
            scanner_suffix: 'humo', scanner_max_gap_ms: 'rápido', scanner_min_length: '-3',
        });
        expect(c.suffix).toBe('enter');
        expect(c.maxGapMs).toBe(DEFAULT_SCANNER_CONFIG.maxGapMs);
        expect(c.minLength).toBe(DEFAULT_SCANNER_CONFIG.minLength);
    });

    it('stripPrefix no toca códigos que no lo llevan', () => {
        expect(stripPrefix('7501234', '*')).toBe('7501234');
        expect(stripPrefix('7501234', '')).toBe('7501234');
    });
});

describe('calibración', () => {
    it('deduce el sufijo y deja margen sobre la velocidad medida', () => {
        const c = configFromSample({
            code: '7501234567890', maxGapMs: 12, totalMs: 150, detectedSuffix: 'tab',
        });
        expect(c.suffix).toBe('tab');
        expect(c.maxGapMs).toBeGreaterThanOrEqual(30);
        expect(c.enabled).toBe(true);
    });

    it('un lector muy lento no queda con un umbral que lo corte', () => {
        const c = configFromSample({
            code: '7501234567890', maxGapMs: 45, totalMs: 600, detectedSuffix: 'enter',
        });
        expect(c.maxGapMs).toBeGreaterThan(45);
    });

    it('el umbral nunca crece tanto como para confundirse con tecleo humano', () => {
        const c = configFromSample({
            code: '750123', maxGapMs: 400, totalMs: 2000, detectedSuffix: 'enter',
        });
        expect(c.maxGapMs).toBeLessThanOrEqual(250);
    });
});

describe('tecleo humano contra lectura de escáner', () => {
    const config = { ...DEFAULT_SCANNER_CONFIG };

    /** Reproduce una tanda de teclas con un hueco dado entre cada una. */
    function teclear(
        handler: (e: KeyboardEvent) => void,
        texto: string,
        huecoMs: number,
        terminar = true,
    ) {
        let reloj = 1000;
        const original = performance.now;
        performance.now = () => reloj;
        try {
            for (const ch of texto) {
                handler({
                    key: ch, ctrlKey: false, metaKey: false, altKey: false,
                    preventDefault: () => {}, target: null,
                } as unknown as KeyboardEvent);
                reloj += huecoMs;
            }
            if (terminar) {
                handler({
                    key: 'Enter', ctrlKey: false, metaKey: false, altKey: false,
                    preventDefault: () => {}, target: null,
                } as unknown as KeyboardEvent);
            }
        } finally {
            performance.now = original;
        }
    }

    it('acepta una lectura del escáner', () => {
        const leidos: string[] = [];
        const handler = createScannerHandler({ onScan: c => leidos.push(c), getConfig: () => config });
        teclear(handler, '7501234567890', 3);
        expect(leidos).toEqual(['7501234567890']);
    });

    it('no se roba lo que alguien teclea, aunque teclee rápido', () => {
        // Cincuenta milisegundos por tecla son 240 pulsaciones por minuto: más
        // rápido que casi cualquiera, y aun así diez veces más lento que un
        // lector. Antes esto vaciaba el buscador y decía "producto no encontrado".
        const leidos: string[] = [];
        const handler = createScannerHandler({ onScan: c => leidos.push(c), getConfig: () => config });
        teclear(handler, 'vestido', 50);
        expect(leidos).toEqual([]);
    });

    it('el Enter de una persona sigue sirviendo para enviar', () => {
        let tragado = false;
        const handler = createScannerHandler({ onScan: () => {}, getConfig: () => config });
        let reloj = 1000;
        const original = performance.now;
        performance.now = () => reloj;
        try {
            for (const ch of '1000') {
                handler({ key: ch, ctrlKey: false, metaKey: false, altKey: false,
                    preventDefault: () => {}, target: null } as unknown as KeyboardEvent);
                reloj += 50;
            }
            handler({ key: 'Enter', ctrlKey: false, metaKey: false, altKey: false,
                preventDefault: () => { tragado = true; }, target: null } as unknown as KeyboardEvent);
        } finally {
            performance.now = original;
        }
        expect(tragado).toBe(false);
    });
});

describe('calibrar el lector', () => {
    /**
     * Es lo que mide un lector de verdad para configurarlo, y de esa medida sale la
     * configuración que decide si escanear funciona. Se usa una vez por tienda, así
     * que un error aquí se descubre en el mostrador y no se vuelve a mirar nunca.
     * `configFromSample` tenía pruebas; lo que **produce** la muestra, no.
     */
    let reloj = 0;
    beforeEach(() => {
        reloj = 5000;
        vi.spyOn(performance, 'now').mockImplementation(() => reloj);
    });
    afterEach(() => vi.restoreAllMocks());

    /** Teclea `code` con huecos de `gaps[i]` ms entre teclas. */
    function teclear(
        handler: (e: KeyboardEvent) => void,
        code: string,
        gaps: number[],
        sufijo: 'Enter' | 'Tab' | null,
    ) {
        code.split('').forEach((ch, i) => {
            if (i > 0) reloj += gaps[i - 1] ?? 0;
            handler(new KeyboardEvent('keydown', { key: ch, cancelable: true }));
        });
        if (sufijo) handler(new KeyboardEvent('keydown', { key: sufijo, cancelable: true }));
    }

    it('mide el código, el hueco mayor, el total y el sufijo', () => {
        const muestras: CalibrationSample[] = [];
        const cal = createCalibrator(m => muestras.push(m));

        teclear(cal, '750123456789', [4, 4, 9, 4, 4, 4, 4, 4, 4, 4, 4], 'Enter');

        expect(muestras).toHaveLength(1);
        const m = muestras[0];
        expect(m.code).toBe('750123456789');
        expect(m.maxGapMs, 'el hueco mayor es el que manda para la configuración').toBe(9);
        expect(m.totalMs).toBe(4 * 10 + 9);
        expect(m.detectedSuffix).toBe('enter');
    });

    it('reconoce el sufijo de tabulador', () => {
        const muestras: CalibrationSample[] = [];
        const cal = createCalibrator(m => muestras.push(m));
        teclear(cal, 'ABCDE', [5, 5, 5, 5], 'Tab');
        expect(muestras[0].detectedSuffix).toBe('tab');
    });

    it('un lector sin sufijo se cierra por silencio', () => {
        vi.useFakeTimers();
        try {
            const muestras: CalibrationSample[] = [];
            const cal = createCalibrator(m => muestras.push(m));
            teclear(cal, 'ABCDE', [5, 5, 5, 5], null);

            expect(muestras, 'todavía no: puede seguir teclando').toHaveLength(0);
            vi.advanceTimersByTime(400);

            expect(muestras).toHaveLength(1);
            expect(muestras[0].detectedSuffix).toBe('none');
            expect(muestras[0].code).toBe('ABCDE');
        } finally {
            vi.useRealTimers();
        }
    });

    it('el sufijo no se escapa al formulario', () => {
        // Sin `preventDefault`, el Enter del lector envía el formulario de Ajustes a
        // media calibración y la muestra se pierde.
        const cal = createCalibrator(() => {});
        const enter = new KeyboardEvent('keydown', { key: 'Enter', cancelable: true });
        cal(enter);
        expect(enter.defaultPrevented).toBe(true);

        const letra = new KeyboardEvent('keydown', { key: '7', cancelable: true });
        cal(letra);
        expect(letra.defaultPrevented).toBe(true);
    });

    it('lo que se mide alcanza para configurar un lector que sirva', () => {
        // El cierre del círculo: de la muestra sale una configuración, y con esa
        // configuración el lector de verdad tiene que reconocerse como lectura.
        const muestras: CalibrationSample[] = [];
        const cal = createCalibrator(m => muestras.push(m));
        teclear(cal, '750123456789', Array(11).fill(6), 'Enter');

        const cfg = configFromSample(muestras[0]);

        expect(cfg.suffix).toBe('enter');
        expect(cfg.maxGapMs, 'con margen sobre el hueco medido').toBeGreaterThan(6);
        expect(cfg.minLength).toBeLessThanOrEqual('750123456789'.length);
        // Y el margen no puede llegar a confundir a una persona teclando.
        expect(cfg.maxGapMs).toBeLessThan(80);
        expect(MS_POR_CARACTER_HUMANO).toBeLessThan(80);
    });

    it('una ráfaga vacía no produce una muestra inventada', () => {
        const muestras: CalibrationSample[] = [];
        const cal = createCalibrator(m => muestras.push(m));
        cal(new KeyboardEvent('keydown', { key: 'Enter', cancelable: true }));
        expect(muestras).toHaveLength(0);
    });
});
