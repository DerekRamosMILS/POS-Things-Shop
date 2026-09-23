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

Las pruebas del frontend corren con un `localStorage` puesto a mano
(`src/__tests__/entorno.ts`). El entorno de jsdom no trae uno, y sin eso el
almacén que usan el carrito, la sesión y las órdenes en espera caía a su respaldo
en memoria: **las pruebas de persistencia pasaban en verde sin ejercitar el
almacenamiento**, y lo que lo usa directo —el tamaño de letra— no tenía forma de
probarse.

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

### Nunca a media venta

`motivoDeEspera()` decide, y mira tres cosas: el ticket en curso, las **órdenes en
espera** y el turno abierto. Las órdenes en espera cuentan porque apartar una con
F8 vacía el carrito —los renglones se mueven a otro cajón—, así que mirando solo
el carrito una orden apartada parecía una caja en calma. El motivo se puede leer
en Ajustes: una actualización que espera sin decir por qué es indistinguible de
una que no llegó, y eso a distancia no se depura.

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
- **Generar código nuevo** — cambia el secreto. Primero le avisa al relevo que
  el viejo queda retirado, y **sin internet no lo cambia**: cambiarlo solo en la
  computadora dejaba a los teléfonos subiendo a una carpeta que ya nadie miraba,
  con el relevo contestándoles que sí había llegado. Un teléfono con el código
  viejo recibe "este código ya no sirve", **conserva lo que capturó**, y lo manda
  en cuanto vuelve a escanear el QR. Lo que alcanzó a subir con el viejo se
  sigue recogiendo durante media hora.

El secreto no sale por la configuración general de la app: solo lo ve un
administrador, en esta pantalla.

Arriba de los botones hay un renglón que dice cuándo se revisó por última vez, o
el error si no se pudo. Es lo primero que hay que leer cuando algo no aparece.

### Qué pasa por ahí y qué no

| | |
|---|---|
| Qué pasa | Lo capturado: nombre, precio, tallas, colores, piezas y fotos. Y el catálogo para poder contar: nombres, códigos y tallas, **sin existencias** |
| Qué no pasa nunca | Ventas, clientes, caja, inventario. El punto de venta solo baja; no expone nada |
| Cuánto se queda | Lo normal es segundos: se recoge, se confirma y se borra. Lo que nadie recoja caduca solo a los 30 días (el catálogo, a los 60) |
| Cuánto cabe | 8 MB por captura, 5 MB el catálogo, 5 000 prendas |

El catálogo lleva las primeras 5 000 prendas activas por orden de nombre. Una
prenda que no está en él no se puede contar desde el celular, y desde el teléfono
eso se ve igual que si no existiera, así que la pantalla de captura **avisa**
cuántas quedaron fuera si alguna vez pasa.

Un conteo que llega sin talla para una prenda que sí tiene tallas se rechaza: el
total de ese producto es la suma de sus tallas, y escribirlo directo rompe esa
cuenta sin que nada lo delate. Pasa cuando el catálogo del teléfono es más viejo
que las tallas; el mensaje pide volver a escanear el QR.

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
alcance entra en la siguiente. Ese camino tiene prueba con 205 capturas: que el
cursor lleve a la siguiente página, que la última diga que ya terminó, que entre las
dos estén todas sin repetir, y que un cursor de una tienda no abra la carpeta de
otra. Un teléfono que capturó días sin conexión sincroniza de golpe, y si el cursor
no viajara bien la tienda se quedaría con las primeras 200 sin un error que lo diga.
El tope de 200 por tanda de confirmación también está probado por los dos lados, para
que el relevo no acepte menos de lo que el punto de venta manda. El listado de KV es *eventualmente consistente* y
puede enseñar algo que ya se borró, lo cual no estorba porque la tienda reconoce
por `captura_id` lo que ya tiene.

La página de captura se guarda en el teléfono para abrir sin la computadora, y se
pide fresca en cada apertura. Si el relevo contesta **mal** —caído, un despliegue a
medias, la página de error de Cloudflare—, se usa la guardada: entregar esa
respuesta dejaba al teléfono mirando un error, y con él inalcanzable la cola de lo
ya capturado, que vive dentro de la página.

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

Todas las copias —el respaldo automático al cerrar turno, el manual y la que se
lleva a la USB— se hacen con `VACUUM INTO` y no copiando el archivo. Copiar el
`.db` se lleva solo lo que ya bajó a él y deja fuera lo que sigue en el `-wal`,
que es justo lo último que se vendió; y un `wal_checkpoint` antes de copiar no
alcanza, porque cuando no puede lo avisa en un renglón en vez de en un error.
Cada copia se abre y se le corre `PRAGMA quick_check` antes de darla por buena:
un respaldo dañado tiene que doler el día que se hace, no el día que hace falta.

