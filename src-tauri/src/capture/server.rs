use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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
use crate::session::{require_admin, require_auth, SessionState};

/// Puerto por defecto. Alto y poco común para no chocar con nada de la tienda.
const PUERTO: u16 = 7423;
/// Puerto del ayudante que entrega el certificado.
///
/// Tiene que ir sin cifrar por el huevo y la gallina: el teléfono necesita el
/// certificado *antes* de poder confiar en la conexión cifrada. Por ahí no pasa
/// ningún dato de la tienda, solo el certificado público y las instrucciones
/// para instalarlo.
const PUERTO_AYUDA: u16 = 7424;
/// Intentos con código incorrecto antes de bloquear.
const MAX_INTENTOS: u32 = 5;
/// Cuánto dura el bloqueo.
const BLOQUEO_SEGUNDOS: i64 = 300;
/// Tope de variantes que puede generar un cruce de tallas y colores.
const MAX_VARIANTES: usize = 60;
/// Tope del cuerpo de una petición: dos fotos de catálogo más los campos.
const MAX_CUERPO: usize = 24 * 1024 * 1024;

/// Estado del servidor mientras está encendido.
struct Encendido {
    codigo: String,
    ip: String,
    /// Un solo aviso para los dos servidores: el cifrado y el del certificado.
    apagar: tokio::sync::watch::Sender<bool>,
    /// Las tareas de esos dos servidores, para poder esperar a que terminen.
    tareas: Vec<tauri::async_runtime::JoinHandle<()>>,
    intentos_fallidos: u32,
    bloqueado_hasta: i64,
}

/// Cuánto se espera, como máximo, a que los servidores suelten los puertos.
const ESPERA_APAGADO: std::time::Duration = std::time::Duration::from_secs(6);

/// Levanta el servidor que entrega el certificado y devuelve su tarea.
///
/// Está aparte para que la prueba ejercite exactamente esto y no una copia
/// parecida: el error era que este servidor se arrancaba **sin** apagado, así
/// que lo que hay que comprobar es que obedece el aviso.
fn servir_ayuda(
    escucha: std::net::TcpListener,
    ayuda: Router,
    mut aviso: tokio::sync::watch::Receiver<bool>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let escucha = match tokio::net::TcpListener::from_std(escucha) {
            Ok(l) => l,
            Err(e) => {
                log::error!("No se pudo abrir la página del certificado: {}", e);
                return;
            }
        };
        let _ = axum::serve(escucha, ayuda)
            .with_graceful_shutdown(async move {
                let _ = aviso.wait_for(|apagar| *apagar).await;
            })
            .await;
        log::info!("Página del certificado apagada");
    })
}

/// Los dos servidores encendidos y la forma de apagarlos.
struct Servidores {
    apagar: tokio::sync::watch::Sender<bool>,
    tareas: Vec<tauri::async_runtime::JoinHandle<()>>,
}

/// Levanta la captura cifrada y la página del certificado.
///
/// Está aparte de la orden de encender para poder ejercitar **esta** secuencia en
/// una prueba: los dos servidores, la señal compartida que los apaga y las tareas
/// que hay que esperar. Es donde vivía el error de que el 7424 se quedara tomado,
/// y es donde una equivocación deja la pantalla diciendo que la captura está
/// encendida mientras el teléfono se queda intentando conectar con nada — que es
/// el peor sitio posible para no tener una prueba.
fn levantar_servidores(
    escucha: std::net::TcpListener,
    escucha_ayuda: std::net::TcpListener,
    app: Router,
    ayuda: Router,
    config_tls: axum_server::tls_rustls::RustlsConfig,
) -> Servidores {
    let manija = axum_server::Handle::new();
    let (apagar_tx, apagar_rx) = tokio::sync::watch::channel(false);

    {
        let manija = manija.clone();
        let mut aviso = apagar_rx.clone();
        tauri::async_runtime::spawn(async move {
            // El mismo aviso llega a los dos servidores.
            //
            // Que el emisor se caiga sin avisar significa que el estado de la
            // captura se perdió: nadie va a poder apagarla nunca, así que se
            // apaga aquí. Se distingue del apagado normal a propósito, porque uno
            // es lo esperado y el otro es un error que hay que poder ver.
            match aviso.wait_for(|apagar| *apagar).await {
                Ok(_) => {}
                Err(_) => log::warn!(
                    "El estado de la captura se perdió sin apagarla; se apaga para no dejar el puerto tomado"
                ),
            }
            manija.graceful_shutdown(Some(std::time::Duration::from_secs(2)));
        });
    }

    let tarea_captura = {
        let manija = manija.clone();
        tauri::async_runtime::spawn(async move {
            let servidor = match axum_server::from_tcp_rustls(escucha, config_tls) {
                Ok(s) => s.handle(manija).serve(app.into_make_service()),
                Err(e) => {
                    log::error!("No se pudo iniciar la captura cifrada: {}", e);
                    return;
                }
            };
            log::info!("Captura por celular escuchando (cifrada)");
            if let Err(e) = servidor.await {
                log::error!("La captura por celular terminó con error: {}", e);
            }
            log::info!("Captura por celular apagada");
        })
    };

    // El servidor del certificado también tiene que obedecer el apagado. Sin
    // esto se quedaba escuchando el 7424 hasta cerrar la aplicación, y el
    // siguiente encendido moría ahí aunque el 7423 estuviera libre.
    let tarea_ayuda = servir_ayuda(escucha_ayuda, ayuda, apagar_rx);

    Servidores { apagar: apagar_tx, tareas: vec![tarea_captura, tarea_ayuda] }
}

/// Apaga los dos servidores y **espera** a que suelten los puertos.
///
/// La espera no es cortesía. Mientras el socket viejo siga escuchando, volver a
/// atar el mismo puerto falla: `SO_REUSEADDR` solo sirve para un puerto en
/// `TIME_WAIT`, y en Windows el socket ni lo lleva puesto. Sin esperar, apagar y
/// volver a encender la captura contestaba "No se pudo abrir el puerto 7423", y
/// regenerar el código —que apaga y enciende de corrido— dejaba la captura
/// apagada con el código ya rotado.
fn apagar_y_esperar(estado: Encendido) {
    let _ = estado.apagar.send(true);
    let tareas = estado.tareas;

    // `block_in_place` deja al hilo salir del runtime para poder esperar aquí.
    // Fuera de un runtime —que es donde corren los comandos— no hace nada.
    tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(async move {
            for tarea in tareas {
                // El tope evita quedarse colgado si un servidor se atora con una
                // conexión abierta: peor que tardar es no volver nunca.
                if tokio::time::timeout(ESPERA_APAGADO, tarea).await.is_err() {
                    log::warn!("Un servidor de la captura no terminó a tiempo; su puerto puede tardar en liberarse");
                }
            }
        })
    });
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
    /// Interfaz que se está usando, para saber si es la correcta de un vistazo.
    pub interfaz: Option<String>,
    /// Las demás direcciones de esta máquina, por si el celular no alcanza la
    /// elegida y hay que probar otra.
    pub alternativas: Vec<DireccionRed>,
}

/// Una dirección por la que el celular podría alcanzar esta máquina.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct DireccionRed {
    /// Nombre de la interfaz, para que se pueda distinguir cuál es cuál.
    pub interfaz: String,
    pub ip: String,
}

