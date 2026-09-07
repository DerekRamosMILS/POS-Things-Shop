//! Certificados para que el celular hable con la caja por HTTPS.
//!
//! No es por paranoia: es un requisito del navegador. Para que la página siga
//! funcionando con la computadora apagada tiene que quedarse guardada en el
//! teléfono, y eso solo lo permite un *service worker*, que a su vez solo corre
//! en un origen seguro. `http://192.168.x.x` no lo es y no hay forma de
//! convencer al navegador. Lo mismo pasa con las herramientas de cifrado del
//! propio navegador. Las dos cosas que hacen falta salen del mismo requisito.
//!
//! En una red local no hay quien firme un certificado, así que la caja se firma
//! el suyo: crea una autoridad propia una sola vez, la guarda, y con ella emite
//! el certificado del servidor. El teléfono instala la autoridad una vez —y solo
//! una— y a partir de ahí confía en la caja como en cualquier sitio.
//!
//! La autoridad vive en la base y no en un archivo suelto para que sobreviva a
//! restaurar un respaldo: si se perdiera habría que volver a instalarla en cada
//! teléfono. El certificado del servidor, en cambio, se vuelve a emitir solo
//! cuando cambia la dirección del equipo, que con DHCP pasa; como lo firma la
//! misma autoridad, el teléfono no se entera.

use std::net::Ipv4Addr;

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SanType,
};
use rusqlite::{params, Connection};

/// Claves de `system_config` donde vive la autoridad.
const CA_CERT: &str = "captura_ca_cert";
const CA_KEY: &str = "captura_ca_key";

/// Años que dura la autoridad. Larga a propósito: cada vencimiento obliga a
/// volver a instalarla en todos los teléfonos.
const ANIOS_CA: i32 = 10;

pub struct Identidad {
    /// Certificado del servidor y su cadena, en PEM.
    pub cert_pem: String,
    /// Llave privada del servidor, en PEM.
    pub key_pem: String,
    /// Certificado de la autoridad, que es lo que se instala en el teléfono.
    pub ca_pem: String,
}

