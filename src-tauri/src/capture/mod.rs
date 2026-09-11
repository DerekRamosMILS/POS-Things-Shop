//! Captura de productos desde el celular.
//!
//! El teléfono captura sin conexión y deja lo capturado en un buzón en internet
//! (el relevo, `relevo/` en la raíz del repositorio). Este punto de venta pasa a
//! recogerlo cada pocos minutos. Nunca se hablan directo.
//!
//! Antes el teléfono se conectaba a un servidor que esta computadora levantaba
//! en el WiFi de la tienda. Funcionaba en la mesa de pruebas y no en la tienda:
//! el router aísla a los clientes entre sí, y ningún permiso de firewall lo
//! cambia. Hacia internet, en cambio, los dos salen sin problema. El buzón vive
//! donde los dos llegan.
//!
//! - `producto`: da de alta lo capturado —tallas, colores, piezas, fotos—.
//! - `conteo`: aplica un conteo respetando lo vendido mientras tanto.
//! - `relevo`: el emparejamiento, el QR y la recogida.

pub mod conteo;
pub mod producto;
pub mod relevo;
