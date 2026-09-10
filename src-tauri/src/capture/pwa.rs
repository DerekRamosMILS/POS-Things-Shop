//! Lo que convierte la página del celular en una aplicación que vive ahí.
//!
//! El manifiesto la deja instalarse en la pantalla de inicio; el *service
//! worker* la guarda para que abra aunque la computadora esté apagada. Sin el
//! segundo, "capturar mientras la caja está apagada" no existe: el teléfono ni
//! siquiera podría cargar la página.
//!
//! El guardado es deliberadamente tonto y por eso es difícil de romper: la
//! página entera es un solo archivo, se guarda ese archivo, y todo lo que sea
//! `/api/` nunca se guarda. Una respuesta de la caja cacheada por error sería
//! mucho peor que no tener nada.

use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

const ICONO_192: &[u8] = include_bytes!("iconos/icono-192.png");
const ICONO_512: &[u8] = include_bytes!("iconos/icono-512.png");
const ICONO_180: &[u8] = include_bytes!("iconos/icono-180.png");

/// Sube cuando cambia la página. El service worker viejo se da cuenta, guarda
/// la nueva y la sirve al siguiente arranque.
const VERSION: &str = "5";

const MANIFIESTO: &str = r##"{
  "name": "Capturar productos - Things Shop",
  "short_name": "Capturar",
  "start_url": "/",
  "scope": "/",
  "display": "standalone",
  "orientation": "portrait",
  "background_color": "#000000",
  "theme_color": "#000000",
  "icons": [
    { "src": "/icono-192.png", "sizes": "192x192", "type": "image/png", "purpose": "any" },
    { "src": "/icono-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any" }
  ]
}"##;