fn leer(db: &Connection, clave: &str) -> Option<String> {
    db.query_row(
        "SELECT value FROM system_config WHERE key = ?1",
        params![clave],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .filter(|v| !v.trim().is_empty())
}

fn guardar(db: &Connection, clave: &str, valor: &str) -> Result<(), String> {
    db.execute(
        "INSERT OR REPLACE INTO system_config (key, value, description, updated_at)
         VALUES (?1, ?2, 'Certificado de la captura por celular', datetime('now','localtime'))",
        params![clave, valor],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Los datos de la autoridad.
///
/// Están en una función y no escritos dos veces porque se usan para crearla y
/// para volver a armarla en cada arranque: si las dos versiones no coincidieran,
/// los certificados que emitiera dejarían de encajar con el que está instalado
/// en el teléfono.
fn parametros_ca() -> CertificateParams {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];

    let mut nombre = DistinguishedName::new();
    // El nombre es lo que la persona va a ver en la lista de certificados del
    // teléfono el día que se pregunte qué es eso.
    nombre.push(DnType::CommonName, "Things Shop - Captura");
    nombre.push(DnType::OrganizationName, "Things Shop");
    params.distinguished_name = nombre;
    params
}

/// La autoridad de la tienda: la crea la primera vez y la reutiliza siempre.
fn autoridad(db: &Connection) -> Result<(Issuer<'static, KeyPair>, String), String> {
    if let (Some(cert_pem), Some(key_pem)) = (leer(db, CA_CERT), leer(db, CA_KEY)) {
        let key = KeyPair::from_pem(&key_pem)
            .map_err(|e| format!("La llave de la autoridad no se pudo leer: {}", e))?;
        // Se rearma desde los mismos datos con que se creó, en vez de leerlos
        // del certificado: evita arrastrar un analizador de X.509 al binario
        // para recuperar algo que ya se sabe.
        return Ok((Issuer::new(parametros_ca(), key), cert_pem));
    }

    let mut params = parametros_ca();
    params.not_after = rcgen::date_time_ymd(hoy_mas_anios(ANIOS_CA), 1, 1);

    let key = KeyPair::generate().map_err(|e| format!("No se pudo generar la llave: {}", e))?;
    let cert = params
        .self_signed(&key)
        .map_err(|e| format!("No se pudo crear la autoridad: {}", e))?;

    let cert_pem = cert.pem();
    guardar(db, CA_CERT, &cert_pem)?;
    guardar(db, CA_KEY, &key.serialize_pem())?;
    log::info!("Autoridad de certificación de la captura creada");

    Ok((Issuer::new(parametros_ca(), key), cert_pem))
}

/// Certificado del servidor para una dirección concreta.
///
/// Se emite en cada arranque en vez de guardarse: es barato, y así seguir la
/// dirección del equipo cuando el router se la cambia no requiere nada.
pub fn identidad_para(db: &Connection, ip: &str) -> Result<Identidad, String> {
    let direccion: Ipv4Addr = ip
        .parse()
        .map_err(|_| format!("La dirección {} no es válida", ip))?;

    let (emisor, ca_pem) = autoridad(db)?;

    let mut params = CertificateParams::default();
    // El navegador valida contra la dirección que se tecleó, así que tiene que
    // estar aquí como tal y no como nombre.
    params.subject_alt_names = vec![SanType::IpAddress(direccion.into())];
    params.not_after = rcgen::date_time_ymd(hoy_mas_anios(2), 1, 1);

    let mut nombre = DistinguishedName::new();
    nombre.push(DnType::CommonName, ip);
    params.distinguished_name = nombre;

    let key = KeyPair::generate().map_err(|e| format!("No se pudo generar la llave: {}", e))?;
    let cert = params
        .signed_by(&key, &emisor)
        .map_err(|e| format!("No se pudo emitir el certificado: {}", e))?;

    Ok(Identidad {
        // La cadena lleva también la autoridad: algunos clientes la piden aunque
        // ya la tengan instalada.
        cert_pem: format!("{}{}", cert.pem(), ca_pem),
        key_pem: key.serialize_pem(),
        ca_pem,
    })
}

fn hoy_mas_anios(anios: i32) -> i32 {
    use chrono::Datelike;
    chrono::Local::now().year() + anios
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db
    }

    #[test]
    fn la_autoridad_se_crea_una_vez_y_se_reutiliza() {
        // Si cambiara, cada arranque obligaría a reinstalarla en los teléfonos.
        let db = base();
        let a = identidad_para(&db, "192.168.0.10").unwrap();
        let b = identidad_para(&db, "192.168.0.10").unwrap();
        assert_eq!(a.ca_pem, b.ca_pem);
        assert!(a.ca_pem.contains("BEGIN CERTIFICATE"));
    }

    #[test]
    fn cambiar_de_direccion_no_obliga_a_reinstalar_nada() {
        // Con DHCP el router cambia la dirección del equipo. El certificado del
        // servidor se vuelve a emitir, pero la autoridad —lo que está instalado
        // en el teléfono— sigue siendo la misma.
        let db = base();
        let antes = identidad_para(&db, "192.168.0.10").unwrap();
        let despues = identidad_para(&db, "192.168.0.55").unwrap();

        assert_eq!(antes.ca_pem, despues.ca_pem, "la autoridad no debe cambiar");
        assert_ne!(antes.cert_pem, despues.cert_pem, "el del servidor sí");
    }

    #[test]
    fn la_autoridad_sobrevive_a_restaurar_un_respaldo() {
        // Vive en la base justamente para esto: si se perdiera habría que
        // volver a instalarla en cada teléfono.
        let db = base();
        let original = identidad_para(&db, "192.168.0.10").unwrap().ca_pem;

        let guardado: String = db
            .query_row("SELECT value FROM system_config WHERE key = ?1", params![CA_CERT], |r| r.get(0))
            .unwrap();
        assert_eq!(guardado, original);
    }

    #[test]
    fn una_direccion_que_no_es_direccion_se_rechaza() {
        let db = base();
        assert!(identidad_para(&db, "no-soy-una-ip").is_err());
        assert!(identidad_para(&db, "").is_err());
    }
}

/// Utilidad de diagnóstico: vuelca la identidad a archivos para inspeccionarla.
#[cfg(test)]
pub fn volcar_para_inspeccion(db: &Connection, ip: &str, dir: &std::path::Path) {
    let id = identidad_para(db, ip).unwrap();
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("ca.pem"), &id.ca_pem).unwrap();
    std::fs::write(dir.join("cadena.pem"), &id.cert_pem).unwrap();
    std::fs::write(dir.join("llave.pem"), &id.key_pem).unwrap();
}