/// Interfaces que casi nunca son la red de la tienda.
///
/// Un VPN, Docker o una máquina virtual crean interfaces con direcciones
/// privadas perfectamente válidas a las que, sin embargo, el celular no llega.
fn es_interfaz_virtual(nombre: &str) -> bool {
    const PREFIJOS: [&str; 8] = ["utun", "tun", "tap", "ppp", "docker", "veth", "vmnet", "bridge"];
    let n = nombre.to_lowercase();
    PREFIJOS.iter().any(|p| n.starts_with(p))
}

/// Direcciones por las que el celular podría llegar, la mejor primero.
///
/// Preguntar "¿por dónde salgo a internet?" —el truco del socket UDP— devuelve
/// la interfaz de la ruta por defecto, que con un VPN encendido es el túnel. El
/// teléfono está en el WiFi de la tienda y a esa dirección no llega nunca, así
/// que hay que mirar todas las interfaces y quedarse con las reales.
pub fn direcciones_disponibles() -> Vec<DireccionRed> {
    let Ok(interfaces) = local_ip_address::list_afinet_netifas() else {
        return Vec::new();
    };

    let mut candidatas: Vec<(u8, DireccionRed)> = interfaces
        .into_iter()
        .filter_map(|(nombre, ip)| match ip {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() && v4.is_private() => {
                // Las interfaces físicas van primero; las virtuales quedan como
                // último recurso por si la tienda usa una configuración rara.
                let prioridad = if es_interfaz_virtual(&nombre) { 1 } else { 0 };
                Some((prioridad, DireccionRed { interfaz: nombre, ip: v4.to_string() }))
            }
            _ => None,
        })
        .collect();

    candidatas.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.interfaz.cmp(&b.1.interfaz)));
    candidatas.into_iter().map(|(_, d)| d).collect()
}

/// Dirección elegida por defecto: la primera real que se encuentre.
fn ip_local() -> Option<String> {
    direcciones_disponibles().first().map(|d| d.ip.clone())
}

/// Clave donde vive el código de emparejamiento entre arranques.
const CLAVE_CODIGO: &str = "captura_codigo";

/// Código de emparejamiento de seis dígitos.
fn nuevo_codigo() -> String {
    // uuid v4 es aleatorio criptográficamente; se toman dígitos de ahí para no
    // arrastrar otra dependencia solo por esto.
    let bytes = uuid::Uuid::new_v4();
    let n = u32::from_be_bytes(bytes.as_bytes()[..4].try_into().unwrap_or([0; 4]));
    format!("{:06}", n % 1_000_000)
}

/// El código de esta tienda: el mismo entre arranques, hasta que se pida otro.
///
/// Antes se generaba uno nuevo cada vez que se encendía la captura. Con el
/// teléfono guardando lo que captura para mandarlo horas después, eso significa
/// que todo lo capturado anoche se rechaza esta mañana y hay que volver a
/// escanear el QR con la cola llena. Lo que protege el código no es que cambie
/// seguido, sino el bloqueo tras cinco intentos fallidos: un candado de seis
/// dígitos que solo admite un intento por minuto no se abre a fuerza bruta.
///
/// Rotarlo sigue siendo posible a propósito, desde Ajustes.
fn codigo_de_la_tienda(db: &rusqlite::Connection) -> String {
    if let Ok(guardado) = db.query_row(
        "SELECT value FROM system_config WHERE key = ?1",
        params![CLAVE_CODIGO],
        |r| r.get::<_, String>(0),
    ) {
        let limpio = guardado.trim().to_string();
        if limpio.len() == 6 && limpio.chars().all(|c| c.is_ascii_digit()) {
            return limpio;
        }
    }

    let codigo = nuevo_codigo();
    db.execute(
        "INSERT OR REPLACE INTO system_config (key, value, description, updated_at)
         VALUES (?1, ?2, 'Código de emparejamiento de la captura por celular', datetime('now','localtime'))",
        params![CLAVE_CODIGO, codigo],
    )
    .ok();
    codigo
}

/// Descarta el código actual para que el siguiente encendido emita otro.
pub fn olvidar_codigo(db: &rusqlite::Connection) {
    db.execute("DELETE FROM system_config WHERE key = ?1", params![CLAVE_CODIGO]).ok();
}

/// Quita espacios, descarta vacíos y elimina repetidos conservando el orden.
fn normalizar(valores: &[String]) -> Vec<String> {
    let mut vistos = std::collections::HashSet::new();
    valores
        .iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .filter(|v| vistos.insert(v.to_lowercase()))
        .collect()
}

/// Cruza tallas con colores. Si solo hay una de las dos listas, la otra queda
/// vacía; si no hay ninguna, el producto no lleva variantes.
fn combinar(tallas: &[String], colores: &[String]) -> Vec<(Option<String>, Option<String>)> {
    match (tallas.is_empty(), colores.is_empty()) {
        (true, true) => Vec::new(),
        (false, true) => tallas.iter().map(|t| (Some(t.clone()), None)).collect(),
        (true, false) => colores.iter().map(|c| (None, Some(c.clone()))).collect(),
        (false, false) => tallas
            .iter()
            .flat_map(|t| colores.iter().map(move |c| (Some(t.clone()), Some(c.clone()))))
            .collect(),
    }
}

/// Piezas capturadas para una combinación concreta, si el teléfono las mandó.
fn piezas_de(
    piezas: &[PiezasDeVariante],
    talla: &Option<String>,
    color: &Option<String>,
) -> Option<i32> {
    let coincide = |a: &Option<String>, b: &Option<String>| match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x.trim().eq_ignore_ascii_case(y.trim()),
        _ => false,
    };
    piezas
        .iter()
        .find(|p| coincide(&p.talla, talla) && coincide(&p.color, color))
        .map(|p| p.cantidad)
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
    let svg = code
        .render()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .dark_color(svg::Color("#0a0716"))
        .light_color(svg::Color("#ffffff"))
        .build();

    // El renderizador fija ancho y alto en píxeles, así que el QR se recortaba
    // al meterlo en un recuadro más chico. Con medidas relativas y su viewBox
    // intacto, escala al espacio que tenga sin perder módulos.
    Some(escalable(&svg))
}

/// Cambia el ancho y alto absolutos del SVG por medidas relativas.
fn escalable(svg: &str) -> String {
    let mut salida = svg.to_string();
    // Solo los atributos de la etiqueta `<svg>`: buscar en todo el documento
    // podría dar con un `stroke-width` de cualquier figura de adentro.
    let Some(abre) = salida.find("<svg") else {
        return salida;
    };
    for atributo in [" width=\"", " height=\""] {
        let Some(inicio) = salida[abre..].find(atributo).map(|i| abre + i) else {
            continue;
        };
        let desde = inicio + atributo.len();
        if let Some(largo) = salida[desde..].find('"') {
            salida.replace_range(desde..desde + largo, "100%");
        }
    }
    salida
}

