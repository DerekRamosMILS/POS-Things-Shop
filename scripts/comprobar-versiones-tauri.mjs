#!/usr/bin/env node
/**
 * Comprueba que los crates de Rust y los paquetes de npm de Tauri vayan al par.
 *
 * `tauri build` se niega a construir si un par no coincide en major/minor, y con
 * razón: son las dos mitades de un mismo puente y una API que existe en una y no
 * en la otra falla en tiempo de ejecución, ya en la tienda.
 *
 * El problema es *cuándo* se enteraba uno. `cargo check` y `cargo test` no miran
 * esto, así que un desajuste vivía en el repo sin molestar hasta el momento de
 * publicar —que es justo cuando menos ganas hay de descubrirlo—. Los dos
 * archivos de bloqueo envejecen por separado y se separan solos.
 *
 * Si esto falla:  cd src-tauri && cargo update    y luego  pnpm up "@tauri-apps/*@latest"
 */
import { readFileSync } from 'node:fs';

/** Paquete de npm → crate de Rust que tiene que ir con él. */
const PARES = {
    '@tauri-apps/api': 'tauri',
    '@tauri-apps/plugin-dialog': 'tauri-plugin-dialog',
    '@tauri-apps/plugin-opener': 'tauri-plugin-opener',
    '@tauri-apps/plugin-process': 'tauri-plugin-process',
    '@tauri-apps/plugin-updater': 'tauri-plugin-updater',
};

/** Versiones de los crates, leídas del Cargo.lock. */
function versionesDeRust() {
    const lock = readFileSync('src-tauri/Cargo.lock', 'utf8');
    const versiones = new Map();
    // Los bloques son [[package]] con name y version en cualquier orden.
    for (const bloque of lock.split('[[package]]')) {
        const nombre = bloque.match(/^\s*name = "(.+)"$/m)?.[1];
        const version = bloque.match(/^\s*version = "(.+)"$/m)?.[1];
        if (nombre && version) versiones.set(nombre, version);
    }
    return versiones;
}

/** Versión instalada de un paquete de npm, o null si no está. */
function versionDeNpm(paquete) {
    try {
        return JSON.parse(readFileSync(`node_modules/${paquete}/package.json`, 'utf8')).version;
    } catch {
        return null;
    }
}

const menor = (v) => v.split('.').slice(0, 2).join('.');

const rust = versionesDeRust();
const problemas = [];

for (const [paquete, crate] of Object.entries(PARES)) {
    const vNpm = versionDeNpm(paquete);
    const vRust = rust.get(crate);

    // Un paquete que no se usa no es un problema; que falte el crate, sí.
    if (!vNpm) continue;
    if (!vRust) {
        problemas.push(`${paquete} ${vNpm} está instalado pero el crate ${crate} no aparece en Cargo.lock`);
        continue;
    }
    if (menor(vNpm) !== menor(vRust)) {
        problemas.push(`${crate} 🦀 ${vRust}  ≠  ${paquete} ⱼₛ ${vNpm}`);
    }
}

if (problemas.length > 0) {
    console.error('\n  ✕ Versiones de Tauri desalineadas entre Rust y npm:\n');
    for (const p of problemas) console.error(`      ${p}`);
    console.error(`
  'tauri build' se niega a construir así, y sin este aviso el desajuste se
  descubre al publicar. Para alinearlas:

      cd src-tauri && cargo update
      pnpm up "@tauri-apps/api@latest" "@tauri-apps/cli@latest" \\
              "@tauri-apps/plugin-dialog@latest" "@tauri-apps/plugin-opener@latest" \\
              "@tauri-apps/plugin-process@latest" "@tauri-apps/plugin-updater@latest"
`);
    process.exit(1);
}

console.log(`  ✓ ${Object.keys(PARES).length} pares de Tauri alineados entre Rust y npm`);