#[cfg(test)]
mod verificacion {
    use super::*;

    #[test]
    fn el_certificado_del_servidor_lo_valida_la_autoridad() {
        // Es la prueba que importa: si esto falla, el teléfono dice "no es
        // seguro" y no hay service worker, que es todo el punto.
        let db = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        let dir = std::env::temp_dir().join(format!("ts-cert-{}", uuid::Uuid::new_v4()));
        volcar_para_inspeccion(&db, "192.168.0.55", &dir);

        let salida = std::process::Command::new("openssl")
            .args(["verify", "-CAfile"])
            .arg(dir.join("ca.pem"))
            .arg(dir.join("cadena.pem"))
            .output();

        let Ok(salida) = salida else {
            eprintln!("openssl no está disponible; se omite la verificación de cadena");
            std::fs::remove_dir_all(&dir).ok();
            return;
        };

        let texto = String::from_utf8_lossy(&salida.stdout).to_string()
            + &String::from_utf8_lossy(&salida.stderr);
        std::fs::remove_dir_all(&dir).ok();
        assert!(salida.status.success(), "la cadena no valida: {}", texto);
    }

    #[test]
    fn el_certificado_dice_la_direccion_correcta() {
        // El navegador compara la dirección que se tecleó contra la del
        // certificado. Si no coincide, avisa de que algo anda mal aunque la
        // autoridad esté instalada.
        let db = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        let dir = std::env::temp_dir().join(format!("ts-san-{}", uuid::Uuid::new_v4()));
        volcar_para_inspeccion(&db, "192.168.0.55", &dir);

        let salida = std::process::Command::new("openssl")
            .args(["x509", "-noout", "-text", "-in"])
            .arg(dir.join("cadena.pem"))
            .output();

        let Ok(salida) = salida else {
            std::fs::remove_dir_all(&dir).ok();
            return;
        };
        let texto = String::from_utf8_lossy(&salida.stdout).to_string();
        std::fs::remove_dir_all(&dir).ok();
        assert!(texto.contains("IP Address:192.168.0.55"), "sin la dirección: {}", texto);
    }
}