// ─── Lo que el celular envía ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ProductoDelCelular {
    // El código de emparejamiento se comprueba en la cabecera X-Codigo, no aquí.
    /// Identificador que el teléfono le pone al capturarlo, no al mandarlo.
    ///
    /// Es lo que hace que reintentar sea seguro: todos los envíos del mismo
    /// producto llevan el mismo, y el segundo encuentra el primero.
    #[serde(default)]
    captura_id: Option<String>,
    nombre: String,
    #[serde(default)]
    precio: Option<f64>,
    #[serde(default)]
    costo: Option<f64>,
    #[serde(default)]
    existencia: Option<i32>,
    #[serde(default)]
    notas: Option<String>,
    /// Tallas y colores elegidos. Se cruzan entre sí: tres tallas y dos colores
    /// dan seis variantes, cada una con su propia existencia.
    #[serde(default)]
    tallas: Vec<String>,
    #[serde(default)]
    colores: Vec<String>,
    /// Piezas de cada combinación, capturadas una por una desde el teléfono.
    /// Cuando llega vacío se usa `existencia` para todas.
    #[serde(default)]
    piezas: Vec<PiezasDeVariante>,
    /// Pares de foto y miniatura, ambas como data URL JPEG.
    #[serde(default)]
    fotos: Vec<FotoDelCelular>,
}

#[derive(Debug, Deserialize, Clone)]
struct PiezasDeVariante {
    #[serde(default)]
    talla: Option<String>,
    #[serde(default)]
    color: Option<String>,
    cantidad: i32,
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
pub struct Contexto {
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
        .route("/api/catalogo", get(catalogo))
        .route("/api/conteo", post(recibir_conteo))
        .merge(super::pwa::rutas())
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
    if entrada.nombre.trim().is_empty() {
        return fallo(StatusCode::BAD_REQUEST, "Ponle nombre al producto".into());
    }
    if entrada.fotos.len() > crate::commands::product_photos::MAX_POR_PRODUCTO as usize {
        return fallo(StatusCode::BAD_REQUEST, "Demasiadas fotos".into());
    }

    // Se recupera de un candado envenenado igual que los comandos: un fallo
    // anterior no debe dejar la captura muerta hasta reiniciar.
    let db = crate::db::connection::recuperar(ctx.db.lock());

    match guardar_producto(&db, entrada) {
        Ok(sku) => (
            StatusCode::OK,
            Json(Respuesta { ok: true, mensaje: "Producto guardado".into(), sku: Some(sku) }),
        ),
        Err(e) => fallo(StatusCode::BAD_REQUEST, e),
    }
}

#[derive(Debug, Serialize)]
struct ParaContar {
    sku: String,
    nombre: String,
    /// Lo que la caja cree que hay. Sirve de referencia al contar, nunca se
    /// impone: quien está frente al perchero ve la verdad.
    stock: i32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    variantes: Vec<VarianteParaContar>,
}

#[derive(Debug, Serialize)]
struct VarianteParaContar {
    id: i64,
    etiqueta: String,
    stock: i32,
}

/// El catálogo que el teléfono se lleva para poder contar sin conexión.
///
/// Solo lo indispensable para reconocer una prenda y anotar cuántas hay: ni
/// precios de compra, ni ventas, ni clientes. Si el teléfono se pierde, lo que
/// se lleva es una lista de nombres.
async fn catalogo(
    AxumState(ctx): AxumState<Contexto>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err((code, msg)) = autorizado(&ctx.captura, &headers) {
        return (code, Json(serde_json::json!({ "ok": false, "mensaje": msg })));
    }

    let db = crate::db::connection::recuperar(ctx.db.lock());
    let productos = (|| -> Result<Vec<ParaContar>, String> {
        let mut stmt = db
            .prepare(
                "SELECT id, sku, name, stock FROM products
                 WHERE is_active = 1 ORDER BY name ASC LIMIT 5000",
            )
            .map_err(|e| e.to_string())?;
        let filas: Vec<(i64, String, String, i32)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        let mut vstmt = db
            .prepare(
                "SELECT id, size, color, stock FROM product_variants
                 WHERE product_id = ?1 AND is_active = 1 ORDER BY id",
            )
            .map_err(|e| e.to_string())?;

        let mut salida = Vec::with_capacity(filas.len());
        for (id, sku, nombre, stock) in filas {
            let variantes: Vec<VarianteParaContar> = vstmt
                .query_map(params![id], |r| {
                    let size: Option<String> = r.get(1)?;
                    let color: Option<String> = r.get(2)?;
                    Ok(VarianteParaContar {
                        id: r.get(0)?,
                        etiqueta: [size, color]
                            .into_iter()
                            .flatten()
                            .filter(|v| !v.trim().is_empty())
                            .collect::<Vec<_>>()
                            .join(" / "),
                        stock: r.get(3)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?;
            salida.push(ParaContar { sku, nombre, stock, variantes });
        }
        Ok(salida)
    })();

    match productos {
        Ok(p) => (StatusCode::OK, Json(serde_json::json!({ "ok": true, "productos": p }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "mensaje": e })),
        ),
    }
}

/// Recibe un conteo hecho con el teléfono, quizá horas antes.
async fn recibir_conteo(
    AxumState(ctx): AxumState<Contexto>,
    headers: HeaderMap,
    Json(entrada): Json<super::conteo::ConteoDelCelular>,
) -> impl IntoResponse {
    if let Err((code, msg)) = autorizado(&ctx.captura, &headers) {
        return (code, Json(serde_json::json!({ "ok": false, "mensaje": msg })));
    }

    let db = crate::db::connection::recuperar(ctx.db.lock());
    match super::conteo::aplicar_conteo(&db, None, &entrada) {
        Ok(r) => {
            log::info!(
                "Conteo desde el celular: {} pasó de {} a {}",
                r.etiqueta, r.antes, r.despues
            );
            (StatusCode::OK, Json(serde_json::json!({ "ok": true, "conteo": r })))
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "mensaje": e })),
        ),
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

    // Se limpian aquí para que el cruce no genere variantes vacías ni repetidas.
    let tallas = normalizar(&entrada.tallas);
    let colores = normalizar(&entrada.colores);
    let combinaciones = combinar(&tallas, &colores);
    if combinaciones.len() > MAX_VARIANTES {
        return Err(format!(
            "Son {} combinaciones de talla y color; el máximo es {}",
            combinaciones.len(), MAX_VARIANTES
        ));
    }

    // Ya llegó antes: se devuelve el código que se le dio entonces. Es lo que
    // permite al teléfono reintentar sin miedo cuando no supo si llegó.
    let captura_id = entrada
        .captura_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    if let Some(ref id) = captura_id {
        if let Ok(sku) = db.query_row(
            "SELECT sku FROM products WHERE captura_id = ?1",
            params![id],
            |r| r.get::<_, String>(0),
        ) {
            log::info!("Captura repetida ignorada ({}), ya era {}", id, sku);
            return Ok(sku);
        }
    }

    db.execute_batch("BEGIN TRANSACTION;").map_err(|e| e.to_string())?;

    let resultado = (|| -> Result<String, String> {
        let sku = siguiente_sku(db)?;

        db.execute(
            "INSERT INTO products (sku, name, description, purchase_price, sale_price, stock, is_active, captura_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                sku,
                entrada.nombre.trim(),
                entrada.notas.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                costo,
                precio,
                existencia,
                activo,
                captura_id
            ],
        ).map_err(|e| e.to_string())?;

        let product_id = db.last_insert_rowid();

        // Las piezas se capturan por combinación. Repartir la misma cantidad
        // entre todas multiplicaba el inventario: nueve vestidos con tres
        // tallas y tres colores se convertían en ochenta y uno.
        if !combinaciones.is_empty() {
            let mut total = 0;
            for (talla, color) in &combinaciones {
                let cantidad = piezas_de(&entrada.piezas, talla, color).unwrap_or(existencia).max(0);
                total += cantidad;
                db.execute(
                    "INSERT INTO product_variants (product_id, size, color, stock)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![product_id, talla, color, cantidad],
                ).map_err(|e| e.to_string())?;
            }
            // El stock del producto es la suma de sus variantes.
            db.execute(
                "UPDATE products SET has_variants = 1, stock = ?1 WHERE id = ?2",
                params![total, product_id],
            ).map_err(|e| e.to_string())?;
        }

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
    ip_preferida: Option<String>,
) -> Result<EstadoCaptura, String> {
    require_admin(&sessions, &token)?;

    {
        let guard = captura.activo.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            drop(guard);
            return capture_server_status(sessions, captura, token);
        }
    }

    // El usuario puede forzar una dirección concreta cuando la automática no es
    // la que ve el celular.
    let ip = match ip_preferida {
        Some(elegida) if !elegida.trim().is_empty() => {
            let existe = direcciones_disponibles().iter().any(|d| d.ip == elegida);
            if !existe {
                return Err(format!("La dirección {} ya no está disponible", elegida));
            }
            elegida
        }
        _ => ip_local().ok_or(
            "No se encontró una dirección de red. Conecta este equipo al WiFi de la tienda.",
        )?,
    };
    let codigo = {
        let db = state.conn();
        codigo_de_la_tienda(&db)
    };

    let ctx = Contexto {
        db: Arc::clone(&state.db),
        captura: Arc::clone(&captura),
    };

    // El certificado se emite para esta dirección concreta, firmado por la
    // autoridad de la tienda. Si el router cambió la dirección desde la última
    // vez, este se vuelve a emitir y el teléfono no se entera: lo que tiene
    // instalado es la autoridad, no esto.
    let identidad = {
        let db = state.conn();
        super::tls::identidad_para(&db, &ip)?
    };

    let app = construir_router(ctx);
    let ayuda = super::ayuda::router(identidad.ca_pem.clone(), ip.clone());

    // Ambos puertos se abren aquí, antes de arrancar nada, para poder avisar de
    // inmediato si alguno está ocupado en vez de fallar en segundo plano.
    let escucha = std::net::TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        PUERTO,
    ))
    .map_err(|e| format!("No se pudo abrir el puerto {}: {}", PUERTO, e))?;
    // Sin esto, entregarle el socket a tokio revienta la tarea y la captura
    // nunca arranca, con la pantalla diciendo que está encendida.
    escucha.set_nonblocking(true).map_err(|e| e.to_string())?;
    let escucha_ayuda = std::net::TcpListener::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        PUERTO_AYUDA,
    ))
    .map_err(|e| format!("No se pudo abrir el puerto {}: {}", PUERTO_AYUDA, e))?;
    escucha_ayuda.set_nonblocking(true).map_err(|e| e.to_string())?;

    let config_tls = tokio::task::block_in_place(|| {
        tauri::async_runtime::block_on(axum_server::tls_rustls::RustlsConfig::from_pem(
            identidad.cert_pem.into_bytes(),
            identidad.key_pem.into_bytes(),
        ))
    })
    .map_err(|e| format!("El certificado de la captura no se pudo usar: {}", e))?;

    let servidores = levantar_servidores(escucha, escucha_ayuda, app, ayuda, config_tls);

    {
        let mut guard = captura.activo.lock().map_err(|e| e.to_string())?;
        *guard = Some(Encendido {
            codigo,
            ip,
            apagar: servidores.apagar,
            tareas: servidores.tareas,
            intentos_fallidos: 0,
            bloqueado_hasta: 0,
        });
    }

    capture_server_status(sessions, captura, token)
}

