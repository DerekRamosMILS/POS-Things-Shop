# Relevo de capturas

Buzón entre el celular y el punto de venta. Existe por una asimetría de red que
no se arregla con código: desde la tienda **se sale** a internet sin problema —la
aplicación se actualiza sola— pero **no se entra**. El router aísla a los
clientes entre sí, o el teléfono y la computadora están en redes distintas, y
ningún permiso de firewall lo cambia.

Un buzón al que los dos lados llegan por su cuenta sí funciona.

```
Teléfono (PWA)  ──sube──►  Worker + KV  ◄──baja── Punto de venta
                            (de paso)             y confirma: bórralo
```

## Qué pasa por aquí y qué no

Pasa lo que se capturó con el teléfono y todavía no llega a su casa: nombre,
precio, tallas, colores, piezas y fotos de mercancía.

**No pasa nada** de ventas, clientes, caja ni inventario. El punto de venta solo
baja; nunca expone nada.

Y está **de paso**: el punto de venta se lo lleva, confirma, y se borra. Lo que
nadie recoja caduca solo a los 30 días, así que esto no puede convertirse en un
archivo permanente de las fotos de la tienda ni por descuido.

## El secreto

No hay registro de tiendas ni lista de secretos válidos. El punto de venta emite
32 bytes al azar, y la carpeta de esa tienda es `SHA-256` de ese secreto. El
secreto no se guarda en ningún lado: quien lo tenga entra a su carpeta, quien no,
no ve ninguna.

Viaja al teléfono en el QR, después del `#`, que es la parte de una URL que
**nunca se manda a ningún servidor**: no aparece en registros ni se filtra por el
Referer.

## Rutas

| | |
|---|---|
| `GET /api/salud` | Sin secreto. Distingue "el relevo no responde" de "mi secreto no sirve" |
| `GET /api/verificar` | Que el secreto tiene forma de secreto |
| `POST /api/subir` | Una captura. `captura_id` la pone el teléfono: reintentar no duplica |
| `GET /api/pendientes` | Lo que falta por recoger, sin el contenido |
| `GET /api/pendiente/:id` | El contenido de una |
| `POST /api/recibido` | El punto de venta ya la guardó: bórrala |
| `PUT` / `GET /api/catalogo` | El catálogo para contar, sin existencias |
| `POST /api/retirar` | Este código ya no sirve para subir. La tienda sí puede seguir recogiendo lo que quedó |

## Retirar un código

Como no hay lista de secretos, el relevo no tiene cómo saber que la tienda
cambió el suyo. Sin `/api/retirar`, un teléfono sin re-emparejar seguía subiendo
con el código viejo: recibía "ok", borraba su copia, y la tienda ya nunca miraba
esa carpeta. Ahora queda una marca (`r/<huella>`, un año) y el relevo le niega al
teléfono `subir`, `verificar` y el catálogo con un 401. El teléfono conserva lo
capturado y pide volver a escanear.

## Trabajar aquí

```bash
pnpm install
pnpm test      # contra el runtime real de Workers, con KV de verdad
pnpm dev       # local
pnpm deploy
pnpm tail      # registros en vivo
```

El listado de KV es **eventualmente consistente**: puede enseñar algo que ya se
borró. No es un problema porque el punto de venta reconoce lo que ya tiene por su
`captura_id`, igual que hace con las capturas de la red local.
