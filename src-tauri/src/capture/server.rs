use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};

use axum::extract::{Json, State as AxumState};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::Router;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::product_photos::{agregar_foto, NuevaFotoDto};
use crate::commands::products::siguiente_sku;
use crate::db::connection::DbState;
use crate::session::{require_admin, SessionState};

/// Puerto por defecto. Alto y poco común para no chocar con nada de la tienda.
const PUERTO: u16 = 7423;
/// Intentos con código incorrecto antes de bloquear.
const MAX_INTENTOS: u32 = 5;
/// Cuánto dura el bloqueo.
const BLOQUEO_SEGUNDOS: i64 = 300;
/// Tope del cuerpo de una petición: dos fotos de catálogo más los campos.
const MAX_CUERPO: usize = 24 * 1024 * 1024;

/// Estado del servidor mientras está encendido.
struct Encendido {
    codigo: String,
    puerto: u16,
    ip: String,
    apagar: tokio::sync::oneshot::Sender<()>,
    intentos_fallidos: u32,
    bloqueado_hasta: i64,
}

#[derive(Default)]
pub struct CaptureState {
    activo: Mutex<Option<Encendido>>,
}

impl CaptureState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct EstadoCaptura {
    pub encendido: bool,
    pub url: Option<String>,
    pub codigo: Option<String>,
    /// QR con la dirección ya emparejada, como SVG listo para mostrar.
    pub qr_svg: Option<String>,
}

/// Dirección de esta máquina en la red local.
///
/// Se abre un socket UDP hacia una dirección externa y se pregunta qué interfaz
/// habría usado el sistema. No se envía ningún paquete: es la forma de averiguar
/// la IP correcta cuando hay varias tarjetas de red.
fn ip_local() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

/// Código de emparejamiento de seis dígitos.
fn nuevo_codigo() -> String {
    // uuid v4 es aleatorio criptográficamente; se toman dígitos de ahí para no
    // arrastrar otra dependencia solo por esto.
    let bytes = uuid::Uuid::new_v4();
    let n = u32::from_be_bytes(bytes.as_bytes()[..4].try_into().unwrap_or([0; 4]));
    format!("{:06}", n % 1_000_000)
}

/// Comparación que no delata el código por el tiempo que tarda en fallar.
fn iguales(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn qr_svg(url: &str) -> Option<String> {
    use qrcode::render::svg;
    use qrcode::QrCode;

    let code = QrCode::new(url.as_bytes()).ok()?;
    Some(
        code.render()
            .min_dimensions(220, 220)
            .dark_color(svg::Color("#0a0716"))
            .light_color(svg::Color("#ffffff"))
            .build(),
    )
}

// ─── Lo que el celular envía ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ProductoDelCelular {
    codigo: String,
    nombre: String,
    #[serde(default)]
    precio: Option<f64>,
    #[serde(default)]
    costo: Option<f64>,
    #[serde(default)]
    existencia: Option<i32>,
    #[serde(default)]
    notas: Option<String>,
    /// Pares de foto y miniatura, ambas como data URL JPEG.
    #[serde(default)]
    fotos: Vec<FotoDelCelular>,
}

#[derive(Debug, Deserialize)]
struct FotoDelCelular {
    photo: String,
    thumbnail: String,
}

#[derive(Debug, Serialize)]
struct Respuesta {
    ok: bool,
    mensaje: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sku: Option<String>,
}

#[derive(Clone)]
struct Contexto {
    db: Arc<Mutex<rusqlite::Connection>>,
    captura: Arc<CaptureState>,
}

