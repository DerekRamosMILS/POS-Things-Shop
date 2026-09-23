import { defineConfig } from 'vitest/config';

export default defineConfig({
    test: {
        environment: 'jsdom',
        environmentOptions: { jsdom: { url: 'http://localhost/' } },
        // El entorno no trae `localStorage`, así que todo lo que se persiste
        // —carrito, sesión, órdenes en espera, tamaño de letra— se venía probando
        // contra el respaldo en memoria en vez del almacenamiento de verdad.
        setupFiles: ['src/__tests__/entorno.ts'],
        include: ['src/**/*.test.ts'],
    },
});