Cuesta más que copiar el archivo: medido sobre una base de 297 MB —dos mil fotos,
el tamaño que este mismo README da como referencia— son 1.3 segundos contra 0.4.
Pasa una vez al día, al cerrar el turno, con la caja cerrada y nadie formado, y a
cambio la copia es consistente y viene ya revisada.

El respaldo al cerrar tiene pruebas propias: que el ajuste se respete, que por
omisión respalde —quien no sabe del ajuste es quien más necesita el respaldo—, que
el archivo aparezca de verdad, y que un fallo del respaldo no impida cerrar la
caja. Esa rutina existe porque la tienda creía tener respaldos automáticos y no
tenía ninguno: el ajuste estaba en la pantalla y no lo leía nadie.

### Nada del negocio se borra

No depende de que cada pantalla lo haga bien: lo garantiza la base de datos, con
disparadores (migración `026_nada_se_borra`), pase el borrado por donde pase.

| | Qué pasa al intentar borrar |
|---|---|
| Ventas, productos, tallas, inventario, caja, apartados, devoluciones, conteos, clientes, proveedores, usuarios, historial de precios | La base se niega y la operación entera se cancela. Lo que en la app dice "eliminar" es desactivar o cancelar |
| Fotos y gastos | Se pueden quitar de la vista, pero antes la base copia la fila completa —la foto con sus bytes— a `product_images_archivo` / `expenses_archivo`, que tampoco se pueden borrar |
| Promociones y categorías | Solo si nunca se usaron o están vacías |
| Sesiones, bitácora, notificaciones, configuración | Se borran con normalidad: no son historia del negocio |

Cambiar la foto desde la ficha del producto solo toca la **principal**; las demás
—las que llegaron del celular, por ejemplo— se quedan.

**Restaurar un respaldo** reemplaza la base entera, y eso ningún disparador lo
detiene. Por eso, antes de reemplazarla, la app guarda una copia completa de la
base actual en `backups/antes-de-restaurar_<fecha>.db` —con `VACUUM INTO`, que
incluye lo que todavía estaba en el `-wal`— y si esa copia falla, no restaura.
Esa copia sale en la lista de respaldos y la rotación nunca la borra.

El respaldo elegido se revisa **antes** de prepararlo: tiene que abrirse como
base de datos, pasar `quick_check` y traer las migraciones de Things Shop. Un
archivo dañado se preparaba igual y la aplicación se cerraba con un aviso de
error en cada arranque, con la tienda parada y la única salida por línea de
comandos.

La restauración se aplica al arrancar, así que la app **se reinicia sola** en
cuanto queda preparada. Si por algo no se reinicia, una restauración preparada
hace más de 15 minutos ya no se aplica: se aparta como
`things_shop.db.restauracion-no-aplicada_<fecha>` y la base se queda como
estaba. Aplicarla días después dejaba fuera de la vista todo lo vendido
entretanto.

La prueba `toda_tabla_tiene_decidido_si_se_puede_borrar` falla si alguien agrega
una tabla sin decidir en cuál de estos grupos va.

### Qué se puede editar en Ajustes

Solo lo que tiene etiqueta en `src/pages/ajustesVisibles.ts`. La reja dibujaba toda
clave de `system_config` que no fuera de hardware, con el nombre técnico por
etiqueta cuando no tenía una, y por ahí se colaban cosas que no son ajustes sino
rastro interno: `version_instalada` —que se escribe en cada arranque y es el rastro
de si la actualización llegó—, `ultima_copia_externa` —de la que depende el aviso
de que hace mucho no sale una copia del equipo— y la huella del catálogo del
relevo. Editarlas a mano hace mentir a lo que se apoya en ellas.

Era una lista negra de claves escondidas, y a una lista negra siempre le falta
algo: cada clave nueva que Rust escriba aparece sola en la pantalla. Ahora es al
revés, y la prueba `ajustesVisibles` cuida las dos puntas: que ningún rastro
interno se ofrezca, y que ningún ajuste sembrado se quede sin etiqueta y desaparezca.

Esa prueba nació mirando solo la migración de arranque, y por ese hueco se cayeron
de la pantalla los datos fiscales del propio negocio —`rfc_emisor`,
`regimen_emisor`, `cp_emisor`, que siembra la 015—: la tienda se quedó sin forma de
capturar su RFC. Ahora recorre **todas** las migraciones, y lo que se siembra y aun
así no se edita a mano va en una lista con su motivo, con una segunda prueba que
falla si alguna entrada deja de corresponder a algo.