/// Prueba de extremo a extremo del camino cifrado.
///
/// Verificar la cadena con openssl comprueba que el certificado esté bien
/// firmado, que es necesario pero no suficiente: falta que rustls acepte el par
/// certificado/llave y que un cliente que solo confía en la autoridad complete
/// el saludo contra el servidor de verdad. Si eso falla, el teléfono dice "no es
/// seguro" y no hay nada de lo demás: ni guardado sin conexión ni cifrado.
///
/// El cliente es rustls y no `openssl s_client` porque las banderas de openssl
/// cambian entre macOS y Linux, y una prueba que solo corre en la máquina de
/// quien la escribió no protege de nada.
#[cfg(test)]
mod extremo_a_extremo {
    use super::*;
    use std::sync::Arc;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_rustls::rustls::pki_types::ServerName;
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};
    use tokio_rustls::TlsConnector;

    /// Un cliente que confía únicamente en la autoridad de esta tienda, como el
    /// teléfono después de instalarla.
    fn cliente_que_confia_solo_en(ca_pem: &str) -> ClientConfig {
        let mut raiz = RootCertStore::empty();
        let mut cursor = std::io::Cursor::new(ca_pem.as_bytes());
        for cert in rustls_pemfile::certs(&mut cursor) {
            raiz.add(cert.expect("la autoridad no se pudo leer")).unwrap();
        }
        ClientConfig::builder()
            .with_root_certificates(raiz)
            .with_no_client_auth()
    }

    async fn servidor_de_prueba(id: &Identidad) -> (u16, axum_server::Handle<std::net::SocketAddr>) {
        let config = axum_server::tls_rustls::RustlsConfig::from_pem(
            id.cert_pem.clone().into_bytes(),
            id.key_pem.clone().into_bytes(),
        )
        .await
        .expect("rustls no aceptó el certificado");

        let escucha = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        // Igual que en el servidor de verdad: entregarle a tokio un socket
        // bloqueante revienta la tarea y la captura nunca arranca, con la
        // pantalla diciendo que está encendida. Esta prueba existe por eso.
        escucha.set_nonblocking(true).unwrap();
        let puerto = escucha.local_addr().unwrap().port();

        let app = axum::Router::new().route("/", axum::routing::get(|| async { "hola" }));
        let manija = axum_server::Handle::new();
        {
            let manija = manija.clone();
            tokio::spawn(async move {
                let _ = axum_server::from_tcp_rustls(escucha, config)
                    .unwrap()
                    .handle(manija)
                    .serve(app.into_make_service())
                    .await;
            });
        }
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        (puerto, manija)
    }

    #[tokio::test]
    async fn el_telefono_con_la_autoridad_instalada_entra_sin_advertencias() {
        let db = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        let id = identidad_para(&db, "127.0.0.1").unwrap();
        let (puerto, manija) = servidor_de_prueba(&id).await;

        let conector = TlsConnector::from(Arc::new(cliente_que_confia_solo_en(&id.ca_pem)));
        let tcp = tokio::net::TcpStream::connect(("127.0.0.1", puerto)).await.unwrap();
        // Se conecta por la dirección, que es lo que va a teclear el teléfono.
        let destino = ServerName::try_from("127.0.0.1").unwrap();

        let mut flujo = conector
            .connect(destino, tcp)
            .await
            .expect("el cliente no confió en la caja");

        flujo
            .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut respuesta = Vec::new();
        flujo.read_to_end(&mut respuesta).await.ok();
        // Se apaga después de leer: hacerlo antes cortaba la respuesta a medias.
        manija.graceful_shutdown(Some(std::time::Duration::from_millis(100)));

        let texto = String::from_utf8_lossy(&respuesta);
        assert!(texto.contains("hola"), "no llegó la página: {}", texto);
    }

    #[tokio::test]
    async fn sin_la_autoridad_instalada_el_telefono_desconfia() {
        // El contrapeso de la prueba anterior: si esto pasara, el certificado
        // estaría siendo aceptado por algo que no es la autoridad de la tienda.
        let db = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        let id = identidad_para(&db, "127.0.0.1").unwrap();
        let (puerto, manija) = servidor_de_prueba(&id).await;

        // Una autoridad distinta: la de otra tienda cualquiera.
        let otra = Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&otra).unwrap();
        let ajena = identidad_para(&otra, "127.0.0.1").unwrap();

        let conector = TlsConnector::from(Arc::new(cliente_que_confia_solo_en(&ajena.ca_pem)));
        let tcp = tokio::net::TcpStream::connect(("127.0.0.1", puerto)).await.unwrap();
        let resultado = conector
            .connect(ServerName::try_from("127.0.0.1").unwrap(), tcp)
            .await;
        manija.graceful_shutdown(Some(std::time::Duration::from_millis(100)));

        assert!(resultado.is_err(), "debió rechazar un certificado ajeno");
    }
}
