//! Recoger del relevo lo que se capturó con el celular.
//!
//! El teléfono no puede alcanzar esta computadora —el router de la tienda aísla a
//! los clientes entre sí, o están en redes distintas, y ningún permiso de
//! firewall lo cambia— pero los dos llegan a internet. Así que no se hablan
//! directo: el teléfono deja lo capturado en un buzón en Cloudflare (`relevo/`
//! en este repositorio) y el punto de venta pasa a recogerlo cada pocos minutos.
//! Solo salen conexiones; nunca entra ninguna.
//!
//! Lo que se trae se da de alta con lo mismo que usaba la red local
//! (`producto::recibir_producto`, `conteo::aplicar_conteo`), que ya reconocen lo
//! repetido por su identificador. Eso es lo que hace seguro reintentar: si la
//! computadora se apaga entre dar de alta y avisarle al buzón, la siguiente
//! vuelta lo vuelve a traer y no pasa nada.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::conteo::{aplicar_conteo, ConteoDelCelular};
use super::producto::{qr_svg, recibir_producto};
use crate::db::connection::{recuperar, DbState};
use crate::session::{require_admin, SessionState};

/// Dónde vive el relevo si no se configuró otro.
pub const URL_PREDETERMINADA: &str = "https://things-shop-relevo.derek-papa.workers.dev";

const CLAVE_URL: &str = "relevo_url";
const CLAVE_SECRETO: &str = "relevo_secreto";
const CLAVE_HUELLA_CATALOGO: &str = "relevo_catalogo_publicado";

/// Cada cuánto se pasa por el buzón.
///
/// Lo decide el cupo gratuito de KV y no el gusto. Cada vuelta hace un listado,
/// y los listados tienen cupo diario: cada minuto serían 1 440 al día. Cada tres
/// son 480, que deja margen para el botón de "traer ahora" y para el día en que
/// la aplicación se quede abierta de corrido.
pub const INTERVALO: Duration = Duration::from_secs(180);

/// Espera antes de la primera vuelta, para no competir con el arranque.
const ESPERA_INICIAL: Duration = Duration::from_secs(8);

/// Tope de una petición al relevo: para no quedarse colgado con un WiFi que se
/// fue a medias.
const TIMEOUT: Duration = Duration::from_secs(40);

/// Páginas de pendientes que se leen por vuelta. Doscientas por página: lo que
/// no alcance, sale en la siguiente vuelta.
const MAX_PAGINAS: usize = 5;

// ─── Configuración ───────────────────────────────────────────────────────────

