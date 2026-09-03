//! Captura de productos desde el celular, por red local.
//!
//! La app de escritorio levanta un servidor HTTP en la red de la tienda. El
//! celular abre esa dirección en su navegador, toma las fotos y las manda. No
//! hay nube, ni servidor externo, ni cuenta que pagar: la caja *es* el servidor
//! y los datos nunca salen del local.
//!
//! Eso también significa que cualquiera conectado al mismo WiFi podría alcanzar
//! el puerto, así que:
//!
//! - el servidor está apagado por defecto y hay que encenderlo a propósito;
//! - cada encendido genera un código de emparejamiento nuevo;
//! - sin código no se acepta nada, y los intentos fallidos bloquean por un rato;
//! - solo se expone dar de alta un producto: no hay forma de leer ventas,
//!   clientes ni ningún otro dato desde ahí.

mod page;
pub mod server;

pub use server::CaptureState;