fn service_worker() -> String {
    format!(
        r#"// Guardado de la página para que abra sin la computadora.
const CACHE = 'things-shop-captura-v{version}';
const CONCHA = '/';

// Lo que se guarda de entrada. La página es lo único imprescindible; el resto
// hace que la aplicación instalada se vea bien sin conexión.
const TESOROS = ['/', '/manifest.webmanifest', '/icono-192.png', '/icono-512.png'];

// Lo que se enseña cuando no hay conexión y tampoco quedó nada guardado. Antes
// aquí se devolvía `undefined`, que para el navegador es un error de red: salía
// la pantalla del dinosaurio, que no explica nada ni dice qué hacer.
const SIN_NADA =
  '<!doctype html><html lang=es><head><meta charset=utf-8>' +
  '<meta name=viewport content="width=device-width,initial-scale=1">' +
  '<title>Sin conexión</title></head>' +
  '<body style="margin:0;background:#000;color:#f4f4f5;font:16px/1.5 -apple-system,Roboto,sans-serif;padding:28px 20px">' +
  '<h1 style="font-size:19px;margin:0 0 10px">No se pudo abrir</h1>' +
  '<p style="color:#a8a8ad;margin:0 0 14px">Esta página todavía no quedó guardada en el teléfono, ' +
  'así que necesita la computadora de la tienda encendida.</p>' +
  '<p style="color:#6e6e76;font-size:13.5px;margin:0">Conéctate al WiFi de la tienda con la caja prendida, ' +
  'abre la captura y espera unos segundos. Cuando diga que está lista, ya va a abrir sola.</p>' +
  '</body></html>';

function paginaSinConexion() {{
  return new Response(SIN_NADA, {{
    status: 200,
    headers: {{ 'content-type': 'text/html; charset=utf-8' }}
  }});
}}

self.addEventListener('install', (e) => {{
  // Se guarda en cuanto se instala, sin esperar a nada: si el teléfono se sale
  // del WiFi en el siguiente minuto, ya está a salvo.
  //
  // Cada uno por separado y sin rendirse. Con un solo `addAll`, que fallara la
  // descarga de un icono hacía fracasar la instalación entera y el teléfono se
  // quedaba sin nada guardado.
  e.waitUntil((async () => {{
    const c = await caches.open(CACHE);
    await Promise.all(TESOROS.map((ruta) => c.add(ruta).catch((err) => {{
      console.warn('No se pudo guardar', ruta, err);
    }})));
    await self.skipWaiting();
  }})());
}});

self.addEventListener('activate', (e) => {{
  e.waitUntil((async () => {{
    // Lo viejo solo se tira cuando lo nuevo ya sirve.
    //
    // Antes se borraba siempre, y combinado con una instalación que nunca falla
    // eso dejaba al teléfono sin NADA: si esta versión se instalaba sin alcanzar
    // la caja, su almacén quedaba vacío y aun así borraba el de la versión
    // anterior, que sí tenía la página. El teléfono perdía lo que ya funcionaba.
    const c = await caches.open(CACHE);
    const listo = await c.match(CONCHA, {{ ignoreSearch: true }});
    if (listo) {{
      const claves = await caches.keys();
      await Promise.all(claves.filter((k) => k !== CACHE).map((k) => caches.delete(k)));
    }}
    await self.clients.claim();
  }})());
}});

/** La página guardada, sea de esta versión o de una anterior. */
async function conchaGuardada() {{
  const propia = await caches.open(CACHE).then((c) => c.match(CONCHA, {{ ignoreSearch: true }}));
  if (propia) return propia;
  for (const clave of await caches.keys()) {{
    const vieja = await caches.open(clave).then((c) => c.match(CONCHA, {{ ignoreSearch: true }}));
    if (vieja) return vieja;
  }}
  return null;
}}

self.addEventListener('fetch', (e) => {{
  const url = new URL(e.request.url);

  // Lo que habla con la caja nunca se guarda. Una respuesta vieja aquí sería
  // peor que un error: el teléfono creería que mandó algo que no mandó.
  if (url.pathname.startsWith('/api/')) return;
  if (e.request.method !== 'GET') return;

  // La página: se intenta pedir fresca y se guarda; si no hay caja, la guardada.
  if (e.request.mode === 'navigate' || url.pathname === CONCHA) {{
    e.respondWith((async () => {{
      try {{
        const r = await fetch(e.request);
        // Solo se guarda lo que salió bien. `cache.put` acepta un 404 o un 500
        // sin chistar, y una vez guardado eso se sirve para siempre: la página
        // quedaría rota sin forma de arreglarla desde el teléfono.
        if (r && r.ok) {{
          const copia = r.clone();
          caches.open(CACHE).then((c) => c.put(CONCHA, copia)).catch(() => {{}});
        }}
        return r;
      }} catch (err) {{
        return (await conchaGuardada()) || paginaSinConexion();
      }}
    }})());
    return;
  }}

  e.respondWith(
    caches.match(e.request, {{ ignoreSearch: true }})
      .then((g) => g || fetch(e.request))
      .catch(() => paginaSinConexion())
  );
}});
"#,
        version = VERSION
    )
}

fn png(bytes: &'static [u8]) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes,
    )
}

