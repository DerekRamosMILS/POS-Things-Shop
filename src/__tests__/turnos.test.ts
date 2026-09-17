import { describe, expect, it } from 'vitest';
import { crearTurnos } from '../utils/turnos';

describe('el último pedido gana', () => {
    it('una respuesta que llega tarde ya no vale', async () => {
        const turnos = crearTurnos();
        const pantalla: string[] = [];
        const buscar = async (texto: string, tarda: number) => {
            const turno = turnos.siguiente();
            await new Promise(r => setTimeout(r, tarda));
            if (turnos.vigente(turno)) pantalla.push(texto);
        };

        await Promise.all([buscar('ves', 30), buscar('vestido', 5)]);

        expect(pantalla).toEqual(['vestido']);
    });
});
