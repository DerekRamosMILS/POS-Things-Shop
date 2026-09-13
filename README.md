# Things Shop POS

Punto de venta de escritorio para tienda de ropa. React + TypeScript en el frente,
Tauri 2 + Rust + SQLite en el backend. Toda la información vive en el equipo donde
corre la app: vender, cobrar, imprimir y cortar caja no dependen de internet.

Dos cosas sí salen a la red, y solo salen —nadie entra a esta computadora—:
las [actualizaciones](#actualizaciones-automáticas), que se bajan de GitHub, y la
[captura desde el celular](#captura-desde-el-celular), que pasa por un buzón en
Cloudflare. Si se cae el internet, las dos esperan y lo demás sigue igual.

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
pnpm test                         # pruebas del frontend
pnpm build                        # typecheck + bundle del frontend
cd src-tauri && cargo test        # pruebas del backend
cd src-tauri && cargo clippy      # linter de Rust
cd relevo && pnpm test            # pruebas del buzón (corren en el runtime de Workers)
```

`pnpm build` se niega a construir si las versiones de Tauri en Rust y en npm se
separaron en mayor o menor. No es celo: `tauri build` sí lo valida, falla al final
de todo y solo en el runner de Windows, donde ya es tarde.

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

## Lo que vive fuera de la tienda

La computadora de la tienda está a 2 000 km y nadie de allá va a instalar ni
configurar nada. Eso obliga a que haya piezas fuera de ella, y conviene tener
claras cuáles son, porque son las únicas que pueden fallar sin que se vea.

```
   Tu Mac                    Internet                        La tienda
   ──────                    ────────                        ─────────

   pnpm publicar ──► GitHub Actions ──► Release + latest.json
                                                  │
                                                  │ la app pregunta
                                                  ▼
                                          Things Shop POS  ◄── vende sin internet
                                             (Windows)
                                                  ▲
                                                  │ la app recoge
                                                  │
   Celular ──────────► Worker + KV (Cloudflare) ──┘
           deja lo         el buzón
           capturado
```

Lo importante del dibujo: **las dos flechas que llegan a la tienda salen de la
tienda**. La aplicación pregunta, baja y recoge; nadie abre una conexión hacia
esa computadora. No hay puerto abierto, no hay IP fija que conseguir, no hay nada
que configurar en el router de la tienda — y no lo hay porque no se puede: ese
router aísla a sus clientes, y es justo lo que tumbó el primer diseño.

| Pieza | Dónde | Para qué | Si se cae |
|---|---|---|---|
| Release + `latest.json` | GitHub, este repo (público) | Que la app encuentre y verifique la versión nueva | No llegan actualizaciones. Vender sigue igual |
| Worker + KV | Cloudflare, cuenta de Derek | Buzón entre el celular y la tienda | No entran capturas nuevas. Vender sigue igual |
| Base de datos | La computadora de la tienda | Todo lo que importa | Se para la tienda. Por eso vive ahí y no aquí |

Ninguna de las dos piezas de fuera guarda nada del negocio: ni ventas, ni
clientes, ni caja, ni existencias. Si mañana se borraran las dos, la tienda
seguiría operando y lo único que se perdería es la comodidad de actualizar a
distancia y de capturar con el teléfono.

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

## Captura desde el celular

Se toman las fotos con el teléfono y el producto se da de alta solo. El teléfono
funciona **sin conexión**: lo capturado se queda guardado en él y se manda en
cuanto haya señal, aunque sea desde fuera de la tienda y con datos móviles.

El teléfono y la computadora **nunca se hablan directo**. El teléfono deja lo
capturado en un buzón en internet, y el punto de venta pasa a recogerlo cada 3
minutos mientras esté abierto. Ese buzón es el *relevo*, y su código vive en
[`relevo/`](relevo/).

### Por qué hay un servidor de por medio

El primer diseño no tenía ninguno: esta computadora levantaba un servidor en el
WiFi de la tienda y el teléfono entraba a él. Funcionó en la mesa de pruebas y
**nunca funcionó en la tienda**. El router de allá aísla a sus clientes entre sí
—no se ven ni estando en la misma red—, y eso no lo arregla ningún permiso de
firewall: lo comprobamos con la regla puesta y el servidor escuchando.

La asimetría que sí sirve es esta: de la tienda **se sale** a internet sin
problema (por ahí se actualiza la app sola), pero **no se entra**. Un buzón al
que los dos lados llegan por su cuenta no necesita que nadie entre a ningún lado.

### Qué es, en Cloudflare

Dos cosas, ambas en el plan gratuito de la cuenta de Derek:

| | |
|---|---|
| **Worker** `things-shop-relevo` | El código de `relevo/src/index.ts`. Atiende la API y **también sirve la página de captura** que abre el teléfono |
| **KV** (binding `CAPTURAS`) | Donde se quedan las capturas mientras nadie las recoge. El identificador del namespace está en `relevo/wrangler.jsonc` |

Vive en `https://things-shop-relevo.derek-papa.workers.dev`, y esa dirección va
horneada en la app (`URL_PREDETERMINADA`, en `src-tauri/src/capture/relevo.rs`).
Se puede apuntar a otro relevo sin recompilar, escribiendo la clave `relevo_url`
en la tabla `system_config`; no hay pantalla para eso a propósito, porque es algo
que solo se toca si se muda el buzón.

La página y la API salen del **mismo origen** (`run_worker_first: ["/api/*"]`:
lo que empieza por `/api/` lo atiende el Worker, el resto son archivos de
`relevo/public/`). Eso no es un detalle de estilo — significa que no hay CORS que
configurar ni un segundo despliegue que pueda quedarse atrás del primero.

### Quién puede entrar

La dirección es pública y no hace falta esconderla: sin el secreto de la tienda
no se ve absolutamente nada.

El punto de venta genera **32 bytes al azar** y la carpeta de esa tienda es el
`SHA-256` de ese secreto. No hay registro de tiendas ni lista de secretos
válidos: quien tiene el secreto llega a su carpeta, quien no, no ve ninguna —ni
sabe si existe—. El Worker rechaza de entrada cualquier secreto de menos de 40
caracteres, porque esto está expuesto a internet y un secreto corto sería una
contraseña adivinable.

El secreto viaja al teléfono dentro del QR, **después del `#`**, que es la única
parte de una URL que los navegadores no mandan al servidor: no aparece en los
registros de Cloudflare ni se filtra por el `Referer`.

Todas las rutas piden ese secreto (`Authorization: Bearer`) menos una:
`/api/salud`, que existe justamente para poder distinguir *"el relevo está
caído"* de *"mi secreto no sirve"*.

### Emparejar un teléfono

**Ajustes → Capturar productos desde el celular**, apuntar la cámara al QR, y
cuando el navegador lo ofrezca, **instalar la página en la pantalla de inicio** y
abrirla siempre desde ahí. Es una vez por teléfono y no hay que instalar ningún
certificado ni dar ningún permiso raro.

Dos botones en esa misma pantalla:

- **Traer ahora** — no espera los 3 minutos. Es la salida de emergencia de la
  captura, igual que *Actualizar ahora* lo es de las actualizaciones.
- **Generar código nuevo** — cambia el secreto. Los teléfonos ya emparejados
  dejan de poder mandar hasta que vuelvan a escanear; antes de cambiarlo se
  recoge lo que ya habían mandado.

Arriba de los botones hay un renglón que dice cuándo se revisó por última vez, o
el error si no se pudo. Es lo primero que hay que leer cuando algo no aparece.

### Qué pasa por ahí y qué no

| | |
|---|---|
| Qué pasa | Lo capturado: nombre, precio, tallas, colores, piezas y fotos. Y el catálogo para poder contar: nombres, códigos y tallas, **sin existencias** |
| Qué no pasa nunca | Ventas, clientes, caja, inventario. El punto de venta solo baja; no expone nada |
| Cuánto se queda | Lo normal es segundos: se recoge, se confirma y se borra. Lo que nadie recoja caduca solo a los 30 días (el catálogo, a los 60) |
| Cuánto cabe | 8 MB por captura, 5 MB el catálogo |

El borrado va **después** de que la captura quedó guardada en la tienda, nunca
antes: entre el relevo y la base de datos de la tienda, la copia que importa es
la de la tienda. Si se corta el internet a media recogida, la captura sigue en el
buzón y entra en la vuelta siguiente.

Lo que llega y no se puede dar de alta —un conteo de una prenda que se borró, un
producto sin nombre— sale del buzón para no tapar lo demás, pero **no se pierde**:
se guarda completo, fotos incluidas, en la tabla `capturas_rechazadas`, y se
enseña en esa misma pantalla con el motivo.

### De dónde salen los 3 minutos

Del cupo gratuito de KV, no del gusto: 1 000 escrituras y 1 000 listados al día.
Cada vuelta hace un listado, así que cada minuto serían 1 440 —se acabaría el
cupo antes de cerrar la tienda— y cada tres son 480, que deja margen para *Traer
ahora* y para el día que la app se quede abierta de corrido. Por lo mismo el
catálogo se publica **solo cuando cambia** y no en cada vuelta.

Los pendientes se leen en páginas de 200, hasta 5 páginas por vuelta; lo que no
alcance entra en la siguiente. El listado de KV es *eventualmente consistente* y
puede enseñar algo que ya se borró, lo cual no estorba porque la tienda reconoce
por `captura_id` lo que ya tiene.

### Cuando no llega algo del celular

En este orden, que va de lo más probable a lo menos:

1. **El renglón de estado** en esa pantalla. Si dice un error, ahí está la
   respuesta. Si dice que revisó hace rato, la app estuvo cerrada.
2. **Traer ahora.** Descarta que sea nada más la espera de los 3 minutos.
3. **¿Capturas rechazadas?** Si el aviso naranja está ahí, sí llegó — lo que
   falló fue darla de alta, y el motivo lo dice cada renglón.
4. **¿Vive el relevo?** Abrir `https://things-shop-relevo.derek-papa.workers.dev/api/salud`
   en cualquier navegador. Si contesta `ok`, el buzón está bien y el problema es
   de la tienda o del secreto.
5. **`cd relevo && pnpm tail`** desde la Mac: los registros del Worker en vivo,
   mientras el teléfono intenta mandar.

### Desplegar el relevo

Va aparte de la app y no se publica con `pnpm publicar`:

```bash
cd relevo
pnpm test        # contra el runtime real de Workers, con KV de verdad
pnpm deploy
```

Solo hace falta que `wrangler` esté conectado a la cuenta (`wrangler login`); no
hay secretos que cargar, porque el Worker no guarda ninguno — la autenticación es
el secreto que trae cada petición.

Cambiar el relevo **no obliga a actualizar la app ni a reemparejar teléfonos**,
mientras la dirección y las rutas no cambien. Los detalles de dentro están en
[`relevo/README.md`](relevo/README.md).

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