Esos tres ajustes existen y se pueden capturar, pero **nada los lee todavía**: la
exportación de ventas por facturar lleva los datos fiscales del cliente, no los del
emisor. Queda como está —quién necesita el RFC del emisor en el CSV lo decide quien
factura— y aquí anotado para que no parezca un olvido.

### Cuando la pantalla se cae

El `ErrorBoundary` evita que la cajera se quede mirando una ventana en blanco y
le ofrece volver al inicio. Además lo anota en `app_logs` con módulo `interfaz`,
así que sale en el reporte de diagnóstico: `console.error` solo se ve con las
herramientas del navegador abiertas y en la compilación de producción no va a
ninguna parte, así que la caja se recuperaba y nadie se enteraba nunca. A 2000 km
eso convierte "a veces se pone raro" en algo imposible de perseguir.

### Escribir en dos tablas es todo o nada

Es la clase de bug que más veces ha aparecido aquí: dos `execute` seguidos, el
segundo falla, y queda un gasto editado con el corte sin ajustar, un abono cobrado
con el saldo sin bajar, una prenda con existencia y sin el movimiento que la
explica. Nada de eso se ve hasta que alguien cuenta billetes o revisa un historial.

La prueba `transacciones` lee el Rust de producción —recortando los módulos de
prueba— y busca funciones que escriban en dos o más tablas sin abrir transacción.
Las excepciones se enumeran con su motivo, y una segunda prueba falla si alguna
excepción se queda sin corresponder a nada: una entrada muerta esconde el siguiente
caso. `app_logs` no cuenta, porque es rastro y puede fallar sin consecuencia.

### Invariantes

Tres cosas que tienen que ser verdad siempre, probadas sobre secuencias largas en
vez de sobre el caso que se le ocurrió a alguien:

| Invariante | Qué lo rompería |
|---|---|
| La existencia de una prenda es la suma de sus movimientos | Un camino que cambie el stock sin dejar renglón, o que anote un monto distinto del que movió |
| La existencia de una talla es la suma de sus movimientos, y el total del producto la suma de sus tallas | Quitar una talla y que su mercancía se esfume; contar una talla y mover otra |
| El efectivo esperado del turno es igual a lo que dicen las tablas de ventas, abonos, devoluciones y gastos | Un movimiento que entre al cajón sin sumarse a la columna del corte, o al revés |
| Una pierna del desglose que el reparto trate como "no entra al cajón" no puede caer en la columna de efectivo | Una forma de pago que el sistema no conozca |

Los dos del inventario se cruzan contra ocho caminos —venta, devolución,
cancelación, compra, ajuste, reserva y cancelación de apartado, conteo del celular
y edición de tallas—. El del cajón, contra una jornada de dieciséis movimientos con
precios que no dividen bien. Las tres están falsificadas: quitar el renglón que
cierra el reparto en tallas, el movimiento de una compra, la columna de un abono o
la de un gasto hace fallar la prueba que corresponde.

### Las formas de pago se revisan en la puerta

Solo efectivo, tarjeta y transferencia. `sales.payment_method` siempre tuvo su
`CHECK` en la base; las piernas del desglose —`sale_payments.method` y
`layaway_payments.payment_method`— no, y dos funciones clasificaban lo desconocido
**al revés**: el reparto del cobro trata todo lo que no sea `cash` como dinero que
no entra al cajón, y la columna del corte manda lo desconocido a la de efectivo. Una
pierna con un método raro —un vale, una mayúscula de más, una forma de pago nueva
que alguien agregue al frontend sin tocar el backend— subía el efectivo esperado sin
que hubiera entrado un peso, y el corte reportaba un faltante de ese tamaño. En los
abonos de apartado era peor: ahí no hay reparto que compense.

Los mapeos a columna del corte ya no llevan comodín: están enumeradas las tres, y
una cuarta revienta la prueba en vez de caer callada en la de efectivo. Quien agregue
una forma de pago tiene que decir dónde cae.

**Escribir y deshacer son distintos.** Cancelar una venta lee el método de la pierna
**de la base** para devolver su importe a la columna donde cayó, y esas filas se
escribieron cuando no había validación: con el mismo mapeo estricto, cancelar una
venta vieja con una pierna rara reventaba el punto de venta y esa venta no se podía
cancelar nunca. Deshacer espeja la historia —lo desconocido volvió a la columna de
efectivo, que es donde el comodín lo había puesto—, y escribir algo nuevo con un
método desconocido sigue siendo un error del programa.