pub fn rutas() -> Router<super::server::Contexto> {
    Router::new()
        .route(
            "/manifest.webmanifest",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/manifest+json")],
                    MANIFIESTO,
                )
            }),
        )
        .route(
            "/sw.js",
            get(|| async {
                (
                    [
                        (header::CONTENT_TYPE, "application/javascript"),
                        // Sin esto el navegador podría servir un service worker
                        // viejo y la página quedaría congelada en una versión.
                        (header::CACHE_CONTROL, "no-cache"),
                    ],
                    service_worker(),
                )
            }),
        )
        .route("/icono-192.png", get(|| async { png(ICONO_192) }))
        .route("/icono-512.png", get(|| async { png(ICONO_512) }))
        .route("/apple-touch-icon.png", get(|| async { png(ICONO_180) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_service_worker_no_guarda_lo_que_habla_con_la_caja() {
        // Guardar una respuesta de `/api/` haría que el teléfono creyera que
        // mandó algo que no mandó. Es lo único que no puede fallar aquí.
        let sw = service_worker();
        assert!(sw.contains("url.pathname.startsWith('/api/')"));
        assert!(sw.contains("return;"));
    }

    #[test]
    fn lo_viejo_solo_se_tira_cuando_lo_nuevo_ya_sirve() {
        // LA REGRESIÓN. El `activate` borraba los almacenes anteriores siempre.
        // Junto con una instalación que ya nunca falla, eso dejaba al teléfono
        // sin nada: si esta versión se instalaba sin alcanzar la caja, su almacén
        // quedaba vacío y aun así borraba el de la versión anterior, que sí tenía
        // la página guardada. Se perdía lo único que funcionaba.
        let sw = service_worker();
        let activate = sw.split("addEventListener('activate'").nth(1).unwrap();
        let borrado = activate.find("caches.delete").expect("debe poder borrar lo viejo");
        let comprobacion = activate.find("if (listo)").expect("debe comprobar antes de borrar");
        assert!(comprobacion < borrado, "primero se comprueba, después se borra");
    }

    #[test]
    fn nunca_se_guarda_una_respuesta_con_error() {
        // `cache.put` acepta un 404 o un 500 sin chistar, y una vez guardado eso
        // se sirve para siempre: la página quedaría rota sin forma de arreglarla
        // desde el teléfono.
        let sw = service_worker();
        assert!(sw.contains("if (r && r.ok)"), "solo se guarda lo que salió bien");
    }

    #[test]
    fn sin_nada_guardado_se_explica_en_vez_de_fallar() {
        // Devolver `undefined` es un error de red para el navegador: sale la
        // pantalla del dinosaurio, que no dice qué pasó ni qué hacer.
        let sw = service_worker();
        assert!(sw.contains("paginaSinConexion"), "tiene que haber una salida digna");
        assert!(sw.contains("Conéctate al WiFi de la tienda"), "y decir qué hacer");
        assert!(!sw.contains("|| undefined"));
    }

    #[test]
    fn si_el_almacen_de_esta_version_esta_vacio_se_busca_en_los_anteriores() {
        // Para que actualizar nunca sea un paso atrás.
        let sw = service_worker();
        assert!(sw.contains("conchaGuardada"));
        let f = sw.split("async function conchaGuardada").nth(1).unwrap();
        assert!(f.contains("caches.keys()"), "tiene que mirar también los viejos");
    }

    #[test]
    fn guardar_un_icono_a_medias_no_tumba_la_instalacion() {
        // Con un `addAll`, que fallara un icono hacía fracasar la instalación
        // completa y el teléfono se quedaba sin nada guardado —el escenario
        // exacto que el guardado existe para evitar.
        let sw = service_worker();
        assert!(sw.contains(".catch("), "cada recurso tiene que poder fallar solo");
        assert!(!sw.contains(".addAll("), "addAll es todo o nada");
        assert!(sw.contains("skipWaiting"), "tiene que activarse igual");
    }

    #[test]
    fn se_guarda_la_pagina_y_lo_que_la_hace_verse_bien() {
        let sw = service_worker();
        for ruta in ["'/'", "/manifest.webmanifest", "/icono-192.png", "/icono-512.png"] {
            assert!(sw.contains(ruta), "falta {} en lo que se guarda", ruta);
        }
    }

    #[test]
    fn subir_la_version_cambia_el_nombre_del_guardado() {
        // Es lo que hace que el service worker viejo suelte lo que tenía.
        let sw = service_worker();
        assert!(sw.contains(&format!("things-shop-captura-v{}", VERSION)));
    }

    #[test]
    fn el_manifiesto_es_json_valido_y_apunta_a_sus_iconos() {
        let v: serde_json::Value = serde_json::from_str(MANIFIESTO).expect("manifiesto inválido");
        assert_eq!(v["display"], "standalone");
        assert_eq!(v["icons"][0]["src"], "/icono-192.png");
        assert_eq!(v["icons"][1]["src"], "/icono-512.png");
    }

    #[test]
    fn los_iconos_son_png_de_verdad() {
        for bytes in [ICONO_192, ICONO_512, ICONO_180] {
            assert_eq!(&bytes[..4], b"\x89PNG", "no es un PNG");
        }
    }
}