fn leer(db: &Connection, clave: &str) -> Option<String> {
    db.query_row(
        "SELECT value FROM system_config WHERE key = ?1",
        params![clave],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
}

fn guardar(db: &Connection, clave: &str, valor: &str, descripcion: &str) -> Result<(), String> {
    db.execute(
        "INSERT OR REPLACE INTO system_config (key, value, description, updated_at)
         VALUES (?1, ?2, ?3, datetime('now','localtime'))",
        params![clave, valor, descripcion],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub(crate) fn url_del_relevo(db: &Connection) -> String {
    leer(db, CLAVE_URL)
        .unwrap_or_else(|| URL_PREDETERMINADA.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Base64 con el alfabeto de las URL y sin relleno: viaja en un enlace.
fn base64url(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut salida = String::with_capacity(bytes.len() * 4 / 3 + 2);
    for trozo in bytes.chunks(3) {
        let b = [trozo[0], *trozo.get(1).unwrap_or(&0), *trozo.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..=trozo.len() {
            salida.push(ABC[((n >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    salida
}

/// 32 bytes al azar: 43 caracteres.
///
/// No es el código de seis dígitos de la red local. Ahí el atacante tenía que
/// estar dentro de la tienda; aquí está internet entero, y seis dígitos se
/// prueban en una tarde.
fn secreto_nuevo() -> String {
    use password_hash::rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    base64url(&bytes)
}

/// El secreto de esta tienda, creándolo la primera vez que se pide.
pub(crate) fn secreto_de_la_tienda(db: &Connection) -> Result<String, String> {
    if let Some(guardado) = leer(db, CLAVE_SECRETO) {
        if guardado.len() >= 40 {
            return Ok(guardado);
        }
    }
    let nuevo = secreto_nuevo();
    guardar(db, CLAVE_SECRETO, &nuevo, "Secreto de emparejamiento con el relevo de capturas")?;
    Ok(nuevo)
}

/// El enlace que va en el QR.
///
/// El secreto va después del `#`: esa parte de una dirección nunca se manda a
/// ningún servidor, así que no aparece en registros ni se filtra por el Referer.
pub(crate) fn enlace(url: &str, secreto: &str) -> String {
    format!("{}/#k={}", url.trim_end_matches('/'), secreto)
}

// ─── Catálogo para contar ────────────────────────────────────────────────────

#[derive(Debug, Serialize, PartialEq)]
pub(crate) struct PrendaParaContar {
    sku: String,
    nombre: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    variantes: Vec<TallaParaContar>,
}

#[derive(Debug, Serialize, PartialEq)]
pub(crate) struct TallaParaContar {
    id: i64,
    etiqueta: String,
}

/// Lo que el teléfono necesita para contar: nombres, códigos y tallas.
///
/// **Sin existencias.** Lo que la caja cree que hay es inventario, y el
/// inventario no sale de la tienda. El conteo no lo necesita: quien está frente
/// al perchero ve la verdad, y la caja le suma después lo vendido mientras tanto.
pub(crate) fn catalogo_para_contar(db: &Connection) -> Result<Vec<PrendaParaContar>, String> {
    let mut stmt = db
        .prepare("SELECT id, sku, name FROM products WHERE is_active = 1 ORDER BY name ASC, id ASC LIMIT 5000")
        .map_err(|e| e.to_string())?;
    let filas: Vec<(i64, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    let mut vstmt = db
        .prepare(
            "SELECT id, size, color FROM product_variants
             WHERE product_id = ?1 AND is_active = 1 ORDER BY id",
        )
        .map_err(|e| e.to_string())?;

    let mut salida = Vec::with_capacity(filas.len());
    for (id, sku, nombre) in filas {
        let variantes = vstmt
            .query_map(params![id], |r| {
                let size: Option<String> = r.get(1)?;
                let color: Option<String> = r.get(2)?;
                Ok(TallaParaContar {
                    id: r.get(0)?,
                    etiqueta: [size, color]
                        .into_iter()
                        .flatten()
                        .map(|v| v.trim().to_string())
                        .filter(|v| !v.is_empty())
                        .collect::<Vec<_>>()
                        .join(" / "),
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        salida.push(PrendaParaContar { sku, nombre, variantes });
    }
    Ok(salida)
}

/// Huella de un texto, para saber si algo cambió desde la última vez.
///
/// No es criptográfica ni tiene que serlo: solo decide si hay que volver a
/// publicar. Si un cambio de Rust la moviera, costaría una publicación de más.
fn huella(texto: &str) -> String {
    let mut h = DefaultHasher::new();
    texto.hash(&mut h);
    format!("{:016x}", h.finish())
}

// ─── Dar de alta lo que llega ────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub(crate) enum Resultado {
    Aplicada(String),
    Rechazada { tipo: String, motivo: String },
}

#[derive(Deserialize)]
struct Captura {
    #[serde(default)]
    tipo: String,
    datos: serde_json::Value,
}

/// Da de alta una captura tal como llegó del relevo.
///
/// Un error aquí es de los datos y no del momento: el producto que se contó ya
/// no existe, falta el nombre, la foto no es una foto. Reintentar mañana daría
/// lo mismo, así que no se reintenta —se aparta (ver `guardar_rechazada`).
pub(crate) fn aplicar_captura(db: &Connection, cuerpo: &str) -> Resultado {
    let captura: Captura = match serde_json::from_str(cuerpo) {
        Ok(c) => c,
        Err(e) => {
            return Resultado::Rechazada {
                tipo: "desconocido".to_string(),
                motivo: format!("No se entiende lo que llegó: {}", e),
            }
        }
    };
    let rechazo = |motivo: String| Resultado::Rechazada { tipo: captura.tipo.clone(), motivo };

    match captura.tipo.as_str() {
        "producto" => match recibir_producto(db, captura.datos) {
            Ok(sku) => Resultado::Aplicada(format!("Producto {} dado de alta", sku)),
            Err(e) => rechazo(e),
        },
        "conteo" => match serde_json::from_value::<ConteoDelCelular>(captura.datos) {
            Ok(c) => match aplicar_conteo(db, None, &c) {
                Ok(r) => Resultado::Aplicada(format!(
                    "Conteo de {}: {} → {}",
                    r.etiqueta, r.antes, r.despues
                )),
                Err(e) => rechazo(e),
            },
            Err(e) => rechazo(format!("El conteo no tiene la forma esperada: {}", e)),
        },
        otro => rechazo(format!("Tipo de captura desconocido: {}", otro)),
    }
}

/// Aparta lo que no se pudo dar de alta, con todo su contenido.
///
/// Sale del buzón igual que lo bueno —dejarlo ahí tapaba todo lo que viniera
/// detrás, que fue exactamente el error de la cola del teléfono— pero no se
/// tira: queda completo en la base, fotos incluidas, con el motivo. Nada de lo
/// que se capturó se pierde en silencio.
fn guardar_rechazada(db: &Connection, captura_id: &str, tipo: &str, motivo: &str, contenido: &str) {
    let nueva = db
        .execute(
            "INSERT OR IGNORE INTO capturas_rechazadas (captura_id, tipo, motivo, contenido)
             VALUES (?1, ?2, ?3, ?4)",
            params![captura_id, tipo, motivo, contenido],
        )
        .unwrap_or(0);
    if nueva > 0 {
        db.execute(
            "INSERT INTO app_logs (level, module, message) VALUES ('warn', 'relevo', ?1)",
            params![format!("Captura del celular rechazada ({}): {}", tipo, motivo)],
        )
        .ok();
    }
}

// ─── Estado compartido ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize)]
pub struct EstadoRelevo {
    /// Última vuelta que terminó bien, en hora local.
    pub ultima_vez: Option<String>,
    /// Por qué falló la última vuelta, si falló.
    pub ultimo_error: Option<String>,
    /// Lo recibido y lo apartado desde que se abrió la aplicación.
    pub recibidas: u64,
    pub rechazadas: u64,
    pub trayendo: bool,
}

#[derive(Default)]
pub struct RelevoState {
    estado: Mutex<EstadoRelevo>,
    en_curso: AtomicBool,
}

impl RelevoState {
    fn estado(&self) -> EstadoRelevo {
        let mut e = self.estado.lock().unwrap_or_else(|p| p.into_inner()).clone();
        e.trayendo = self.en_curso.load(Ordering::SeqCst);
        e
    }
}

/// Una sola vuelta a la vez: el botón de "traer ahora" puede caer justo cuando
/// pasa la automática, y dos vueltas a la vez se pisarían al confirmar.
struct Turno<'a>(&'a AtomicBool);

impl<'a> Turno<'a> {
    fn tomar(relevo: &'a RelevoState) -> Option<Self> {
        relevo
            .en_curso
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Turno(&relevo.en_curso))
    }
}

impl Drop for Turno<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

// ─── La vuelta ───────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Serialize, PartialEq)]
pub struct Resumen {
    pub recibidas: u64,
    pub rechazadas: u64,
    pub catalogo_publicado: bool,
}

#[derive(Deserialize)]
struct Listado {
    #[serde(default)]
    pendientes: Vec<Pendiente>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct Pendiente {
    captura_id: String,
}

fn cliente() -> Result<reqwest::Client, String> {
    // El actualizador instala el proveedor de cifrado solo cuando le toca buscar
    // una versión, y esto puede correr antes. Sin proveedor, armar el cliente
    // revienta.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(concat!("things-shop-pos/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("No se pudo preparar la conexión: {}", e))
}

fn de_red(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "El relevo tardó demasiado en contestar".to_string()
    } else if e.is_connect() {
        "Sin internet, o el relevo no responde".to_string()
    } else {
        format!("No se pudo hablar con el relevo: {}", e)
    }
}

/// Deja pasar una respuesta buena y convierte una mala en un mensaje legible.
async fn exito(r: reqwest::Response) -> Result<reqwest::Response, String> {
    let status = r.status();
    if status.is_success() {
        return Ok(r);
    }
    let cuerpo = r.text().await.unwrap_or_default();
    let mensaje = serde_json::from_str::<serde_json::Value>(&cuerpo)
        .ok()
        .and_then(|v| v.get("mensaje").and_then(|m| m.as_str()).map(str::to_string))
        .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
    Err(if status == reqwest::StatusCode::UNAUTHORIZED {
        format!("El relevo no reconoce el código de esta tienda: {}", mensaje)
    } else {
        format!("El relevo contestó con un error: {}", mensaje)
    })
}

/// Un identificador que el relevo aceptaría. Lo que no lo sea no se pide.
fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 120
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

async fn vuelta(db: &Arc<Mutex<Connection>>) -> Result<Option<Resumen>, String> {
    // Todo lo que hace falta de la base, de una vez, y soltando el candado antes
    // de tocar la red: si la base se quedara tomada mientras el WiFi tarda, la
    // caja se congelaría a media venta.
    let (url, secreto, catalogo, huella_nueva, huella_vieja) = {
        let conn = recuperar(db.lock());
        // Sin secreto nadie ha emparejado un teléfono todavía: no hay a quién
        // preguntarle nada, y no se toca la red.
        let Some(secreto) = leer(&conn, CLAVE_SECRETO) else {
            return Ok(None);
        };
        let catalogo = serde_json::to_string(&serde_json::json!({
            "productos": catalogo_para_contar(&conn)?
        }))
        .map_err(|e| e.to_string())?;
        // El secreto entra en la huella para que cambiar de código vuelva a
        // publicar el catálogo en la carpeta nueva.
        let nueva = huella(&format!("{}\n{}", secreto, catalogo));
        (url_del_relevo(&conn), secreto, catalogo, nueva, leer(&conn, CLAVE_HUELLA_CATALOGO))
    };

    let cliente = cliente()?;
    let mut resumen = Resumen::default();

    // 1. El catálogo, solo si cambió. Publicarlo en cada vuelta se comería el
    //    cupo diario de escrituras antes de la hora de comer.
    if huella_vieja.as_deref() != Some(huella_nueva.as_str()) {
        let r = cliente
            .put(format!("{}/api/catalogo", url))
            .bearer_auth(&secreto)
            .header("content-type", "application/json")
            .body(catalogo)
            .send()
            .await
            .map_err(de_red)?;
        exito(r).await?;
        let conn = recuperar(db.lock());
        guardar(&conn, CLAVE_HUELLA_CATALOGO, &huella_nueva, "Huella del último catálogo publicado en el relevo")?;
        resumen.catalogo_publicado = true;
    }

    // 2. Qué hay por recoger.
    let mut ids: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PAGINAS {
        let base = format!("{}/api/pendientes", url);
        let direccion = match &cursor {
            Some(c) => reqwest::Url::parse_with_params(&base, &[("cursor", c.as_str())]),
            None => reqwest::Url::parse(&base),
        }
        .map_err(|e| format!("La dirección del relevo no es válida: {}", e))?;
        let r = cliente.get(direccion).bearer_auth(&secreto).send().await.map_err(de_red)?;
        let listado: Listado = exito(r).await?.json().await.map_err(de_red)?;
        ids.extend(listado.pendientes.into_iter().map(|p| p.captura_id).filter(|id| id_valido(id)));
        cursor = listado.cursor;
        if cursor.is_none() {
            break;
        }
    }

    // 3. Una por una: bajar, dar de alta, y apuntar para confirmar.
    let mut confirmar: Vec<String> = Vec::new();
    for id in ids {
        let r = cliente
            .get(format!("{}/api/pendiente/{}", url, id))
            .bearer_auth(&secreto)
            .send()
            .await
            .map_err(de_red)?;
        // El listado de KV tarda en ponerse al día: puede enseñar algo que ya
        // se borró. No es un error, simplemente ya no está.
        if r.status() == reqwest::StatusCode::NOT_FOUND {
            continue;
        }
        let cuerpo = exito(r).await?.text().await.map_err(de_red)?;

        let resultado = {
            let conn = recuperar(db.lock());
            let resultado = aplicar_captura(&conn, &cuerpo);
            match &resultado {
                Resultado::Aplicada(que) => {
                    conn.execute(
                        "INSERT INTO app_logs (level, module, message) VALUES ('info', 'relevo', ?1)",
                        params![format!("Del celular: {}", que)],
                    )
                    .ok();
                }
                Resultado::Rechazada { tipo, motivo } => {
                    guardar_rechazada(&conn, &id, tipo, motivo, &cuerpo);
                }
            }
            resultado
        };
        match resultado {
            Resultado::Aplicada(_) => resumen.recibidas += 1,
            Resultado::Rechazada { .. } => resumen.rechazadas += 1,
        }
        confirmar.push(id);
    }

    // 4. Confirmar al final y no antes: entre el buzón y la tienda, la copia que
    //    importa es la de la tienda. Si esto falla, la siguiente vuelta lo trae
    //    otra vez y el identificador impide que se duplique.
    for tanda in confirmar.chunks(200) {
        let r = cliente
            .post(format!("{}/api/recibido", url))
            .bearer_auth(&secreto)
            .json(&serde_json::json!({ "ids": tanda }))
            .send()
            .await
            .map_err(de_red)?;
        exito(r).await?;
    }

    Ok(Some(resumen))
}

/// Pasa por el buzón una vez y deja el resultado en el estado compartido.
pub async fn sincronizar(db: Arc<Mutex<Connection>>, relevo: Arc<RelevoState>) -> Result<Resumen, String> {
    let Some(_turno) = Turno::tomar(&relevo) else {
        return Err("Ya se está trayendo lo del celular".to_string());
    };

    let resultado = vuelta(&db).await;

    let mut estado = relevo.estado.lock().unwrap_or_else(|p| p.into_inner());
    match resultado {
        Ok(Some(r)) => {
            estado.ultima_vez = Some(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
            estado.ultimo_error = None;
            estado.recibidas += r.recibidas;
            estado.rechazadas += r.rechazadas;
            Ok(r)
        }
        Ok(None) => Ok(Resumen::default()),
        Err(e) => {
            estado.ultimo_error = Some(e.clone());
            Err(e)
        }
    }
}

/// Arranca las vueltas automáticas.
///
/// Sin nada que hacer en la tienda —ni botón que encender ni servidor que
/// dejar prendido—: mientras la aplicación esté abierta y haya internet, lo
/// capturado llega solo.
pub fn arrancar(db: Arc<Mutex<Connection>>, relevo: Arc<RelevoState>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(ESPERA_INICIAL).await;
        loop {
            match sincronizar(Arc::clone(&db), Arc::clone(&relevo)).await {
                Ok(r) if r.recibidas + r.rechazadas > 0 => {
                    log::info!("Relevo: {} recibidas, {} apartadas", r.recibidas, r.rechazadas)
                }
                Ok(_) => {}
                // A la bitácora de archivo y no a la base: sin internet esto
                // fallaría cada tres minutos y llenaría la tabla de nada.
                Err(e) => log::warn!("Relevo: {}", e),
            }
            tokio::time::sleep(INTERVALO).await;
        }
    });
}

// ─── Lo que ve la pantalla ───────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CapturaRechazada {
    captura_id: String,
    tipo: String,
    motivo: String,
    recibida_en: String,
}

#[derive(Debug, Serialize)]
pub struct VistaRelevo {
    /// El enlace con el secreto, que es lo que va en el QR.
    enlace: String,
    qr_svg: Option<String>,
    estado: EstadoRelevo,
    rechazadas: Vec<CapturaRechazada>,
    rechazadas_total: i64,
}

fn vista(db: &Connection, relevo: &RelevoState) -> Result<VistaRelevo, String> {
    let secreto = secreto_de_la_tienda(db)?;
    let enlace = enlace(&url_del_relevo(db), &secreto);

    let mut stmt = db
        .prepare(
            "SELECT captura_id, tipo, motivo, recibida_en FROM capturas_rechazadas
             ORDER BY id DESC LIMIT 10",
        )
        .map_err(|e| e.to_string())?;
    let rechazadas = stmt
        .query_map([], |r| {
            Ok(CapturaRechazada {
                captura_id: r.get(0)?,
                tipo: r.get(1)?,
                motivo: r.get(2)?,
                recibida_en: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let rechazadas_total: i64 = db
        .query_row("SELECT COUNT(*) FROM capturas_rechazadas", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    Ok(VistaRelevo {
        qr_svg: qr_svg(&enlace),
        enlace,
        estado: relevo.estado(),
        rechazadas,
        rechazadas_total,
    })
}

/// El QR y cómo va la recogida. Solo administradores: el enlace lleva el
/// secreto de la tienda, y con él se puede subir productos.
#[tauri::command]
pub fn relevo_estado(
    state: State<'_, DbState>,
    sessions: State<'_, SessionState>,
    relevo: State<'_, Arc<RelevoState>>,
    token: String,
) -> Result<VistaRelevo, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    vista(&db, &relevo)
}

/// Pasa por el buzón ahora, sin esperar la vuelta automática.
#[tauri::command]
pub async fn relevo_sincronizar(
    state: State<'_, DbState>,
    sessions: State<'_, SessionState>,
    relevo: State<'_, Arc<RelevoState>>,
    token: String,
) -> Result<VistaRelevo, String> {
    require_admin(&sessions, &token)?;
    {
        let db = state.conn();
        secreto_de_la_tienda(&db)?;
    }
    let rel = Arc::clone(relevo.inner());
    // Lo que falle queda en el estado, y la vista lo enseña junto a lo demás.
    let _ = sincronizar(Arc::clone(&state.db), Arc::clone(&rel)).await;
    let db = state.conn();
    vista(&db, &rel)
}

/// Emite un secreto nuevo. Los teléfonos emparejados dejan de servir hasta que
/// vuelvan a escanear el QR.
#[tauri::command]
pub async fn relevo_regenerar(
    state: State<'_, DbState>,
    sessions: State<'_, SessionState>,
    relevo: State<'_, Arc<RelevoState>>,
    token: String,
) -> Result<VistaRelevo, String> {
    require_admin(&sessions, &token)?;
    let rel = Arc::clone(relevo.inner());
    // Antes de cambiarlo se recoge lo que quedó con el viejo: después, lo que
    // estuviera en el buzón bajo el código anterior ya no se podría pedir.
    let _ = sincronizar(Arc::clone(&state.db), Arc::clone(&rel)).await;

    let db = state.conn();
    guardar(&db, CLAVE_SECRETO, &secreto_nuevo(), "Secreto de emparejamiento con el relevo de capturas")?;
    db.execute("DELETE FROM system_config WHERE key = ?1", params![CLAVE_HUELLA_CATALOGO]).ok();
    db.execute(
        "INSERT INTO app_logs (level, module, message) VALUES ('warn', 'relevo', 'Código de emparejamiento del celular cambiado')",
        [],
    )
    .ok();
    vista(&db, &rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&conn).unwrap();
        conn.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (1, 'TS-000001', 'Vestido amarillo', 200.0, 499.0, 20)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn el_secreto_mide_43_y_solo_lleva_caracteres_de_url() {
        let s = secreto_nuevo();
        assert_eq!(s.len(), 43);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{}", s);
    }

    #[test]
    fn el_secreto_alcanza_el_minimo_que_exige_el_relevo() {
        // El relevo rechaza cualquier cosa de menos de 40 caracteres.
        assert!(secreto_nuevo().len() >= 40);
    }

    #[test]
    fn dos_secretos_nunca_salen_iguales() {
        assert_ne!(secreto_nuevo(), secreto_nuevo());
    }

    #[test]
    fn base64url_coincide_con_el_estandar_cambiando_el_alfabeto() {
        assert_eq!(base64url(b""), "");
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn la_tienda_conserva_su_secreto_entre_llamadas() {
        let conn = db();
        let primero = secreto_de_la_tienda(&conn).unwrap();
        assert_eq!(secreto_de_la_tienda(&conn).unwrap(), primero);
    }

    #[test]
    fn un_secreto_guardado_demasiado_corto_se_reemplaza() {
        // Un valor viejo o tecleado a mano no puede quedarse de secreto.
        let conn = db();
        guardar(&conn, CLAVE_SECRETO, "123456", "").unwrap();
        assert!(secreto_de_la_tienda(&conn).unwrap().len() >= 40);
    }

    #[test]
    fn el_secreto_va_despues_del_gato() {
        // Lo que va después del `#` nunca llega a ningún servidor.
        let e = enlace("https://relevo.ejemplo/", "SECRETO");
        assert_eq!(e, "https://relevo.ejemplo/#k=SECRETO");
    }

    #[test]
    fn sin_configurar_se_usa_el_relevo_de_la_tienda() {
        let conn = db();
        assert_eq!(url_del_relevo(&conn), URL_PREDETERMINADA);
        guardar(&conn, CLAVE_URL, "https://otro.ejemplo///", "").unwrap();
        assert_eq!(url_del_relevo(&conn), "https://otro.ejemplo");
    }

    #[test]
    fn el_catalogo_no_lleva_existencias() {
        // Lo que la caja cree que hay es inventario, y no sale de la tienda.
        let conn = db();
        conn.execute(
            "INSERT INTO product_variants (product_id, size, color, stock) VALUES (1, 'M', 'Negro', 7)",
            [],
        )
        .unwrap();
        let json = serde_json::to_string(&catalogo_para_contar(&conn).unwrap()).unwrap();
        assert!(!json.contains("stock"), "{}", json);
        assert!(!json.contains("\"7\"") && !json.contains(":7"), "se coló una cantidad: {}", json);
        assert!(json.contains("M / Negro"));
    }

    #[test]
    fn el_catalogo_deja_fuera_lo_inactivo() {
        let conn = db();
        conn.execute(
            "INSERT INTO products (sku, name, purchase_price, sale_price, stock, is_active)
             VALUES ('TS-000002', 'Borrador', 0, 0, 0, 0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO product_variants (product_id, size, is_active) VALUES (1, 'XL', 0)", [])
            .unwrap();
        let c = catalogo_para_contar(&conn).unwrap();
        assert_eq!(c.len(), 1);
        assert!(c[0].variantes.is_empty());
    }

    #[test]
    fn la_huella_cambia_solo_cuando_cambia_el_texto() {
        assert_eq!(huella("a"), huella("a"));
        assert_ne!(huella("a"), huella("b"));
    }

    #[test]
    fn un_producto_bien_formado_se_da_de_alta() {
        let conn = db();
        let r = aplicar_captura(
            &conn,
            r#"{"tipo":"producto","datos":{"captura_id":"x1","nombre":"Blusa roja","precio":249}}"#,
        );
        assert!(matches!(r, Resultado::Aplicada(_)), "{:?}", r);
    }

    #[test]
    fn un_conteo_se_aplica() {
        let conn = db();
        let r = aplicar_captura(
            &conn,
            r#"{"tipo":"conteo","datos":{"conteo_id":"c1","sku":"TS-000001","contado":12,"contado_en":"2026-01-01 10:00:00"}}"#,
        );
        assert!(matches!(r, Resultado::Aplicada(_)), "{:?}", r);
        let stock: i32 = conn.query_row("SELECT stock FROM products WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(stock, 12);
    }

    #[test]
    fn lo_que_no_se_entiende_se_rechaza_sin_reventar() {
        let conn = db();
        for malo in [
            "esto no es json",
            r#"{"tipo":"producto","datos":{"nombre":""}}"#,
            r#"{"tipo":"conteo","datos":{"sku":"NO-EXISTE","conteo_id":"c9","contado":1,"contado_en":"x"}}"#,
            r#"{"tipo":"otra-cosa","datos":{}}"#,
        ] {
            assert!(matches!(aplicar_captura(&conn, malo), Resultado::Rechazada { .. }), "{}", malo);
        }
    }

    #[test]
    fn lo_rechazado_se_guarda_una_sola_vez_con_su_contenido() {
        let conn = db();
        guardar_rechazada(&conn, "r1", "producto", "sin nombre", "{\"x\":1}");
        guardar_rechazada(&conn, "r1", "producto", "sin nombre", "{\"x\":1}");
        let (cuantas, contenido): (i64, String) = conn
            .query_row("SELECT COUNT(*), MAX(contenido) FROM capturas_rechazadas", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(cuantas, 1, "reintentar no debe apartar dos veces lo mismo");
        assert_eq!(contenido, "{\"x\":1}");
    }

    #[test]
    fn solo_se_piden_identificadores_que_el_relevo_aceptaria() {
        assert!(id_valido("c-abc_123"));
        for malo in ["", "../otra", "con/barra", "con espacio"] {
            assert!(!id_valido(malo), "{}", malo);
        }
    }

    #[test]
    fn dos_vueltas_a_la_vez_no_pueden_empezar() {
        let relevo = RelevoState::default();
        let primera = Turno::tomar(&relevo);
        assert!(primera.is_some());
        assert!(Turno::tomar(&relevo).is_none(), "la segunda tiene que esperar");
        drop(primera);
        assert!(Turno::tomar(&relevo).is_some(), "al terminar se libera");
    }
}

/// La vuelta completa contra un relevo de mentira que habla HTTP de verdad.
///
/// Las pruebas de arriba cubren las piezas; esta cubre cómo encajan, que es donde
/// vivieron los errores de ayer: listar, bajar, dar de alta, apartar lo malo,
/// confirmar, y qué pasa si algo se cae en medio.
#[cfg(test)]
mod vuelta_completa {
    use super::*;
    use axum::extract::{Path, State as Estado};
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post, put};
    use axum::{Json, Router};
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Buzon {
        secreto: Mutex<String>,
        pendientes: Mutex<BTreeMap<String, String>>,
        catalogos: Mutex<Vec<String>>,
        /// Cuántas confirmaciones más van a fallar antes de funcionar.
        fallar_confirmaciones: Mutex<u32>,
    }

    fn autorizado(b: &Buzon, h: &HeaderMap) -> bool {
        h.get("authorization").and_then(|v| v.to_str().ok())
            == Some(format!("Bearer {}", b.secreto.lock().unwrap()).as_str())
    }

    async fn levantar(buzon: Arc<Buzon>) -> String {
        let app = Router::new()
            .route(
                "/api/catalogo",
                put(|Estado(b): Estado<Arc<Buzon>>, h: HeaderMap, cuerpo: String| async move {
                    if !autorizado(&b, &h) {
                        return StatusCode::UNAUTHORIZED;
                    }
                    b.catalogos.lock().unwrap().push(cuerpo);
                    StatusCode::OK
                }),
            )
            .route(
                "/api/pendientes",
                get(|Estado(b): Estado<Arc<Buzon>>, h: HeaderMap| async move {
                    if !autorizado(&b, &h) {
                        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "ok": false, "mensaje": "Falta el código" })));
                    }
                    let lista: Vec<_> = b
                        .pendientes
                        .lock()
                        .unwrap()
                        .keys()
                        .map(|k| serde_json::json!({ "captura_id": k }))
                        .collect();
                    (StatusCode::OK, Json(serde_json::json!({ "ok": true, "pendientes": lista, "cursor": null })))
                }),
            )
            .route(
                "/api/pendiente/:id",
                get(|Estado(b): Estado<Arc<Buzon>>, h: HeaderMap, Path(id): Path<String>| async move {
                    if !autorizado(&b, &h) {
                        return (StatusCode::UNAUTHORIZED, String::new());
                    }
                    match b.pendientes.lock().unwrap().get(&id) {
                        Some(c) => (StatusCode::OK, c.clone()),
                        None => (StatusCode::NOT_FOUND, String::new()),
                    }
                }),
            )
            .route(
                "/api/recibido",
                post(|Estado(b): Estado<Arc<Buzon>>, h: HeaderMap, Json(v): Json<serde_json::Value>| async move {
                    if !autorizado(&b, &h) {
                        return StatusCode::UNAUTHORIZED;
                    }
                    {
                        let mut fallar = b.fallar_confirmaciones.lock().unwrap();
                        if *fallar > 0 {
                            *fallar -= 1;
                            return StatusCode::INTERNAL_SERVER_ERROR;
                        }
                    }
                    let mut p = b.pendientes.lock().unwrap();
                    for id in v["ids"].as_array().unwrap() {
                        p.remove(id.as_str().unwrap());
                    }
                    StatusCode::OK
                }),
            )
            .with_state(buzon);

        let escucha = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let direccion = format!("http://{}", escucha.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(escucha, app).await.unwrap() });
        direccion
    }

    /// Una tienda con un producto, emparejada con el buzón.
    async fn tienda(buzon: &Arc<Buzon>) -> (Arc<Mutex<Connection>>, Arc<RelevoState>) {
        let url = levantar(Arc::clone(buzon)).await;
        let conn = super::tests_db();
        guardar(&conn, CLAVE_URL, &url, "").unwrap();
        *buzon.secreto.lock().unwrap() = secreto_de_la_tienda(&conn).unwrap();
        (Arc::new(Mutex::new(conn)), Arc::new(RelevoState::default()))
    }

    fn dejar(buzon: &Buzon, id: &str, cuerpo: serde_json::Value) {
        buzon.pendientes.lock().unwrap().insert(id.to_string(), cuerpo.to_string());
    }

    fn productos(db: &Arc<Mutex<Connection>>, nombre: &str) -> i64 {
        db.lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM products WHERE name = ?1", params![nombre], |r| r.get(0))
            .unwrap()
    }

    #[tokio::test]
    async fn trae_da_de_alta_aparta_lo_malo_y_vacia_el_buzon() {
        let buzon = Arc::new(Buzon::default());
        let (db, relevo) = tienda(&buzon).await;
        dejar(&buzon, "p1", serde_json::json!({ "tipo": "producto", "datos": { "captura_id": "p1", "nombre": "Falda azul", "precio": 350 } }));
        dejar(&buzon, "c1", serde_json::json!({ "tipo": "conteo", "datos": { "conteo_id": "c1", "sku": "TS-000001", "contado": 9, "contado_en": "2026-01-01 10:00:00" } }));
        dejar(&buzon, "malo", serde_json::json!({ "tipo": "producto", "datos": { "nombre": "" } }));

        let r = sincronizar(Arc::clone(&db), Arc::clone(&relevo)).await.unwrap();

        assert_eq!(r, Resumen { recibidas: 2, rechazadas: 1, catalogo_publicado: true });
        assert_eq!(productos(&db, "Falda azul"), 1);
        let stock: i32 = db.lock().unwrap().query_row("SELECT stock FROM products WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(stock, 9);
        let apartadas: i64 = db.lock().unwrap().query_row("SELECT COUNT(*) FROM capturas_rechazadas WHERE captura_id = 'malo'", [], |r| r.get(0)).unwrap();
        assert_eq!(apartadas, 1, "lo malo se aparta, no se tira");
        assert!(buzon.pendientes.lock().unwrap().is_empty(), "el buzón queda vacío, lo malo incluido");
        assert!(!buzon.catalogos.lock().unwrap()[0].contains("stock"), "el catálogo salió con existencias");
        assert_eq!(relevo.estado().recibidas, 2);
    }

    #[tokio::test]
    async fn el_catalogo_solo_se_publica_cuando_cambia() {
        // Cada publicación gasta del cupo diario de escrituras.
        let buzon = Arc::new(Buzon::default());
        let (db, relevo) = tienda(&buzon).await;

        assert!(sincronizar(Arc::clone(&db), Arc::clone(&relevo)).await.unwrap().catalogo_publicado);
        assert!(!sincronizar(Arc::clone(&db), Arc::clone(&relevo)).await.unwrap().catalogo_publicado);

        db.lock().unwrap().execute("UPDATE products SET name = 'Vestido verde' WHERE id = 1", []).unwrap();
        assert!(sincronizar(Arc::clone(&db), Arc::clone(&relevo)).await.unwrap().catalogo_publicado);
        assert_eq!(buzon.catalogos.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn si_se_cae_antes_de_confirmar_la_siguiente_vuelta_no_duplica() {
        // La transición que importa: dado de alta aquí, pero el buzón no se
        // enteró. Lo vuelve a entregar, y el identificador lo reconoce.
        let buzon = Arc::new(Buzon::default());
        let (db, relevo) = tienda(&buzon).await;
        dejar(&buzon, "p1", serde_json::json!({ "tipo": "producto", "datos": { "captura_id": "p1", "nombre": "Chamarra", "precio": 900 } }));
        *buzon.fallar_confirmaciones.lock().unwrap() = 1;

        assert!(sincronizar(Arc::clone(&db), Arc::clone(&relevo)).await.is_err());
        assert_eq!(productos(&db, "Chamarra"), 1, "ya se dio de alta aunque no se confirmó");
        assert_eq!(buzon.pendientes.lock().unwrap().len(), 1, "y sigue en el buzón");

        sincronizar(Arc::clone(&db), Arc::clone(&relevo)).await.unwrap();
        assert_eq!(productos(&db, "Chamarra"), 1, "la segunda vuelta no la duplica");
        assert!(buzon.pendientes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn un_codigo_equivocado_se_dice_y_no_toca_nada() {
        let buzon = Arc::new(Buzon::default());
        let (db, relevo) = tienda(&buzon).await;
        *buzon.secreto.lock().unwrap() = "otro-secreto".to_string();
        dejar(&buzon, "p1", serde_json::json!({ "tipo": "producto", "datos": { "captura_id": "p1", "nombre": "Ajena" } }));

        let err = sincronizar(Arc::clone(&db), Arc::clone(&relevo)).await.unwrap_err();
        assert!(err.contains("no reconoce"), "{}", err);
        assert_eq!(productos(&db, "Ajena"), 0);
        assert_eq!(relevo.estado().ultimo_error.as_deref(), Some(err.as_str()));
    }

    #[tokio::test]
    async fn sin_internet_se_dice_con_palabras() {
        let conn = super::tests_db();
        // Un puerto que nadie escucha.
        let muerto = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", muerto.local_addr().unwrap());
        drop(muerto);
        guardar(&conn, CLAVE_URL, &url, "").unwrap();
        secreto_de_la_tienda(&conn).unwrap();

        let err = sincronizar(Arc::new(Mutex::new(conn)), Arc::new(RelevoState::default())).await.unwrap_err();
        assert!(err.contains("Sin internet"), "{}", err);
    }

    #[tokio::test]
    async fn sin_telefono_emparejado_no_se_toca_la_red() {
        // Una instalación que nunca abrió el QR no tiene por qué andar
        // preguntándole nada a internet cada tres minutos.
        let conn = super::tests_db();
        guardar(&conn, CLAVE_URL, "http://127.0.0.1:9", "").unwrap();
        let r = sincronizar(Arc::new(Mutex::new(conn)), Arc::new(RelevoState::default())).await;
        assert_eq!(r, Ok(Resumen::default()));
    }
}

/// Base con un producto, compartida por los dos módulos de prueba.
#[cfg(test)]
fn tests_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    crate::db::migrations::run_migrations(&conn).unwrap();
    conn.execute(
        "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
         VALUES (1, 'TS-000001', 'Vestido amarillo', 200.0, 499.0, 20)",
        [],
    )
    .unwrap();
    conn
}
