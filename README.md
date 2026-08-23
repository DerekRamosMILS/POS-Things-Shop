# Things Shop POS

Punto de venta de escritorio para tienda de ropa. React + TypeScript en el frente,
Tauri 2 + Rust + SQLite en el backend. Funciona sin internet: toda la información
vive en el equipo donde corre la app.

## Documentación

- [Manual de operación](docs/MANUAL.md) — para el cajero y el administrador.
  Dentro de la app, **F1** abre lo mismo en pantalla.
- [Decisiones de arquitectura](docs/ARQUITECTURA.md) — una caja vs. varias,
  facturación, manejo del dinero y por qué de cada cosa.

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

**El instalador se construye en GitHub Actions, no en local.** El código de la
impresora y el cajón usa la API de Windows y no compila desde macOS ni Linux
(`ring`, dependencia del actualizador, necesita el SDK de Microsoft).

Para publicar una versión:

```bash
git tag v0.2.0 && git push --tags
```

Eso dispara `.github/workflows/release.yml`, que en un runner de Windows corre
las pruebas, construye el `.msi`, lo firma si hay certificado, genera el
manifiesto del actualizador y publica todo en GitHub Releases.

Cada push a `main` también compila y prueba en Windows (`ci.yml`), así que una
regresión en el código específico de esa plataforma se detecta enseguida.

Si tienes Windows a la mano, el equivalente local es:

```bash
pnpm tauri build
```

### Secretos del repositorio

| Secreto | Para qué | Sin él |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Firma del actualizador | El `.msi` sale sin `.sig` y el actualizador lo rechaza |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Contraseña de esa llave | Solo si la llave la tiene |
| `WINDOWS_CERT_BASE64` | Certificado de firma en base64 | Windows advierte "editor desconocido" al instalar |
| `WINDOWS_CERT_PASSWORD` | Contraseña del `.pfx` | — |

El contenido de `TAURI_SIGNING_PRIVATE_KEY` es el archivo
`~/.things-shop-updater.key`. La firma de código es opcional: sin ella el
instalador funciona, pero SmartScreen muestra una advertencia en cada
instalación. Un certificado estándar la reduce con el tiempo; uno EV la elimina
desde el primer día.

## Actualizaciones automáticas

Ya está todo conectado: `plugins.updater.endpoints` apunta al `latest.json` de
la última release de GitHub, y el flujo de publicación lo genera con la firma
correcta. Desde **Ajustes → Buscar actualizaciones** la tienda instala la
versión nueva y la app se reinicia sola.

La llave pública está en `src-tauri/tauri.conf.json`; **la privada vive fuera
del repositorio** en `~/.things-shop-updater.key` y no debe versionarse ni
perderse: sin ella no se pueden firmar versiones nuevas.

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

## Hardware de mostrador

Nada está atado a una marca concreta. Lo que cambia entre modelos se configura
desde **Ajustes → Impresora / Lector**, no en el código.

### Lector de código de barras

Los lectores se presentan al sistema como un teclado: "teclean" el código y
terminan con una tecla. No requieren driver ni importa la simbología (EAN-13,
UPC, Code128, QR, DataMatrix — todas entregan texto).

Conecta el lector, abre **Ajustes → Lector de código de barras**, haz clic en
"Prueba de lectura" y escanea cualquier producto: la app mide la velocidad real,
detecta si termina con Enter, Tab o nada, y calcula los umbrales. Guarda y listo.

La lectura se captura aunque el cursor esté dentro de un campo —en el mostrador
casi siempre lo está— y el campo se restaura para que el código no quede pegado
en el buscador.

### Impresora de tickets y cajón de dinero

El cajón de dinero **se conecta a la impresora**, no a la computadora: lleva un
cable tipo telefónico (RJ11/RJ12) al puerto DK de la impresora, y esta lo abre al
recibir una orden ESC/POS. Por eso ambos se configuran juntos.

La app manda los tickets en ESC/POS directo al spooler de Windows en modo RAW.
Eso significa que imprime sin abrir el diálogo del sistema, funciona con
cualquier impresora que tenga driver instalado, y puede accionar el cajón.

| Ajuste | Para qué |
|---|---|
| Impresora | Se elige de las instaladas en Windows |
| Ancho del papel | 58 mm (32 caracteres) u 80 mm (48) |
| Imprimir al cobrar | Ticket automático al cerrar la venta |
| Abrir cajón con efectivo | Solo se abre si entró efectivo |
| Comando del cajón | `1B 70 00 19 FA` (pin 2). Si no responde, prueba `1B 70 01 19 FA` (pin 5) |

Los botones **Imprimir prueba** y **Probar cajón** confirman que el hardware
responde antes de abrir la tienda. En Caja hay un botón **Abrir Cajón** para dar
cambio sin cobrar.

Si no se configura impresora, el sistema sigue funcionando igual e imprime por el
diálogo del sistema, como antes.

Los acentos se transliteran (`Niña` → `Nina`) porque la página de códigos por
defecto varía entre modelos: un ticket legible en cualquier impresora vale más
que uno con acentos en unas y basura en otras.

### Qué comprar

El combo estándar de retail es una **impresora térmica de 58 u 80 mm con puerto
DK** más un **cajón con conector RJ11/RJ12**. Cualquier marca compatible con
ESC/POS sirve; es lo más barato y lo mejor soportado.

## Conciliación del efectivo

El corte de caja calcula el efectivo esperado así:

```
fondo de apertura
+ ventas en efectivo
+ abonos de apartados en efectivo
− devoluciones en efectivo
− gastos
```

Cada uno de esos movimientos se registra contra el turno abierto en el momento en
que ocurre, y el desglose se muestra al cerrar la caja.

## Seguridad

- Contraseñas con Argon2 y salt por usuario
- Sesiones con token opaco y caducidad configurable (12 h por defecto)
- Cada comando del backend resuelve el usuario desde su sesión; el frontend
  nunca decide quién ejecuta una operación
- Reportes, usuarios, productos, proveedores y respaldos son solo de administrador
- Cada cobro lleva un identificador único: reenviar el mismo devuelve la venta
  original en vez de duplicarla
