/**
 * Lo que se le dice a la tienda después de una prueba de impresión.
 *
 * La prueba usa la impresora **elegida en pantalla**, que es lo correcto: probar
 * antes de guardar es justo para lo que sirve. Pero el ticket de verdad sale por
 * la que está guardada en la base. Sin avisar, alguien elige una impresora,
 * prueba, sale el ticket, cierra Ajustes convencido — y los tickets reales siguen
 * saliendo por la de antes, o por ninguna. La calibración del lector sí lo decía;
 * esto no.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { mensajeDePrueba } from '../utils/impresora';

describe('el aviso después de probar la impresora', () => {
    it('recuerda guardar cuando la elegida no es la guardada', () => {
        const m = mensajeDePrueba({ conCajon: false, sinGuardar: true });
        expect(m).toMatch(/guarda/i);
    });

    it('lo dice también cuando se probó el cajón', () => {
        const m = mensajeDePrueba({ conCajon: true, sinGuardar: true });
        expect(m).toMatch(/cajón/i);
        expect(m).toMatch(/guarda/i);
    });

    it('sin cambios pendientes no molesta con el recordatorio', () => {
        expect(mensajeDePrueba({ conCajon: false, sinGuardar: false })).toBe('Ticket de prueba enviado');
        expect(mensajeDePrueba({ conCajon: true, sinGuardar: false })).toBe('Ticket enviado y cajón accionado');
    });

    it('la pantalla de hardware usa el aviso y sabe qué está sin guardar', () => {
        const pantalla = readFileSync('src/components/HardwareSettings.tsx', 'utf-8');
        expect(pantalla).toMatch(/mensajeDePrueba/);
        expect(pantalla, 'necesita saber si la selección está sin guardar').toMatch(/sinGuardar/);

        const ajustes = readFileSync('src/pages/SettingsPage.tsx', 'utf-8');
        expect(ajustes, 'y Ajustes tiene que pasárselo').toMatch(/sinGuardar=/);
    });
});