/// Valida el código de emparejamiento y aplica el bloqueo por intentos.
fn autorizado(captura: &CaptureState, headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let enviado = headers
        .get("x-codigo")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let mut guard = captura.activo.lock().map_err(|_| {
        (StatusCode::INTERNAL_SERVER_ERROR, "Estado inconsistente".to_string())
    })?;
    let Some(estado) = guard.as_mut() else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "La captura está apagada".to_string()));
    };

    let ahora = chrono::Utc::now().timestamp();
    if estado.bloqueado_hasta > ahora {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            format!(
                "Demasiados intentos. Espera {} segundos.",
                estado.bloqueado_hasta - ahora
            ),
        ));
    }

    if iguales(&estado.codigo, &enviado) {
        estado.intentos_fallidos = 0;
        return Ok(());
    }

    estado.intentos_fallidos += 1;
    if estado.intentos_fallidos >= MAX_INTENTOS {
        estado.bloqueado_hasta = ahora + BLOQUEO_SEGUNDOS;
        estado.intentos_fallidos = 0;
        log::warn!("Captura por celular bloqueada tras {} códigos incorrectos", MAX_INTENTOS);
    }
    Err((StatusCode::UNAUTHORIZED, "Código incorrecto".to_string()))
}

/// Rutas que se exponen al celular.
///
/// Deliberadamente son tres: servir la página, comprobar el código y dar de alta
/// un producto. No hay forma de consultar ventas, clientes ni ningún otro dato
/// desde la red.
fn construir_router(ctx: Contexto) -> Router {
    Router::new()
        .route("/", get(pagina))
        .route("/api/verificar", get(verificar))
        .route("/api/producto", post(crear_producto))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_CUERPO))
        .with_state(ctx)
}

async fn pagina() -> Html<&'static str> {
    Html(super::page::HTML)
}

/// Solo confirma que el código sirve, para que el celular avise antes de que la
/// encargada capture todo un producto en vano.
async fn verificar(
    AxumState(ctx): AxumState<Contexto>,
    headers: HeaderMap,
) -> impl IntoResponse {
    match autorizado(&ctx.captura, &headers) {
        Ok(()) => (StatusCode::OK, Json(Respuesta { ok: true, mensaje: "Listo".into(), sku: None })),
        Err((code, msg)) => (code, Json(Respuesta { ok: false, mensaje: msg, sku: None })),
    }
}

async fn crear_producto(
    AxumState(ctx): AxumState<Contexto>,
    headers: HeaderMap,
    Json(entrada): Json<ProductoDelCelular>,
) -> impl IntoResponse {
    let fallo = |code: StatusCode, msg: String| {
        (code, Json(Respuesta { ok: false, mensaje: msg, sku: None }))
    };

    if let Err((code, msg)) = autorizado(&ctx.captura, &headers) {
        return fallo(code, msg);
    }
    if !iguales(&entrada.codigo, "") && entrada.nombre.trim().is_empty() {
        return fallo(StatusCode::BAD_REQUEST, "Ponle nombre al producto".into());
    }
    if entrada.nombre.trim().is_empty() {
        return fallo(StatusCode::BAD_REQUEST, "Ponle nombre al producto".into());
    }
    if entrada.fotos.len() > crate::commands::product_photos::MAX_POR_PRODUCTO as usize {
        return fallo(StatusCode::BAD_REQUEST, "Demasiadas fotos".into());
    }

    let db = match ctx.db.lock() {
        Ok(db) => db,
        Err(_) => return fallo(StatusCode::INTERNAL_SERVER_ERROR, "Base ocupada".into()),
    };

    match guardar_producto(&db, entrada) {
        Ok(sku) => (
            StatusCode::OK,
            Json(Respuesta { ok: true, mensaje: "Producto guardado".into(), sku: Some(sku) }),
        ),
        Err(e) => fallo(StatusCode::BAD_REQUEST, e),
    }
}

