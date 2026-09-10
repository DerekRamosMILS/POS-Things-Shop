#!/usr/bin/env node
/**
 * Publica una versión nueva: sube el número, commitea, etiqueta y empuja.
 *
 * La versión vive en tres archivos y el actualizador depende de que los tres
 * digan lo mismo. Subirlos a mano es exactamente la clase de tarea que se olvida
 * a la mitad, y equivocarse aquí no da un error: da una tienda que se reinstala
 * en un bucle infinito, porque la app compara la versión que trae horneada
 * contra la que anuncia el manifiesto. De ahí que esto sea un script y no una
 * lista de pasos en el README.
 *
 *   pnpm publicar 0.2.0
 *   pnpm publicar patch     # 0.1.0 -> 0.1.1
 *   pnpm publicar minor     # 0.1.0 -> 0.2.0
 */
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';

const ARCHIVOS = {
    paquete: 'package.json',
    tauri: 'src-tauri/tauri.conf.json',
    cargo: 'src-tauri/Cargo.toml',
};

const sh = (cmd, args, opts = {}) =>
    execFileSync(cmd, args, { encoding: 'utf8', ...opts }).trim();

function morir(mensaje) {
    console.error(`\n  ✕ ${mensaje}\n`);
    process.exit(1);
}

function versionActual() {
    return JSON.parse(readFileSync(ARCHIVOS.tauri, 'utf8')).version;
}

/** Traduce "patch"/"minor"/"major" o valida un número explícito. */
function resolverVersion(pedida, actual) {
    const salto = { major: 0, minor: 1, patch: 2 }[pedida];
    if (salto !== undefined) {
        const partes = actual.split('.').map(Number);
        if (partes.length !== 3 || partes.some(Number.isNaN)) {
            morir(`La versión actual "${actual}" no es x.y.z; pásame el número completo.`);
        }
        partes[salto] += 1;
        for (let i = salto + 1; i < 3; i++) partes[i] = 0;
        return partes.join('.');
    }
    if (!/^\d+\.\d+\.\d+$/.test(pedida)) {
        morir(`"${pedida}" no es una versión válida. Usa x.y.z, o patch/minor/major.`);
    }
    return pedida;
}

/** Compara x.y.z numéricamente. Devuelve true si `a` es mayor que `b`. */
function esMayor(a, b) {
    const [x, y] = [a.split('.').map(Number), b.split('.').map(Number)];
    for (let i = 0; i < 3; i++) {
        if (x[i] !== y[i]) return x[i] > y[i];
    }
    return false;
}

function escribirVersion(nueva) {
    for (const ruta of [ARCHIVOS.paquete, ARCHIVOS.tauri]) {
        const json = JSON.parse(readFileSync(ruta, 'utf8'));
        json.version = nueva;
        writeFileSync(ruta, JSON.stringify(json, null, 2) + '\n');
    }

    // En Cargo.toml solo la versión del paquete, no la de una dependencia: por
    // eso se ancla al primer `version =` que sigue a `[package]`.
    const cargo = readFileSync(ARCHIVOS.cargo, 'utf8');
    let visto = false;
    const actualizado = cargo.replace(/^version = ".*"$/gm, (linea) => {
        if (visto) return linea;
        visto = true;
        return `version = "${nueva}"`;
    });
    if (!visto) morir('No encontré la versión en Cargo.toml.');
    writeFileSync(ARCHIVOS.cargo, actualizado);
}

// ── Comprobaciones antes de tocar nada ───────────────────────────────────────

const pedida = process.argv[2];
if (!pedida) {
    console.error(`
  Uso:  pnpm publicar <versión|patch|minor|major>

  Versión actual: ${versionActual()}

  Sube el número en los tres archivos, commitea, etiqueta y empuja. El CI
  construye el instalador de Windows y publica el release que la tienda
  descarga sola.
`);
    process.exit(1);
}

const rama = sh('git', ['rev-parse', '--abbrev-ref', 'HEAD']);
if (rama !== 'main') {
    morir(`Estás en "${rama}". Las versiones se publican desde main.`);
}

if (sh('git', ['status', '--porcelain'])) {
    morir('Hay cambios sin commitear. Commitea o guarda antes de publicar.');
}

sh('git', ['fetch', 'origin', 'main', '--tags']);
if (sh('git', ['rev-parse', 'HEAD']) !== sh('git', ['rev-parse', 'origin/main'])) {
    morir('main no está sincronizado con origin. Haz pull (o push) antes de publicar.');
}

const actual = versionActual();
const nueva = resolverVersion(pedida, actual);

// El actualizador solo va hacia adelante: nunca "baja" a una versión anterior.
// Publicar un número menor o igual deja a la tienda sin recibir nada, callado.
if (!esMayor(nueva, actual)) {
    morir(`${nueva} no es mayor que la versión actual ${actual}. ` +
          `El actualizador solo avanza: para deshacer un cambio, publica una versión más alta con la corrección.`);
}

const etiquetas = sh('git', ['tag', '--list']).split('\n');
if (etiquetas.includes(`v${nueva}`)) {
    morir(`La etiqueta v${nueva} ya existe.`);
}

// ── Adelante ─────────────────────────────────────────────────────────────────

console.log(`\n  Publicando ${actual} → ${nueva}\n`);

escribirVersion(nueva);

// Cargo.lock lleva la versión del paquete adentro; sin esto el build en el CI
// falla con el lockfile desactualizado.
console.log('  · Actualizando Cargo.lock');
sh('cargo', ['update', '--package', 'things-shop', '--precise', nueva], {
    cwd: 'src-tauri',
    stdio: ['ignore', 'ignore', 'inherit'],
});

console.log('  · Commit y etiqueta');
sh('git', ['add', ARCHIVOS.paquete, ARCHIVOS.tauri, ARCHIVOS.cargo, 'src-tauri/Cargo.lock']);
sh('git', ['commit', '-m', `release: v${nueva}`]);
sh('git', ['tag', '-a', `v${nueva}`, '-m', `Things Shop POS v${nueva}`]);

console.log('  · Empujando a origin');
sh('git', ['push', 'origin', 'main']);
sh('git', ['push', 'origin', `v${nueva}`]);

const repo = sh('git', ['remote', 'get-url', 'origin']).replace(/\.git$/, '');
console.log(`
  ✓ v${nueva} en camino.

    El CI está construyendo el instalador (~10 min). Cuando termine, la tienda
    lo recibe sola: al abrir la app, o esa misma tarde si dejan la caja cerrada.

    Seguimiento:  ${repo}/actions
    Release:      ${repo}/releases/tag/v${nueva}
`);
