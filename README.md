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

## Publicar una versión

**El instalador se construye en GitHub Actions, no en local.** El código de la
impresora y el cajón usa la API de Windows y no compila desde macOS ni Linux
(`ring`, dependencia del actualizador, necesita el SDK de Microsoft).

Desde la Mac, todo el proceso es un comando:

```bash
pnpm publicar 0.2.0        # o: patch / minor / major
```

Eso sube la versión en los tres archivos que la llevan, commitea, etiqueta y
empuja. `release.yml` corre las pruebas en un runner de Windows, construye el
instalador, lo firma y publica el release con su `latest.json`. Unos diez
minutos. La tienda lo recibe sola (ver la sección siguiente).

**No etiquetes a mano.** La versión vive en `package.json`, `tauri.conf.json` y
`Cargo.toml`, y el actualizador compara la que trae horneada el binario contra la
que anuncia el manifiesto. Si la etiqueta va por delante de los archivos, se
publica una versión que la app cree más nueva que sí misma: instala, arranca, se
cree vieja y se reinstala **en un bucle infinito**. El script los mueve juntos y
el CI se niega a publicar si no coinciden, pero un `git tag` a secas se salta las
dos protecciones.

Cada push a `main` también compila y prueba en Windows (`ci.yml`), así que una
regresión en el código específico de esa plataforma se detecta enseguida.

### Secretos del repositorio

| Secreto | Para qué | Sin él |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Firma del actualizador | El instalador sale sin `.sig` y la app rechaza la versión |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Contraseña de esa llave | El build falla al firmar |
| `WINDOWS_CERT_BASE64` | Certificado de firma de código, en base64 | Windows advierte "editor desconocido" al instalar |
| `WINDOWS_CERT_PASSWORD` | Contraseña del `.pfx` | — |

Los dos primeros ya están cargados. **La llave privada del actualizador es
irreemplazable**: la pública queda horneada dentro de cada instalador, así que si
se pierde la privada, las computadoras que ya tengan la app instalada no aceptan
ninguna actualización más y hay que reinstalar a mano en cada una. Guárdala en un
gestor de contraseñas y nunca en el repositorio, que es público.

Las dos firmas son cosas distintas y es fácil confundirlas: la del actualizador
(minisign) prueba que la versión salió de aquí y es gratis; la de código
(certificado de Windows) es lo que evita la advertencia de SmartScreen y cuesta.
Sin la segunda el sistema funciona, pero la primera instalación muestra "Windows
protegió su PC" y hay que entrar en *Más información → Ejecutar de todas formas*.

## Actualizaciones automáticas

La tienda está lejos y de allá nadie va a instalar nada: publicas desde la Mac y
la aplicación se actualiza sola.

Lo delicado no es bajar el instalador, es **cuándo** dejarlo correr: en Windows el
instalador cierra la aplicación para poder reemplazarla, y reiniciar a media venta
es lo único que no puede pasar. Por eso hay dos ventanas:

| Cuándo | Qué hace |
|---|---|
| Los primeros 3 minutos desde que abre | Instala sin preguntar, salvo que haya un ticket a medias. Recién arrancada nadie está a media operación, y es la ventana que atrapa casi todas las versiones porque la caja se abre todos los días. |
| Cada 4 horas | Descarga en silencio y espera. Instala solo si el carrito está vacío **y** la caja está cerrada; mientras tanto muestra un aviso en la barra. |

**No es instantáneo, y no está fallando.** El instalador pesa casi cuatro megas:
entre que se encuentra la versión y que se instala pasan minutos, más en el
internet de una tienda. La barra lateral muestra el avance de la descarga para
que se note que está trabajando.

La ventana de arranque tiene que ser por tiempo y no por "que no haya nadie
dentro": la sesión y el turno sobreviven a cerrar la aplicación, así que al
reabrirla el usuario ya está dentro y la caja sigue abierta. Pedir que no
hubiera turno abierto era pedir algo que en una tienda no pasa nunca.

**Ajustes → Actualizar ahora**, junto a la versión instalada, la instala en el
momento saltándose esa espera. Existe porque la automática puede quedarse
esperando por un motivo que no previmos, y entonces hace falta una salida que no
dependa de que hayamos acertado.

El instalador es **NSIS por usuario** (`installMode: currentUser`), y eso no es un
detalle: el `.msi` de WiX se instala por máquina y pide permiso de Administrador
en cada actualización — un UAC en la cara de la cajera, que además no puede
aceptarlo si su cuenta de Windows no es administradora. NSIS por usuario instala
en `%LOCALAPPDATA%` sin pedir nada, lo que además encaja con que la base de datos
ya vive en `%APPDATA%`: la app ya era por usuario en sus datos.

La contrapartida: si en esa computadora se usaran **varias cuentas de Windows**,
cada una tendría su instalación y su propia base de datos. Con una sola cuenta
—el caso de una tienda— no aplica.

### Cómo saber a distancia si una versión llegó

La app anota en la bitácora cuándo cambió de versión, comparando el binario
contra la última versión que vio. Se detecta **después** de que la actualización
ocurrió, que es lo único fiable: durante la instalación la aplicación muere, así
que lo que se escribiera antes podría no guardarse.

Ese renglón sale en **Ajustes → Reporte de diagnóstico**, el archivo que el
encargado de la tienda puede mandar sin entender nada de lo que contiene.

### Dos cosas de las que depende

- **El repositorio tiene que seguir público.** Es lo que permite que el
  actualizador lea el `latest.json` sin credenciales. Si se pone privado, el
  endpoint devuelve 404 y las actualizaciones dejan de llegar **en silencio**.
- **El actualizador solo avanza.** No sabe bajar de versión. Para deshacer un
  cambio no se republica la anterior: se publica una **más alta** con la
  corrección (`pnpm publicar patch`).

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
