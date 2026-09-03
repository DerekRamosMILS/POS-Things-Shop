# Decisiones de arquitectura

Las decisiones que condicionan lo que el sistema puede y no puede hacer, y el
costo de cambiarlas.

## Una caja, no varias

**Estado actual:** el sistema corre en un solo equipo. La base de datos es un
archivo SQLite local; no hay servidor ni red.

**Por qué:** para una tienda con un mostrador es la opción correcta. No depende
de internet, no hay servidor que administrar, arranca instantáneo y el respaldo
es copiar un archivo. Un POS que deja de vender porque se cayó la red es peor
que uno que nunca la necesitó.

**Qué no se puede hacer hoy:** una segunda caja cobrando al mismo tiempo, una
tablet para inventario, o consultar las ventas desde otro lugar.

**Lo que NO hay que hacer:** poner la base en una carpeta compartida de red.
SQLite sobre SMB o similar corrompe datos con escrituras concurrentes. Es la
solución que parece obvia y es la que destruye la información.

**Si algún día hacen falta dos cajas**, hay dos caminos:

| Camino | Qué implica | Cuándo conviene |
|---|---|---|
| Servidor local | Un equipo hace de servidor (Postgres o SQLite con un servicio HTTP delante) y las cajas son clientes | Dos o más cajas en la misma tienda |
| Nube | Backend hospedado; las cajas sincronizan | Varias sucursales, o consultar desde fuera |

**Lo que ya está listo para ese día:**

- Todo el acceso a datos pasa por comandos del backend. La interfaz nunca toca
  la base directamente, así que el cambio se concentra en una capa.
- Cada venta y cada turno de caja guardan `terminal_id`, así que los datos de
  dos equipos se pueden distinguir y reconciliar.
- Los folios llevan el identificador de terminal en cuanto deja de ser `01`:
  la caja 1 emite `V-20260823-001` y la caja 2 emite `V02-20260823-001`, sin
  posibilidad de chocar.

**Lo que faltaría:** mover `SessionState` fuera del proceso, reemplazar el acceso
directo a SQLite por llamadas al servidor y resolver qué pasa cuando una caja se
queda sin red a media venta. Es un trabajo de semanas, no de días.

## Facturación electrónica (CFDI)

**Estado actual:** el sistema captura, valida y conserva los datos fiscales, y
exporta las ventas por facturar en CSV. **No timbra.**

**Por qué:** timbrar exige un contrato con un PAC autorizado y los certificados
de sello digital del negocio. Nada de eso se puede dar por supuesto desde el
código.

**Lo que ya funciona:**

- RFC, razón social, régimen fiscal, código postal y uso de CFDI por cliente,
  validados contra los catálogos del SAT al capturarlos.
- Al cobrar se marca si la venta requiere factura y se guarda una **copia** de
  los datos fiscales del cliente en ese momento — si después cambia su RFC, la
  factura se emite con el que estaba vigente al vender.
- Exportación en CSV de las ventas por facturar, desde Reportes.
- `marcar_facturada` registra el folio fiscal (UUID) que devuelve el PAC, para
  que esa venta deje de aparecer como pendiente.

**Lo que falta para timbrar desde la app:** elegir PAC, cargar los CSD del
negocio, generar el XML del CFDI 4.0, sellarlo y enviarlo. Los datos ya están
listos; lo que falta es el paso de sellado.

## El dinero se calcula en centavos, se guarda en decimales

**Estado actual:** todo cálculo monetario ocurre en enteros de centavos
(`money::Cents`). El almacenamiento en la base sigue siendo `REAL`.

**Por qué:** el problema real del punto flotante no es guardar un importe, es
**acumularlo**: sumar `0.1` diez veces en `f64` no da `1.0`. Un ticket de 300
partidas o un turno con cientos de movimientos arrastran ese error hasta el
corte. Operando en centavos eso desaparece.

Cambiar el esquema de doce tablas con datos ya cargados tiene un riesgo mayor
que el problema que resolvería: los importes se redondean al centavo en cada
frontera, y un `REAL` representa cualquier valor de dos decimales con precisión
más que suficiente para los montos de una tienda.

**Si algún día se migra a enteros en la base**, el trabajo ya está a medio
camino: la aritmética vive en un solo tipo y solo habría que mover la frontera
de conversión.

## El descuento lo decide el servidor

El punto de venta manda **qué** promoción se aplicó, nunca **cuánto** descuenta.
El backend valida vigencia, estado y alcance, y recalcula el importe. Los
descuentos por línea se topan al valor de la línea.

Antes el frontend mandaba el importe y el backend lo aceptaba, lo que permitía
dejar cualquier venta en cero desde un cliente manipulado.

## El ticket se arma desde la base

Los tickets no se construyen con lo que hay en pantalla sino con lo que quedó
guardado de la venta. El papel dice exactamente lo que se cobró, y por eso una
venta de hace un mes se puede reimprimir igual.