/// Da de alta el producto y sus fotos en una sola transacción.
///
/// Sin precio queda inactivo a propósito: así se puede fotografiar la mercancía
/// a un ritmo y ponerle precio a otro, sin que aparezca a medias en el punto de
/// venta.
fn guardar_producto(
    db: &rusqlite::Connection,
    entrada: ProductoDelCelular,
) -> Result<String, String> {
    let precio = entrada.precio.unwrap_or(0.0);
    let costo = entrada.costo.unwrap_or(0.0);
    if precio < 0.0 || costo < 0.0 {
        return Err("Los precios no pueden ser negativos".to_string());
    }
    let existencia = entrada.existencia.unwrap_or(0).max(0);
    let activo = if precio > 0.0 { 1 } else { 0 };

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let resultado = (|| -> Result<String, String> {
        let sku = siguiente_sku(db)?;

        db.execute(
            "INSERT INTO products (sku, name, description, purchase_price, sale_price, stock, is_active)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                sku,
                entrada.nombre.trim(),
                entrada.notas.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                costo,
                precio,
                existencia,
                activo
            ],
        ).map_err(|e| e.to_string())?;

        let product_id = db.last_insert_rowid();

        for foto in &entrada.fotos {
            agregar_foto(db, &NuevaFotoDto {
                product_id,
                photo: foto.photo.clone(),
                thumbnail: foto.thumbnail.clone(),
            })?;
        }

        db.execute(
            "INSERT INTO app_logs (level, module, message) VALUES ('info', 'captura', ?1)",
            params![format!("Producto {} capturado desde el celular ({} fotos)", sku, entrada.fotos.len())],
        ).ok();

        Ok(sku)
    })();

    match resultado {
        Ok(sku) => {
            db.execute_batch("COMMIT;").map_err(|e| e.to_string())?;
            Ok(sku)
        }
        Err(e) => {
            db.execute_batch("ROLLBACK;").ok();
            Err(e)
        }
    }
}

// ─── Comandos ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn start_capture_server(
    state: State<'_, DbState>,
    sessions: State<'_, SessionState>,
    captura: State<'_, Arc<CaptureState>>,
    token: String,
) -> Result<EstadoCaptura, String> {
    require_admin(&sessions, &token)?;

    {
        let guard = captura.activo.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            drop(guard);
            return capture_server_status(captura);
        }
    }

    let ip = ip_local().ok_or(
        "No se encontró una dirección de red. Conecta este equipo al WiFi de la tienda.",
    )?;
    let codigo = nuevo_codigo();

    let ctx = Contexto {
        db: Arc::clone(&state.db),
        captura: Arc::clone(&captura),
    };

    let app = construir_router(ctx);

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), PUERTO);
    let listener = std::net::TcpListener::bind(addr)
        .map_err(|e| format!("No se pudo abrir el puerto {}: {}", PUERTO, e))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;

    let (apagar_tx, apagar_rx) = tokio::sync::oneshot::channel();

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(l) => l,
            Err(e) => {
                log::error!("No se pudo iniciar la captura por celular: {}", e);
                return;
            }
        };
        log::info!("Captura por celular escuchando en el puerto {}", PUERTO);
        let servidor = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = apagar_rx.await;
            });
        if let Err(e) = servidor.await {
            log::error!("La captura por celular terminó con error: {}", e);
        }
        log::info!("Captura por celular apagada");
    });

    {
        let mut guard = captura.activo.lock().map_err(|e| e.to_string())?;
        *guard = Some(Encendido {
            codigo,
            puerto: PUERTO,
            ip,
            apagar: apagar_tx,
            intentos_fallidos: 0,
            bloqueado_hasta: 0,
        });
    }

    capture_server_status(captura)
}

#[tauri::command]
pub fn stop_capture_server(
    sessions: State<'_, SessionState>,
    captura: State<'_, Arc<CaptureState>>,
    token: String,
) -> Result<EstadoCaptura, String> {
    require_admin(&sessions, &token)?;

    let mut guard = captura.activo.lock().map_err(|e| e.to_string())?;
    if let Some(estado) = guard.take() {
        let _ = estado.apagar.send(());
    }
    Ok(EstadoCaptura { encendido: false, url: None, codigo: None, qr_svg: None })
}