#[tauri::command]
pub fn stop_capture_server(
    sessions: State<'_, SessionState>,
    captura: State<'_, Arc<CaptureState>>,
    token: String,
) -> Result<EstadoCaptura, String> {
    require_admin(&sessions, &token)?;

    let apagado = {
        let mut guard = captura.activo.lock().map_err(|e| e.to_string())?;
        guard.take()
    };
    // Fuera del candado: esperar con él tomado bloquearía cualquier petición que
    // el celular tuviera a medias.
    if let Some(estado) = apagado {
        apagar_y_esperar(estado);
    }
    Ok(EstadoCaptura {
        encendido: false, url: None, codigo: None, qr_svg: None,
        interfaz: None, alternativas: direcciones_disponibles(),
    })
}

/// Emite un código de emparejamiento nuevo e invalida el anterior.
///
/// Existe como orden propia porque el código dejó de cambiar solo al reiniciar:
/// ahora dura para que lo capturado sin conexión pueda mandarse mañana. Cuando
/// se quiere cambiar, se quiere a propósito.
#[tauri::command]
pub fn regenerar_codigo_captura(
    state: State<'_, DbState>,
    sessions: State<'_, SessionState>,
    captura: State<'_, Arc<CaptureState>>,
    token: String,
) -> Result<EstadoCaptura, String> {
    require_admin(&sessions, &token)?;

    let estaba_encendida = {
        let guard = captura.activo.lock().map_err(|e| e.to_string())?;
        guard.as_ref().map(|e| e.ip.clone())
    };

    let apagado = {
        let mut guard = captura.activo.lock().map_err(|e| e.to_string())?;
        guard.take()
    };
    if let Some(estado) = apagado {
        apagar_y_esperar(estado);
    }
    {
        let db = state.conn();
        olvidar_codigo(&db);
    }

    match estaba_encendida {
        // Se vuelve a encender en la misma red en la que estaba.
        Some(ip) => start_capture_server(state, sessions, captura, token, Some(ip)),
        None => capture_server_status(sessions, captura, token),
    }
}