Y una nota que vale más que el arreglo: la prueba que existía afirmaba
`register_field("desconocido") == "total_cash_sales"`. Tenía el bug escrito como
comportamiento correcto, así que ninguna corrida en verde iba a delatarlo. **Una
prueba también puede proteger un bug.**

### Lo que crece para siempre necesita índice

Desde la 026 el historial del negocio no se borra, así que esas tablas solo crecen.
Una consulta que las recorra sin índice se vuelve más lenta cada mes, en la
computadora más lenta del negocio, y nada avisa: se nota como "la aplicación está
pesada".

El caso que lo destapó fue de cosecha propia. La utilidad de las partidas viejas
—las de antes de que se guardara el costo al vender— se busca en `price_history` con
una subconsulta por partida, y esa tabla no tenía **ningún** índice: medido sobre
4000 cambios de costo y 3000 partidas legadas, la consulta pasa de **222 ms a 1 ms**,
y la pantalla de Reportes carga tres de esas. La 028 agrega ese índice, el de los
gastos por turno y el de las capturas rechazadas por fecha.

La prueba `ninguna_consulta_caliente_recorre_una_tabla_que_crece` le pregunta a
SQLite con `EXPLAIN QUERY PLAN` si va a recorrer la tabla entera, en vez de confiar en
que los índices "se vean bien". Cubre diez consultas: las de los reportes, las
partidas y pagos de una venta, los gastos del turno, los conteos y las fotos.

La regla es a propósito más estricta que el problema —nada que crezca para siempre se
recorre entero, aunque hoy tarde poco—, porque la alternativa es enterarse de la
lentitud desde 2000 km cuando ya llevan meses aguantándola. Por eso el índice de los
abonos por fecha está ahí siendo prevención y no arreglo: medido sobre tres años de
abonos, esa consulta tardaba 0.7 ms.

**La búsqueda del mostrador queda fuera de la regla, y a propósito.** Usa
`LIKE '%texto%'`, y un comodín inicial no lo sirve ningún índice de árbol; cambiarlo a
prefijo haría que buscar "vestido" no encuentre "Blusa vestido azul", que es justo
como busca quien está frente al cliente. Medido sobre 5000 productos con 1200 fotos,
cada tecla cuesta 3 ms: el catálogo tendría que crecer un orden de magnitud para que
se sienta.

### Revisiones de consistencia

Los arreglos impiden que se produzcan nuevas inconsistencias, pero **ninguna
migración rellena lo que ya estaba mal**: una tienda que vino operando con
versiones anteriores puede arrastrar historia torcida, y desde 2000 km no hay
forma de enterarse. El reporte de diagnóstico trae una sección que pregunta lo
que debería dar cero:

- Apartados entregados sin su venta (antes de la 021, entregar no dejaba venta)
- Productos con tallas cuyo total no es la suma de sus tallas
- Partidas con más piezas devueltas que vendidas
- Apartados con más abonado que su total
- Turnos cerrados sin lo que se contó
- Ventas sin ninguna partida
- Pagos con una forma que el sistema no conoce (el dinero está cobrado y el reporte
  diario no tiene columna donde enseñarlo)

Solo cuenta; no corrige. Tocar historia real es una decisión de quien es dueño de
esos datos, no del programa.

### Los datos de prueba

**Ajustes → Cargar datos de prueba** solo funciona en una instalación nueva: ni
una venta, ni un producto, ni un cliente, ni un proveedor propios. Mirar solo las
ventas no alcanzaba —una tienda pasa días capturando su catálogo antes de abrir—,
y desde `026_nada_se_borra` diez prendas inventadas metidas al catálogo de verdad
**ya no se pueden borrar**: solo dar de baja, una por una.

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

## El ticket que sobrevive a cerrar la aplicación

El carrito y las órdenes en espera se guardan en el equipo, para que un corte de
luz no obligue a rearmar la venta con la fila esperando. Llevan dentro una copia
del producto —nombre, precio y existencia de cuando se agregó—, y el cobro toma
el precio de **la base**: el que manda la pantalla se ignora a propósito para que
nadie pueda cobrarse de menos (`el_precio_sale_de_la_base_aunque_el_cliente_mienta`).
El costo era que un ticket guardado de ayer enseñaba un total y cobraba otro.

Al abrir el punto de venta y al recuperar una orden en espera, los renglones se
ponen al día contra el catálogo: se toma el precio de ahora, sale lo que se dio de
baja o se quedó sin existencia, se recorta la cantidad a lo que queda, y se dice
en pantalla qué cambió. Un renglón con talla no se recorta por el total del
producto, porque ahí la existencia vive en la talla.

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

### Exportar a CSV