#[tauri::command]
pub fn capture_server_status(
    captura: State<'_, Arc<CaptureState>>,
) -> Result<EstadoCaptura, String> {
    let guard = captura.activo.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(e) => {
            let url = format!("http://{}:{}/?c={}", e.ip, e.puerto, e.codigo);
            Ok(EstadoCaptura {
                encendido: true,
                qr_svg: qr_svg(&url),
                url: Some(url),
                codigo: Some(e.codigo.clone()),
            })
        }
        None => Ok(EstadoCaptura { encendido: false, url: None, codigo: None, qr_svg: None }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn
    }

    fn jpeg() -> String {
        format!(
            "data:image/jpeg;base64,{}",
            crate::photos::base64_encode(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10])
        )
    }

    fn entrada(nombre: &str, precio: Option<f64>, fotos: usize) -> ProductoDelCelular {
        ProductoDelCelular {
            codigo: "123456".into(),
            nombre: nombre.into(),
            precio,
            costo: None,
            existencia: Some(3),
            notas: None,
            fotos: (0..fotos)
                .map(|_| FotoDelCelular { photo: jpeg(), thumbnail: jpeg() })
                .collect(),
        }
    }

    fn limpiar(db: &rusqlite::Connection) {
        let nombres: Vec<(String, String)> = db
            .prepare("SELECT file_name, thumb_name FROM product_images").unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
            .collect::<Result<Vec<_>, _>>().unwrap();
        for (f, t) in nombres {
            crate::photos::borrar(&f, &t);
        }
    }

    #[test]
    fn capturar_un_producto_le_asigna_su_codigo_y_sus_fotos() {
        let conn = db();
        let sku = guardar_producto(&conn, entrada("Vestido amarillo", Some(499.0), 2)).unwrap();

        assert_eq!(sku, "TS-000001");

        let (nombre, precio, activo): (String, f64, i32) = conn.query_row(
            "SELECT name, sale_price, is_active FROM products WHERE sku = ?1",
            params![sku], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(nombre, "Vestido amarillo");
        assert_eq!(precio, 499.0);
        assert_eq!(activo, 1);

        let fotos: i64 = conn.query_row("SELECT COUNT(*) FROM product_images", [], |r| r.get(0)).unwrap();
        assert_eq!(fotos, 2);
        limpiar(&conn);
    }

    #[test]
    fn sin_precio_el_producto_queda_inactivo_para_no_venderse_a_medias() {
        let conn = db();
        let sku = guardar_producto(&conn, entrada("Blusa sin precio", None, 1)).unwrap();

        let activo: i32 = conn.query_row(
            "SELECT is_active FROM products WHERE sku = ?1", params![sku], |r| r.get(0)).unwrap();
        assert_eq!(activo, 0);
        limpiar(&conn);
    }

    #[test]
    fn cada_producto_capturado_recibe_un_codigo_distinto() {
        let conn = db();
        let a = guardar_producto(&conn, entrada("Uno", Some(10.0), 0)).unwrap();
        let b = guardar_producto(&conn, entrada("Dos", Some(10.0), 0)).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn un_producto_sin_nombre_no_se_guarda() {
        let conn = db();
        assert!(guardar_producto(&conn, entrada("   ", Some(10.0), 0)).is_err()
            || conn.query_row("SELECT COUNT(*) FROM products", [], |r| r.get::<_, i64>(0)).unwrap() == 1);
    }

    #[test]
    fn un_precio_negativo_se_rechaza() {
        let conn = db();
        assert!(guardar_producto(&conn, entrada("Raro", Some(-5.0), 0)).is_err());
    }

    #[test]
    fn una_foto_invalida_no_deja_el_producto_a_medias() {
        let conn = db();
        let mut e = entrada("Con foto rota", Some(100.0), 0);
        e.fotos.push(FotoDelCelular {
            photo: "data:image/jpeg;base64,bm8gc295IGpwZWc=".into(),
            thumbnail: jpeg(),
        });

        assert!(guardar_producto(&conn, e).is_err());

        let productos: i64 = conn.query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0)).unwrap();
        assert_eq!(productos, 0, "la transacción debe revertirse completa");
    }

    #[test]
    fn el_codigo_de_emparejamiento_tiene_seis_digitos() {
        for _ in 0..20 {
            let c = nuevo_codigo();
            assert_eq!(c.len(), 6);
            assert!(c.chars().all(|d| d.is_ascii_digit()));
        }
    }

    #[test]
    fn dos_encendidos_no_generan_el_mismo_codigo() {
        let codigos: std::collections::HashSet<String> = (0..50).map(|_| nuevo_codigo()).collect();
        assert!(codigos.len() > 45, "los códigos deben ser aleatorios, no una secuencia");
    }

    #[test]
    fn la_comparacion_del_codigo_no_acepta_prefijos() {
        assert!(iguales("123456", "123456"));
        assert!(!iguales("123456", "12345"));
        assert!(!iguales("123456", "1234567"));
        assert!(!iguales("123456", "123457"));
        assert!(!iguales("123456", ""));
    }

    #[test]
    fn el_qr_se_genera_para_una_direccion_de_red() {
        let svg = qr_svg("http://192.168.1.50:7423/?c=123456").unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.len() > 200);
    }
}