#[tauri::command]
pub fn capture_server_status(
    sessions: State<'_, SessionState>,
    captura: State<'_, Arc<CaptureState>>,
    token: String,
) -> Result<EstadoCaptura, String> {
    // El estado incluye la dirección y el código de emparejamiento: sin sesión
    // no tiene por qué verlos nadie, igual que para encender o apagar.
    require_auth(&sessions, &token)?;

    let guard = captura.activo.lock().map_err(|e| e.to_string())?;
    // Enumerar las interfaces del equipo no es gratis y el botón de Productos
    // consulta esto cada pocos segundos: se hace una sola vez.
    let alternativas = direcciones_disponibles();
    match guard.as_ref() {
        Some(e) => {
            // El QR lleva a la página del permiso, no directo a la captura: un
            // teléfono sin el permiso instalado no puede abrir la dirección
            // cifrada, y quien ya lo tiene solo toca el último botón de esa
            // página, que lleva el código consigo.
            let url = format!("http://{}:{}/?c={}", e.ip, PUERTO_AYUDA, e.codigo);
            let interfaz = alternativas
                .iter()
                .find(|d| d.ip == e.ip)
                .map(|d| d.interfaz.clone());
            Ok(EstadoCaptura {
                encendido: true,
                qr_svg: qr_svg(&url),
                url: Some(url),
                codigo: Some(e.codigo.clone()),
                interfaz,
                alternativas,
            })
        }
        None => Ok(EstadoCaptura {
            encendido: false, url: None, codigo: None, qr_svg: None,
            interfaz: None, alternativas,
        }),
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
            captura_id: None,
            nombre: nombre.into(),
            precio,
            costo: None,
            existencia: Some(3),
            notas: None,
            tallas: vec![],
            colores: vec![],
            piezas: vec![],
            fotos: (0..fotos)
                .map(|_| FotoDelCelular { photo: jpeg(), thumbnail: jpeg() })
                .collect(),
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
    }

    #[test]
    fn sin_precio_el_producto_queda_inactivo_para_no_venderse_a_medias() {
        let conn = db();
        let sku = guardar_producto(&conn, entrada("Blusa sin precio", None, 1)).unwrap();

        let activo: i32 = conn.query_row(
            "SELECT is_active FROM products WHERE sku = ?1", params![sku], |r| r.get(0)).unwrap();
        assert_eq!(activo, 0);
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
    fn las_tallas_y_colores_se_cruzan_en_variantes() {
        let conn = db();
        let mut e = entrada("Vestido", Some(499.0), 0);
        e.tallas = vec!["S".into(), "M".into(), "L".into()];
        e.colores = vec!["Rojo".into(), "Azul".into()];
        e.existencia = Some(2);

        let sku = guardar_producto(&conn, e).unwrap();

        let variantes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM product_variants", [], |r| r.get(0)).unwrap();
        assert_eq!(variantes, 6, "tres tallas por dos colores");

        // El stock del producto debe ser la suma de sus variantes.
        let (stock, tiene): (i32, i32) = conn.query_row(
            "SELECT stock, has_variants FROM products WHERE sku = ?1",
            params![sku], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(stock, 12);
        assert_eq!(tiene, 1);
    }

    #[test]
    fn mandar_dos_veces_la_misma_captura_no_crea_dos_productos() {
        // El teléfono guarda lo capturado y lo manda cuando alcanza la caja. Si
        // el WiFi se cae entre "lo mandé" y "me contestaron", va a reintentar; y
        // quien esté esperando también le va a dar otra vez al botón.
        let conn = db();
        let mut e = entrada("Vestido amarillo", Some(499.0), 0);
        e.captura_id = Some("abc-123".into());

        let primero = guardar_producto(&conn, e).unwrap();

        let mut otra_vez = entrada("Vestido amarillo", Some(499.0), 0);
        otra_vez.captura_id = Some("abc-123".into());
        let segundo = guardar_producto(&conn, otra_vez).unwrap();

        assert_eq!(primero, segundo, "el reintento devuelve el mismo código");
        let cuantos: i64 = conn
            .query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cuantos, 1, "y no da de alta el vestido dos veces");
    }

    #[test]
    fn dos_capturas_distintas_siguen_siendo_dos_productos() {
        // Lo contrario también importa: dos prendas iguales capturadas a
        // propósito son dos prendas.
        let conn = db();
        let mut a = entrada("Blusa", Some(299.0), 0);
        a.captura_id = Some("uno".into());
        let mut b = entrada("Blusa", Some(299.0), 0);
        b.captura_id = Some("dos".into());

        let sku_a = guardar_producto(&conn, a).unwrap();
        let sku_b = guardar_producto(&conn, b).unwrap();

        assert_ne!(sku_a, sku_b);
    }

    #[test]
    fn una_captura_sin_identificador_se_acepta_igual() {
        // La app de escritorio y las versiones viejas del teléfono no lo mandan.
        let conn = db();
        let sku = guardar_producto(&conn, entrada("Falda", Some(350.0), 0)).unwrap();
        assert_eq!(sku, "TS-000001");
    }

    #[test]
    fn las_piezas_se_capturan_por_combinacion_no_se_multiplican() {
        // Nueve vestidos en tres tallas no son veintisiete: son nueve
        // repartidos. Antes se ponían nueve de cada una.
        let conn = db();
        let mut e = entrada("Vestido", Some(499.0), 0);
        e.tallas = vec!["CH".into(), "M".into(), "G".into()];
        e.existencia = Some(9);
        e.piezas = vec![
            PiezasDeVariante { talla: Some("CH".into()), color: None, cantidad: 2 },
            PiezasDeVariante { talla: Some("M".into()), color: None, cantidad: 4 },
            PiezasDeVariante { talla: Some("G".into()), color: None, cantidad: 3 },
        ];

        let sku = guardar_producto(&conn, e).unwrap();

        let stock: i32 = conn.query_row(
            "SELECT stock FROM products WHERE sku = ?1", params![sku], |r| r.get(0)).unwrap();
        assert_eq!(stock, 9, "el total es la suma de lo capturado, no un múltiplo");

        let de_mediana: i32 = conn.query_row(
            "SELECT stock FROM product_variants WHERE size = 'M'", [], |r| r.get(0)).unwrap();
        assert_eq!(de_mediana, 4);
    }

    #[test]
    fn las_piezas_por_talla_y_color_se_respetan_una_a_una() {
        let conn = db();
        let mut e = entrada("Blusa", Some(299.0), 0);
        e.tallas = vec!["CH".into(), "G".into()];
        e.colores = vec!["Rojo".into(), "Azul".into()];
        e.piezas = vec![
            PiezasDeVariante { talla: Some("CH".into()), color: Some("Rojo".into()), cantidad: 1 },
            PiezasDeVariante { talla: Some("CH".into()), color: Some("Azul".into()), cantidad: 2 },
            PiezasDeVariante { talla: Some("G".into()), color: Some("Rojo".into()), cantidad: 3 },
            PiezasDeVariante { talla: Some("G".into()), color: Some("Azul".into()), cantidad: 4 },
        ];

        let sku = guardar_producto(&conn, e).unwrap();

        let stock: i32 = conn.query_row(
            "SELECT stock FROM products WHERE sku = ?1", params![sku], |r| r.get(0)).unwrap();
        assert_eq!(stock, 10);

        let g_azul: i32 = conn.query_row(
            "SELECT stock FROM product_variants WHERE size = 'G' AND color = 'Azul'",
            [], |r| r.get(0)).unwrap();
        assert_eq!(g_azul, 4);
    }

    #[test]
    fn una_sola_talla_usa_la_cantidad_general_sin_preguntar() {
        // Con una sola combinación no hay nada que repartir.
        let conn = db();
        let mut e = entrada("Bufanda", Some(99.0), 0);
        e.tallas = vec!["Unitalla".into()];
        e.existencia = Some(6);

        let sku = guardar_producto(&conn, e).unwrap();

        let stock: i32 = conn.query_row(
            "SELECT stock FROM products WHERE sku = ?1", params![sku], |r| r.get(0)).unwrap();
        assert_eq!(stock, 6);
    }

    #[test]
    fn una_combinacion_sin_piezas_capturadas_usa_la_cantidad_general() {
        let conn = db();
        let mut e = entrada("Playera", Some(199.0), 0);
        e.tallas = vec!["CH".into(), "G".into()];
        e.existencia = Some(5);
        e.piezas = vec![
            PiezasDeVariante { talla: Some("CH".into()), color: None, cantidad: 2 },
        ];

        guardar_producto(&conn, e).unwrap();

        let g: i32 = conn.query_row(
            "SELECT stock FROM product_variants WHERE size = 'G'", [], |r| r.get(0)).unwrap();
        assert_eq!(g, 5, "lo que no se capturó cae en la cantidad general");
    }

    #[test]
    fn las_piezas_se_emparejan_sin_importar_mayusculas_ni_espacios() {
        let conn = db();
        let mut e = entrada("Falda", Some(350.0), 0);
        e.tallas = vec!["M".into()];
        e.colores = vec!["Rojo".into()];
        e.piezas = vec![
            PiezasDeVariante { talla: Some(" m ".into()), color: Some("ROJO".into()), cantidad: 7 },
        ];

        guardar_producto(&conn, e).unwrap();

        let stock: i32 = conn.query_row("SELECT stock FROM product_variants", [], |r| r.get(0)).unwrap();
        assert_eq!(stock, 7);
    }

    #[test]
    fn una_cantidad_negativa_por_variante_se_trata_como_cero() {
        let conn = db();
        let mut e = entrada("Rara", Some(10.0), 0);
        e.tallas = vec!["CH".into()];
        e.piezas = vec![
            PiezasDeVariante { talla: Some("CH".into()), color: None, cantidad: -5 },
        ];

        guardar_producto(&conn, e).unwrap();

        let stock: i32 = conn.query_row("SELECT stock FROM product_variants", [], |r| r.get(0)).unwrap();
        assert_eq!(stock, 0);
    }

    #[test]
    fn solo_tallas_no_inventa_colores() {
        let conn = db();
        let mut e = entrada("Playera", Some(199.0), 0);
        e.tallas = vec!["CH".into(), "G".into()];

        guardar_producto(&conn, e).unwrap();

        let variantes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM product_variants WHERE color IS NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(variantes, 2);
    }

    #[test]
    fn solo_colores_tampoco_inventa_tallas() {
        let conn = db();
        let mut e = entrada("Bufanda", Some(99.0), 0);
        e.colores = vec!["Negro".into()];

        guardar_producto(&conn, e).unwrap();

        let variantes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM product_variants WHERE size IS NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(variantes, 1);
    }

    #[test]
    fn sin_tallas_ni_colores_el_producto_no_lleva_variantes() {
        let conn = db();
        let mut e = entrada("Cinturón", Some(150.0), 0);
        e.existencia = Some(7);

        let sku = guardar_producto(&conn, e).unwrap();

        let (stock, tiene): (i32, i32) = conn.query_row(
            "SELECT stock, has_variants FROM products WHERE sku = ?1",
            params![sku], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(stock, 7);
        assert_eq!(tiene, 0);
    }

    #[test]
    fn las_tallas_repetidas_o_vacias_se_descartan() {
        let conn = db();
        let mut e = entrada("Blusa", Some(299.0), 0);
        e.tallas = vec!["M".into(), " m ".into(), "".into(), "  ".into(), "G".into()];

        guardar_producto(&conn, e).unwrap();

        let variantes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM product_variants", [], |r| r.get(0)).unwrap();
        assert_eq!(variantes, 2, "'M' y ' m ' son la misma talla");
    }

    #[test]
    fn un_cruce_absurdo_de_tallas_y_colores_se_rechaza() {
        let conn = db();
        let mut e = entrada("Imposible", Some(10.0), 0);
        e.tallas = (0..20).map(|i| format!("T{}", i)).collect();
        e.colores = (0..20).map(|i| format!("C{}", i)).collect();

        let err = guardar_producto(&conn, e).unwrap_err();
        assert!(err.contains("combinaciones"), "mensaje poco claro: {}", err);

        let productos: i64 = conn.query_row("SELECT COUNT(*) FROM products", [], |r| r.get(0)).unwrap();
        assert_eq!(productos, 0, "no debe quedar el producto sin sus variantes");
    }

    #[test]
    fn normalizar_conserva_el_orden_en_que_se_capturaron() {
        let v = normalizar(&["L".into(), "S".into(), "M".into(), "s".into()]);
        assert_eq!(v, vec!["L", "S", "M"]);
    }

    #[test]
    fn las_interfaces_de_vpn_y_contenedores_se_reconocen() {
        // Con un VPN encendido, su túnel tiene una IP privada válida a la que el
        // celular no llega nunca; no puede ser la que aparece en el QR.
        for virtual_ in ["utun4", "utun0", "tun0", "tap0", "ppp0", "docker0", "vmnet1", "bridge100"] {
            assert!(es_interfaz_virtual(virtual_), "{} debería tratarse como virtual", virtual_);
        }
        for real in ["en0", "en1", "eth0", "wlan0", "Wi-Fi", "Ethernet"] {
            assert!(!es_interfaz_virtual(real), "{} es una interfaz real", real);
        }
    }

    #[test]
    fn la_deteccion_de_mayusculas_no_deja_pasar_un_tunel() {
        assert!(es_interfaz_virtual("UTUN4"));
        assert!(es_interfaz_virtual("Docker0"));
    }

    #[test]
    fn solo_se_ofrecen_direcciones_privadas_alcanzables() {
        // No se puede fijar qué interfaces tiene la máquina que corre la prueba,
        // pero sí que nada de lo ofrecido sea inservible para un celular.
        for d in direcciones_disponibles() {
            let ip: std::net::Ipv4Addr = d.ip.parse().expect("debe ser IPv4");
            assert!(ip.is_private(), "{} no es una dirección de red local", d.ip);
            assert!(!ip.is_loopback(), "{} es loopback", d.ip);
            assert!(!ip.is_link_local(), "{} es autoasignada", d.ip);
            assert!(!d.interfaz.is_empty());
        }
    }

    #[test]
    fn las_interfaces_reales_se_ofrecen_antes_que_los_tuneles() {
        let lista = direcciones_disponibles();
        let primer_virtual = lista.iter().position(|d| es_interfaz_virtual(&d.interfaz));
        let ultima_real = lista.iter().rposition(|d| !es_interfaz_virtual(&d.interfaz));

        if let (Some(v), Some(r)) = (primer_virtual, ultima_real) {
            assert!(r < v, "una interfaz virtual quedó antes que una real: {:?}", lista);
        }
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

    #[test]
    fn el_qr_escala_a_su_contenedor_en_vez_de_recortarse() {
        // Con ancho y alto en píxeles, el código se cortaba dentro de un
        // recuadro más chico y dejaba de poder escanearse.
        let svg = qr_svg("http://192.168.1.50:7423/?c=123456").unwrap();
        assert!(svg.contains(r#"width="100%""#), "falta el ancho relativo: {}", &svg[..120]);
        assert!(svg.contains(r#"height="100%""#), "falta el alto relativo");
        assert!(svg.contains("viewBox"), "sin viewBox no puede escalar");
    }

    #[test]
    fn escalable_no_confunde_el_ancho_del_svg_con_el_de_una_figura() {
        // Buscar "width=" en todo el documento daba con el trazo de adentro y
        // dejaba el SVG con su tamaño fijo, es decir recortado otra vez.
        let original = r#"<svg width="220" height="220" viewBox="0 0 9 9"><path stroke-width="2"/></svg>"#;
        let resultado = escalable(original);
        assert!(resultado.contains(r#"<svg width="100%" height="100%""#));
        assert!(resultado.contains(r#"stroke-width="2""#), "no debe tocar la figura");
    }

    #[test]
    fn escalable_no_toca_el_resto_del_svg() {
        let original = r#"<svg width="220" height="220" viewBox="0 0 25 25"><rect x="1" y="2"/></svg>"#;
        let resultado = escalable(original);
        assert!(resultado.contains(r#"viewBox="0 0 25 25""#));
        assert!(resultado.contains(r#"<rect x="1" y="2"/>"#));
    }
}

/// Apagado: que los puertos queden libres para el siguiente encendido.
///
/// Esta es la parte que no se podía probar desde `levantar`, que monta su propio
/// servidor con su propio apagado. El error vivía justo aquí: el servidor del
/// certificado se arrancaba sin apagado ninguno, así que el 7424 se quedaba
/// tomado hasta cerrar la aplicación y el siguiente encendido moría ahí.
#[cfg(test)]
mod apagado {
    use super::*;

    fn puerto_libre() -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    }

    fn ocupado(puerto: u16) -> bool {
        std::net::TcpListener::bind(("0.0.0.0", puerto)).is_err()
    }

    /// Levanta dos servidores del certificado con `servir_ayuda`, que es la
    /// función que usa el encendido de verdad.
    fn dos_servidores(a: u16, b: u16)
        -> (tokio::sync::watch::Sender<bool>, Vec<tauri::async_runtime::JoinHandle<()>>)
    {
        let (tx, rx) = tokio::sync::watch::channel(false);
        let mut tareas = Vec::new();
        for puerto in [a, b] {
            let escucha = std::net::TcpListener::bind(("0.0.0.0", puerto)).unwrap();
            escucha.set_nonblocking(true).unwrap();
            let app: Router = Router::new().route("/", get(|| async { "ok" }));
            tareas.push(servir_ayuda(escucha, app, rx.clone()));
        }
        // Espera breve a que los dos estén escuchando.
        for _ in 0..200 {
            if ocupado(a) && ocupado(b) { break; }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        (tx, tareas)
    }

    /// Lo mismo que hace `apagar_y_esperar`, sobre las tareas sueltas.
    fn apagar(tx: tokio::sync::watch::Sender<bool>, tareas: Vec<tauri::async_runtime::JoinHandle<()>>) -> usize {
        let _ = tx.send(true);
        tokio::task::block_in_place(|| {
            tauri::async_runtime::block_on(async move {
                let mut terminaron = 0;
                for t in tareas {
                    if tokio::time::timeout(ESPERA_APAGADO, t).await.is_ok() { terminaron += 1; }
                }
                terminaron
            })
        })
    }

    #[test]
    fn apagar_libera_los_puertos_antes_de_volver() {
        let (a, b) = (puerto_libre(), puerto_libre());
        let (tx, tareas) = dos_servidores(a, b);
        assert!(ocupado(a) && ocupado(b), "los dos deben estar escuchando");

        apagar(tx, tareas);

        // El punto: al volver de la espera, los puertos ya se pueden reatar. Sin
        // la espera esto fallaba por carrera, y con este servidor arrancado sin
        // apagado fallaba siempre, por más que se esperara.
        assert!(!ocupado(a), "el primer puerto sigue tomado");
        assert!(!ocupado(b), "el segundo puerto sigue tomado");
    }

    #[test]
    fn volver_a_encender_en_los_mismos_puertos_funciona() {
        // El ciclo completo: apagar y encender de corrido, que es lo que hace
        // regenerar el código de emparejamiento.
        let (a, b) = (puerto_libre(), puerto_libre());

        let (tx, tareas) = dos_servidores(a, b);
        apagar(tx, tareas);

        let (tx2, tareas2) = dos_servidores(a, b);
        assert!(ocupado(a) && ocupado(b), "el segundo encendido no tomó los puertos");
        apagar(tx2, tareas2);
    }

    #[test]
    fn una_sola_senal_apaga_a_los_dos() {
        // El comentario del código lo prometía y no era verdad: la señal solo
        // llegaba al servidor cifrado.
        let (a, b) = (puerto_libre(), puerto_libre());
        let (tx, tareas) = dos_servidores(a, b);

        assert_eq!(apagar(tx, tareas), 2, "los dos deben terminar con un solo aviso");
    }
}

/// El encendido completo, por el camino de verdad.
///
/// Aquí no se imita nada: se llama a `levantar_servidores` con el certificado que
/// emite la tienda y se habla con los dos puertos como hablaría el teléfono —TLS
/// real en la captura, HTTP plano en la página del permiso—. Es la prueba que
/// faltaba: todo lo demás verificaba las piezas por separado, y los tres errores
/// de este servidor vivieron justo en cómo encajan.
#[cfg(test)]
mod encendido {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_rustls::rustls::pki_types::ServerName;
    use tokio_rustls::rustls::{ClientConfig, RootCertStore};
    use tokio_rustls::TlsConnector;

    fn base() -> Arc<Mutex<rusqlite::Connection>> {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        Arc::new(Mutex::new(conn))
    }

    fn escuchar() -> (std::net::TcpListener, u16) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = l.local_addr().unwrap().port();
        l.set_nonblocking(true).unwrap();
        (l, puerto)
    }

    /// Un teléfono con la autoridad de la tienda instalada.
    fn telefono(ca_pem: &str) -> ClientConfig {
        let mut raiz = RootCertStore::empty();
        let mut cursor = std::io::Cursor::new(ca_pem.as_bytes());
        for cert in rustls_pemfile::certs(&mut cursor) {
            raiz.add(cert.unwrap()).unwrap();
        }
        ClientConfig::builder().with_root_certificates(raiz).with_no_client_auth()
    }

    async fn pedir_cifrado(puerto: u16, ca_pem: &str, ruta: &str, codigo: &str) -> String {
        let conector = TlsConnector::from(Arc::new(telefono(ca_pem)));
        let tcp = tokio::net::TcpStream::connect(("127.0.0.1", puerto)).await
            .expect("no se pudo abrir el socket de la captura");
        let mut flujo = conector
            .connect(ServerName::try_from("127.0.0.1").unwrap(), tcp)
            .await
            .expect("el teléfono no confió en la caja");
        let peticion = format!(
            "GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\nX-Codigo: {}\r\n\r\n",
            ruta, codigo
        );
        flujo.write_all(peticion.as_bytes()).await.unwrap();
        let mut buf = Vec::new();
        flujo.read_to_end(&mut buf).await.ok();
        String::from_utf8_lossy(&buf).to_string()
    }

    async fn pedir_plano(puerto: u16) -> String {
        let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", puerto)).await
            .expect("no se pudo abrir el socket del permiso");
        tcp.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .await.unwrap();
        let mut buf = Vec::new();
        tcp.read_to_end(&mut buf).await.ok();
        String::from_utf8_lossy(&buf).to_string()
    }

    /// Enciende como lo hace la aplicación y devuelve con qué hablarle.
    async fn encender() -> (Servidores, u16, u16, String, String) {
        let db = base();
        let (identidad, codigo) = {
            let conn = db.lock().unwrap();
            (super::super::tls::identidad_para(&conn, "127.0.0.1").unwrap(),
             codigo_de_la_tienda(&conn))
        };
        let ca = identidad.ca_pem.clone();

        let captura = Arc::new(CaptureState::default());
        let app = construir_router(Contexto { db: Arc::clone(&db), captura: Arc::clone(&captura) });
        let ayuda = super::super::ayuda::router(identidad.ca_pem.clone(), "127.0.0.1".to_string());

        let config = axum_server::tls_rustls::RustlsConfig::from_pem(
            identidad.cert_pem.into_bytes(),
            identidad.key_pem.into_bytes(),
        ).await.unwrap();

        let (l1, p1) = escuchar();
        let (l2, p2) = escuchar();
        let servidores = levantar_servidores(l1, l2, app, ayuda, config);

        // El estado tiene que quedar encendido o `autorizado` rechaza todo.
        {
            let (tx_falso, _rx) = tokio::sync::watch::channel(false);
            let mut guard = captura.activo.lock().unwrap();
            *guard = Some(Encendido {
                codigo: codigo.clone(), ip: "127.0.0.1".into(),
                apagar: tx_falso, tareas: Vec::new(),
                intentos_fallidos: 0, bloqueado_hasta: 0,
            });
        }

        // Espera a que los dos acepten conexiones.
        for _ in 0..200 {
            if std::net::TcpStream::connect(("127.0.0.1", p1)).is_ok()
                && std::net::TcpStream::connect(("127.0.0.1", p2)).is_ok() { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        (servidores, p1, p2, ca, codigo)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn los_dos_puertos_sirven_de_verdad() {
        // Lo que el teléfono hace al escanear el QR: primero la página del
        // permiso, sin cifrar; después la captura, cifrada.
        let (servidores, puerto_captura, puerto_ayuda, ca, codigo) = encender().await;

        let permiso = pedir_plano(puerto_ayuda).await;
        assert!(permiso.contains("200 OK"), "el permiso no contestó: {}", &permiso[..permiso.len().min(200)]);
        assert!(permiso.contains("Preparar el teléfono"));

        let pagina = pedir_cifrado(puerto_captura, &ca, "/", &codigo).await;
        assert!(pagina.contains("200 OK"), "la captura no contestó: {}", &pagina[..pagina.len().min(200)]);
        assert!(pagina.contains("Capturar producto"));

        let _ = servidores.apagar.send(true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn siguen_sirviendo_un_rato_despues_de_encender() {
        // El apagado lo dispara una señal compartida. Una equivocación ahí apaga
        // los servidores solos justo después de arrancar, y la pantalla sigue
        // diciendo que la captura está encendida mientras el teléfono se queda
        // intentando conectar con nada.
        let (servidores, puerto_captura, puerto_ayuda, ca, codigo) = encender().await;

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        assert!(pedir_plano(puerto_ayuda).await.contains("200 OK"), "el permiso se apagó solo");
        assert!(pedir_cifrado(puerto_captura, &ca, "/", &codigo).await.contains("200 OK"),
                "la captura se apagó sola");

        let _ = servidores.apagar.send(true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn el_service_worker_se_sirve_por_el_puerto_cifrado() {
        // Sin esto no hay guardado en el teléfono: los service workers solo corren
        // en un origen seguro, así que tiene que salir por el 7423 y no por el
        // 7424, que va sin cifrar.
        let (servidores, puerto_captura, _p, ca, codigo) = encender().await;

        let sw = pedir_cifrado(puerto_captura, &ca, "/sw.js", &codigo).await;
        assert!(sw.contains("200 OK"), "no se sirvió el service worker");
        assert!(sw.contains("things-shop-captura-v"), "no es el service worker");

        let manifiesto = pedir_cifrado(puerto_captura, &ca, "/manifest.webmanifest", &codigo).await;
        assert!(manifiesto.contains("200 OK"), "no se sirvió el manifiesto");
        assert!(manifiesto.contains("standalone"));

        let _ = servidores.apagar.send(true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn el_apagado_libera_los_dos_puertos() {
        let (servidores, p1, p2, _ca, _c) = encender().await;

        let _ = servidores.apagar.send(true);
        for tarea in servidores.tareas {
            let _ = tokio::time::timeout(ESPERA_APAGADO, tarea).await;
        }

        assert!(std::net::TcpListener::bind(("127.0.0.1", p1)).is_ok(), "el 7423 sigue tomado");
        assert!(std::net::TcpListener::bind(("127.0.0.1", p2)).is_ok(), "el 7424 sigue tomado");
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
        let (tx, mut rx) = tokio::sync::watch::channel(false);
        {
            let mut guard = captura.activo.lock().unwrap();
            *guard = Some(Encendido {
                codigo: codigo.to_string(),
                ip: "127.0.0.1".into(),
                apagar: tx,
                tareas: Vec::new(),
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
                .with_graceful_shutdown(async move { let _ = rx.wait_for(|a| *a).await; })
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
    fn la_pagina_ofrece_camara_y_galeria_por_separado() {
        // Un solo botón deja al teléfono decidir, y decide distinto en cada
        // modelo; la encargada necesita poder elegir.
        let s = levantar("123456");
        let (_, cuerpo) = pedir(s.puerto, "GET", "/", "", None);

        assert!(cuerpo.contains("Tomar foto"));
        assert!(cuerpo.contains("De la galería"));
        assert!(cuerpo.contains(r#"capture="environment""#), "falta el atributo que abre la cámara");
    }

    #[test]
    fn la_pagina_permite_capturar_tallas_y_colores() {
        let s = levantar("123456");
        let (_, cuerpo) = pedir(s.puerto, "GET", "/", "", None);

        assert!(cuerpo.contains("Tallas"));
        assert!(cuerpo.contains("Colores"));
    }

    #[test]
    fn un_producto_con_tallas_llega_con_sus_variantes() {
        let s = levantar("123456");
        let cuerpo = r#"{"codigo":"","nombre":"Vestido amarillo","precio":499.0,
            "existencia":2,"tallas":["S","M","L"],"colores":["Amarillo"],"fotos":[]}"#;

        let (estado, resp) = pedir(s.puerto, "POST", "/api/producto", "123456", Some(cuerpo));
        assert_eq!(estado, 200, "respuesta: {}", resp);

        let db = s.db.lock().unwrap();
        let variantes: i64 = db.query_row(
            "SELECT COUNT(*) FROM product_variants", [], |r| r.get(0)).unwrap();
        assert_eq!(variantes, 3);

        let stock: i32 = db.query_row("SELECT stock FROM products", [], |r| r.get(0)).unwrap();
        assert_eq!(stock, 6, "dos piezas por cada una de las tres tallas");
    }

    #[test]
    fn un_producto_sin_precio_llega_como_borrador() {
        let s = levantar("123456");
        let cuerpo = r#"{"codigo":"","nombre":"Falta precio","existencia":1,"fotos":[]}"#;

        let (estado, _) = pedir(s.puerto, "POST", "/api/producto", "123456", Some(cuerpo));
        assert_eq!(estado, 200);

        let activo: i32 = s.db.lock().unwrap()
            .query_row("SELECT is_active FROM products", [], |r| r.get(0)).unwrap();
        assert_eq!(activo, 0);
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