Las dos exportaciones de Reportes —el reporte de ventas y las ventas por
facturar— piden dónde guardar, arman el archivo en Rust y confirman cuántos
renglones escribieron. La del reporte bajaba un blob del navegador: nadie elegía
dónde, nadie sabía dónde quedaba, y el aviso decía "exportado exitosamente" sin
haber comprobado nada, ni que el archivo se escribiera ni que alguien no hubiera
cancelado. Las dos llevan marca de orden de bytes para que Excel las abra como
UTF-8.

### El reembolso reparte sin perder centavos

Lo que se devuelve es lo que el cliente pagó por esas piezas, no lo que dice la
lista de precios: el ticket pudo llevar una promoción y un impuesto, y ninguno de
los dos aparece en el renglón. Así que el total cobrado se reparte entre las
partidas.

Repartir con una regla de tres partida por partida y redondear cada resultado al
centavo **no suma el total**. Devolver una venta de a una pieza dejaba un centavo en
la caja —o regalaba uno: con dos piezas de un centavo y 7% de descuento cobraba 9 y
devolvía 10—. Ahora se reparte por acumulado, entre partidas y entre las piezas de
cada partida: lo que les toca a las primeras *i* menos lo que les tocaba a las
primeras *i−1*. La diferencia del redondeo la recoge la siguiente en vez de
perderse, y el orden va por `id`, que no cambia, para que el reparto sea el mismo
sin importar en cuántos viajes se devuelva.

Lo cuida una malla de 300 formas de ticket —precios que dividen mal, de uno a once
piezas, con y sin promoción— que comprueba que devolver todo de a una pieza regrese
exactamente lo cobrado. Esa malla es la que encontró el bug; las ocho formas que se
me habían ocurrido a mano solo veían una de las dos direcciones.

### De dónde sale la utilidad

Del costo que se guardó en la partida al vender (`sale_items.unit_cost`), no del
costo de hoy. Las ventas anteriores a que existiera esa columna no lo traen, y
caían al costo actual del producto: subirle el costo a una prenda reescribía hacia
atrás la utilidad de todos los meses en que se vendía más barata. Para esas
partidas se busca en el historial de costos (`price_history` con `tipo = 'costo'`)
el primer cambio posterior a la venta —su `old_price` es lo que costaba ese día— y
solo si no hay ninguno se usa el costo actual, que entonces sí es el mismo.

### Todas las fechas se comparan en local

El backend compara siempre contra `date('now','localtime')` y el punto de venta
contra `toLocaleDateString('en-CA')`. Sacar "hoy" de `toISOString()` da la fecha
de **UTC**, que a partir de las seis de la tarde en México ya es la de mañana: una
promoción creada de tarde arrancaba al día siguiente y ni aparecía en el mostrador,
y la exportación de ventas por facturar se dejaba fuera un día entero. Para eso
están `hoyLocal()` y `fechaLocal(dias)` en `src/utils`, y una prueba que falla si
`DiscountsPage` vuelve a usar `toISOString`.

## Seguridad

- Contraseñas con Argon2 y salt por usuario, de ocho caracteres para arriba. El
  mínimo vive en `MIN_PASSWORD_LEN` (Rust) y la pantalla lo toma de
  `MIN_CONTRASENA`; una prueba falla si dejan de coincidir. Estaba escrito tres
  veces y una de las copias decía seis: la pantalla aceptaba una de seis
  prometiendo que bastaba y el backend la rechazaba pidiendo ocho
- Sesiones con token opaco y caducidad configurable (12 h por defecto). La
  sesión se renueva con el uso: solo vence tras ese tiempo sin actividad, y
  cuando vence la app regresa sola a la pantalla de inicio
- Los descuentos por renglón solo los da un administrador; el cobro los
  rechaza para cualquier otro usuario. Las promociones las crea un
  administrador y cualquiera puede aplicarlas
- Un producto dado de baja no se vende ni se aparta, aunque siga en un
  carrito guardado o se escanee el código de una de sus tallas
- Cada comando del backend resuelve el usuario desde su sesión; el frontend
  nunca decide quién ejecuta una operación
- Reportes, usuarios, productos, proveedores, respaldos y el Dashboard son solo de
  administrador. El rol se decide en Rust con `require_admin`; la pantalla no es
  una frontera de seguridad, solo evita ofrecer lo que va a ser negado. La prueba
  `permisosDePantalla` cruza las dos cosas: falla si una pantalla que la cajera
  puede abrir pide un comando de administrador, y si el menú ofrece un enlace que
  el enrutado rechaza
- Cada cobro lleva un identificador único: reenviar el mismo devuelve la venta
  original en vez de duplicarla