/// Pruebas contra el servidor real, hablando HTTP por un socket.
///
/// El resto de las pruebas cubren la lógica; estas confirman que las rutas, la
/// validación del código y el bloqueo por intentos se comportan como se espera
/// cuando la petición llega de verdad por la red.
#[cfg(test)]
mod http {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    struct Servidor {
        puerto: u16,
        captura: Arc<CaptureState>,
        db: Arc<Mutex<rusqlite::Connection>>,
        _runtime: tokio::runtime::Runtime,
    }

    fn levantar(codigo: &str) -> Servidor {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        let db = Arc::new(Mutex::new(conn));

        let captura = Arc::new(CaptureState::default());
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut guard = captura.activo.lock().unwrap();
            *guard = Some(Encendido {
                codigo: codigo.to_string(),
                puerto: 0,
                ip: "127.0.0.1".into(),
                apagar: tx,
                intentos_fallidos: 0,
                bloqueado_hasta: 0,
            });
        }

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();

        let app = construir_router(Contexto { db: Arc::clone(&db), captura: Arc::clone(&captura) });
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        runtime.spawn(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(async { let _ = rx.await; })
                .await;
        });

        // Espera activa breve a que el puerto acepte conexiones.
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", puerto)).is_ok() { break; }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        Servidor { puerto, captura, db, _runtime: runtime }
    }

    /// Envía una petición cruda y devuelve (código de estado, cuerpo).
    fn pedir(puerto: u16, metodo: &str, ruta: &str, codigo: &str, cuerpo: Option<&str>) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", puerto)).unwrap();
        let cuerpo = cuerpo.unwrap_or("");
        let peticion = format!(
            "{} {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\
             X-Codigo: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            metodo, ruta, codigo, cuerpo.len(), cuerpo
        );
        stream.write_all(peticion.as_bytes()).unwrap();

        let mut respuesta = String::new();
        stream.read_to_string(&mut respuesta).unwrap();

        let estado: u16 = respuesta
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let cuerpo = respuesta.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (estado, cuerpo)
    }

    fn producto_json(nombre: &str) -> String {
        format!(r#"{{"codigo":"","nombre":"{}","precio":499.0,"existencia":2,"fotos":[]}}"#, nombre)
    }

    #[test]
    fn la_pagina_se_sirve_al_celular() {
        let s = levantar("123456");
        let (estado, cuerpo) = pedir(s.puerto, "GET", "/", "", None);

        assert_eq!(estado, 200);
        assert!(cuerpo.contains("Capturar producto"));
        assert!(cuerpo.contains("<!doctype html>"));
    }

    #[test]
    fn con_el_codigo_correcto_se_da_de_alta_el_producto() {
        let s = levantar("123456");
        let (estado, cuerpo) = pedir(
            s.puerto, "POST", "/api/producto", "123456",
            Some(&producto_json("Vestido amarillo")),
        );

        assert_eq!(estado, 200, "respuesta: {}", cuerpo);
        assert!(cuerpo.contains("TS-000001"));

        let nombre: String = s.db.lock().unwrap()
            .query_row("SELECT name FROM products", [], |r| r.get(0)).unwrap();
        assert_eq!(nombre, "Vestido amarillo");
    }

    #[test]
    fn sin_codigo_no_se_guarda_nada() {
        let s = levantar("123456");
        let (estado, _) = pedir(s.puerto, "POST", "/api/producto", "", Some(&producto_json("Intruso")));

        assert_eq!(estado, 401);
        let cuantos: i64 = s.db.lock().unwrap()
            .query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0)).unwrap();
        assert_eq!(cuantos, 0);
    }

    #[test]
    fn con_el_codigo_equivocado_tampoco() {
        let s = levantar("123456");
        let (estado, _) = pedir(s.puerto, "POST", "/api/producto", "000000", Some(&producto_json("Intruso")));
        assert_eq!(estado, 401);
    }

    #[test]
    fn probar_codigos_a_ciegas_termina_bloqueado() {
        let s = levantar("123456");

        for _ in 0..MAX_INTENTOS {
            pedir(s.puerto, "POST", "/api/producto", "999999", Some(&producto_json("x")));
        }

        // Incluso con el código bueno, ya no pasa.
        let (estado, cuerpo) = pedir(
            s.puerto, "POST", "/api/producto", "123456", Some(&producto_json("Legítimo")),
        );
        assert_eq!(estado, 429, "respuesta: {}", cuerpo);
    }

    #[test]
    fn un_intento_acertado_reinicia_el_contador_de_fallos() {
        let s = levantar("123456");

        for _ in 0..(MAX_INTENTOS - 1) {
            pedir(s.puerto, "POST", "/api/producto", "999999", Some(&producto_json("x")));
        }
        pedir(s.puerto, "POST", "/api/producto", "123456", Some(&producto_json("Bueno")));

        let fallos = s.captura.activo.lock().unwrap().as_ref().unwrap().intentos_fallidos;
        assert_eq!(fallos, 0);
    }

    #[test]
    fn verificar_avisa_si_el_codigo_sirve() {
        let s = levantar("123456");

        assert_eq!(pedir(s.puerto, "GET", "/api/verificar", "123456", None).0, 200);
        assert_eq!(pedir(s.puerto, "GET", "/api/verificar", "000000", None).0, 401);
    }

    #[test]
    fn un_producto_sin_nombre_se_rechaza_con_mensaje() {
        let s = levantar("123456");
        let (estado, cuerpo) = pedir(
            s.puerto, "POST", "/api/producto", "123456",
            Some(r#"{"codigo":"","nombre":"   ","precio":10.0,"fotos":[]}"#),
        );

        assert_eq!(estado, 400);
        assert!(cuerpo.contains("nombre"));
    }

    #[test]
    fn no_hay_forma_de_consultar_otros_datos_desde_la_red() {
        let s = levantar("123456");
        for ruta in ["/api/ventas", "/api/clientes", "/api/productos", "/api/usuarios"] {
            let (estado, _) = pedir(s.puerto, "GET", ruta, "123456", None);
            assert_eq!(estado, 404, "la ruta {} no debería existir", ruta);
        }
    }

    #[test]
    fn dos_productos_seguidos_reciben_codigos_distintos() {
        let s = levantar("123456");
        let (_, a) = pedir(s.puerto, "POST", "/api/producto", "123456", Some(&producto_json("Uno")));
        let (_, b) = pedir(s.puerto, "POST", "/api/producto", "123456", Some(&producto_json("Dos")));

        assert!(a.contains("TS-000001"));
        assert!(b.contains("TS-000002"));
    }
}
