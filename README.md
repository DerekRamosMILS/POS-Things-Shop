# Things Shop POS

Punto de venta de escritorio para tienda de ropa. React + TypeScript en el frente,
Tauri 2 + Rust + SQLite en el backend. Funciona sin internet: toda la información
vive en el equipo donde corre la app.

## Requisitos

- Node 20+ y **pnpm** (el proyecto no usa npm ni yarn)
- Rust estable + [dependencias de Tauri](https://tauri.app/start/prerequisites/)

## Desarrollo

```bash
pnpm install
pnpm tauri dev
```

`pnpm dev` levanta solo el frontend en `http://127.0.0.1:1420` con datos de
demostración en memoria — útil para trabajar en la interfaz, pero sin backend real.

## Verificación

```bash
pnpm build                        # typecheck + bundle del frontend
cd src-tauri && cargo test        # pruebas del backend
cd src-tauri && cargo clippy      # linter de Rust
```

## Compilar el instalador (Windows)

```bash
pnpm tauri build
```

Genera un `.msi` en `src-tauri/target/release/bundle/msi/`.

Para que el instalador quede firmado para el actualizador, exporta la llave antes
de compilar:

```bash
export TAURI_SIGNING_PRIVATE_KEY_PATH="$HOME/.things-shop-updater.key"
```

## Actualizaciones automáticas

La app usa `tauri-plugin-updater`. La llave pública ya está en
`src-tauri/tauri.conf.json`; **la privada vive fuera del repositorio** en
`~/.things-shop-updater.key` y no debe versionarse ni perderse: sin ella no se
pueden firmar versiones nuevas.

Falta un paso para activarlo: publicar el manifiesto y apuntar
`plugins.updater.endpoints` en `src-tauri/tauri.conf.json` a su URL. El manifiesto
es un JSON con esta forma:

```json
{
  "version": "0.2.0",
  "notes": "Qué cambió en esta versión",
  "pub_date": "2026-01-01T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<contenido del .msi.sig generado por tauri build>",
      "url": "https://tu-servidor/things-shop/ThingsShopPOS_0.2.0_x64.msi"
    }
  }
}
```

Mientras el endpoint no exista, «Buscar actualizaciones» en Configuración
simplemente reporta que no hay nada nuevo.

## Dónde viven los datos

| | Ruta |
|---|---|
| Windows | `%APPDATA%\things-shop\` |
| macOS | `~/Library/Application Support/things-shop/` |
| Linux | `$XDG_DATA_HOME/things-shop/` |

Dentro de esa carpeta: `things_shop.db` (base de datos), `backups/` (respaldos
rotados) y `things-shop.log` (bitácora, rotada a los 5 MB).

## Primer inicio

Se crea un usuario `admin` con la contraseña `admin1234`. La app **no deja pasar
de la primera pantalla hasta que se cambie** — no es una contraseña con la que se
pueda operar. Mínimo 8 caracteres; tras 5 intentos fallidos la cuenta se bloquea
5 minutos.

Los equipos que ya tenían el sistema instalado también quedan obligados a rotar
la contraseña de `admin` al actualizar.

## Seguridad

- Contraseñas con Argon2 y salt por usuario
- Sesiones con token opaco y caducidad configurable (12 h por defecto)
- Cada comando del backend resuelve el usuario desde su sesión; el frontend
  nunca decide quién ejecuta una operación
- Reportes, usuarios, productos, proveedores y respaldos son solo de administrador
