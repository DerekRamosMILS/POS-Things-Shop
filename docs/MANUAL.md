# Manual de operación — Things Shop POS

Este manual es para imprimir y dejar junto a la caja. Dentro de la aplicación,
la tecla **F1** abre la misma información en pantalla.

---

## Para el cajero

### Empezar el día

1. Entra a **Caja** → **Abrir Caja**.
2. Cuenta el dinero con el que arrancas y captúralo como fondo.

Sin caja abierta no se puede cobrar. Es a propósito: así el corte del día
siempre cuadra contra algo.

### Cobrar

1. Escanea los productos, o búscalos con **F2**.
2. Si el producto tiene tallas o colores, elige la variante.
3. **F10** para cobrar.
4. Elige forma de pago.
5. En efectivo, captura lo que recibiste. El cambio se calcula solo.

El lector funciona siempre, aunque el cursor esté dentro de un campo.

### Pago con dos formas a la vez

En la pantalla de cobro elige **Mixto**. Captura cuánto va en tarjeta y cuánto
en transferencia; lo que falte se cubre en efectivo y ahí se calcula el cambio.
El ticket sale con el desglose.

### Apartados

- **F9** convierte la orden en apartado.
- El inventario se descuenta desde que se crea el apartado.
- Cada abono imprime un comprobante con el saldo que resta.
- Solo se puede entregar cuando el saldo llega a cero.

### Devoluciones

Ventas → abre la venta → **Devolver artículos**. Indica **cómo** le regresaste
el dinero: solo el efectivo sale del cajón. Para devolver efectivo tiene que
haber una caja abierta.

### Cliente que pide factura

El cliente debe estar dado de alta en **Clientes** con su RFC. Selecciónalo en
el ticket antes de cobrar y marca **Requiere factura**.

### Cerrar el día

**Caja** → **Cerrar Caja**. El sistema muestra qué debe haber en el cajón y de
dónde sale cada monto. Cuenta el dinero y captúralo. Si hay diferencia queda
registrada — repórtala, no la escondas.

### Atajos

| Tecla | Qué hace |
|---|---|
| F1 | Ayuda |
| F2 | Buscador de productos |
| F3 | Nombre del cliente |
| F4 | Descuento de la línea seleccionada |
| F8 | Poner la orden en espera |
| F9 | Convertir en apartado |
| F10 | Cobrar |
| ↑ ↓ | Seleccionar línea del ticket |
| ← → | Quitar o agregar una unidad |
| Supr | Quitar la línea seleccionada |
| Esc | Cerrar ventana o volver al buscador |

---

## Para el administrador

### Primer arranque

Entra con el usuario `admin` y la contraseña `admin1234`. **El sistema no deja
pasar hasta cambiarla.** Después crea un usuario por cada cajero: las ventas se
registran a nombre de quien tiene la sesión abierta.

### Conectar el hardware

**Lector de código de barras.** Conéctalo, entra a **Ajustes → Lector de código
de barras**, haz clic en Prueba de lectura y escanea cualquier producto. El
sistema mide su velocidad, detecta con qué tecla termina y se calibra solo.

**Impresora y cajón.** El cajón se conecta con un cable telefónico al puerto DK
**de la impresora**, no a la computadora. En Ajustes elige la impresora, el
ancho del papel (58 u 80 mm) y usa **Imprimir prueba** y **Probar cajón**. Si el
cajón no responde, cambia el comando a `1B 70 01 19 FA`: algunos modelos usan el
pin 5 en lugar del pin 2.

### Respaldos

Se crean solos al cerrar la caja, si está activado en Ajustes. También se pueden
crear a mano y exportar a una memoria USB.

**Restaurar reemplaza todo lo posterior al respaldo.** El cambio se aplica al
reiniciar la aplicación, no en el momento.

### Configuración que conviene revisar

| Ajuste | Para qué |
|---|---|
| Tasa de impuesto | 0 si los precios ya lo incluyen |
| Umbral de stock mínimo | Cuándo avisa que hay que resurtir |
| Duración de la sesión | Cuánto dura una sesión antes de pedir contraseña |
| Máximo de respaldos | Cuántos conservar antes de borrar los viejos |
| Días de bitácora | Cuánto historial técnico guardar |

### Facturación

Captura los datos fiscales del cliente en **Clientes → Datos para facturar**. En
**Reportes → Por facturar** se exportan las ventas marcadas que aún no tienen
folio fiscal, en CSV, para entregárselo al contador o al PAC.

El sistema no timbra facturas: prepara los datos.

### Cuando algo falla

1. **Ajustes → Reporte de diagnóstico** genera un archivo con la versión, el
   estado de la base, la configuración y la bitácora.
2. Envía ese archivo. Contiene lo necesario para diagnosticar y no incluye
   contraseñas.

### Actualizaciones

**Ajustes → Buscar actualizaciones.** Cierra la caja antes de actualizar: la
aplicación se reinicia al terminar.
