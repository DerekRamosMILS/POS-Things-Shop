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
const VERSION: &str = "4";

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

self.addEventListener('install', (e) => {{
  // Se guarda en cuanto se instala, sin esperar a nada: si el teléfono se sale
  // del WiFi en el siguiente minuto, ya está a salvo.
  //
  // Cada uno por separado y sin rendirse. Con un solo `addAll`, que fallara la
  // descarga de un icono hacía fracasar la instalación entera: el service worker
  // no llegaba a activarse y el teléfono se quedaba sin **nada** guardado, que es
  // justo el escenario que esto existe para evitar. Si la página no se pudo
  // guardar ahora, el manejador de abajo la guarda en la primera visita que
  // funcione.
  e.waitUntil((async () => {{
    const c = await caches.open(CACHE);
    await Promise.all(TESOROS.map((ruta) => c.add(ruta).catch((err) => {{
      console.warn('No se pudo guardar', ruta, err);
    }})));
    await self.skipWaiting();
  }})());
}});

self.addEventListener('activate', (e) => {{
  e.waitUntil(
    caches.keys()
      .then((claves) => Promise.all(
        claves.filter((k) => k !== CACHE).map((k) => caches.delete(k))
      ))
      .then(() => self.clients.claim())
  );
}});

self.addEventListener('fetch', (e) => {{
  const url = new URL(e.request.url);

  // Lo que habla con la caja nunca se guarda. Una respuesta vieja aquí sería
  // peor que un error: el teléfono creería que mandó algo que no mandó.
  if (url.pathname.startsWith('/api/')) return;
  if (e.request.method !== 'GET') return;

  // La página: se intenta pedir fresca y se guarda; si no hay caja, la guardada.
  if (e.request.mode === 'navigate' || url.pathname === CONCHA) {{
    e.respondWith(
      fetch(e.request)
        .then((r) => {{
          const copia = r.clone();
          caches.open(CACHE).then((c) => c.put(CONCHA, copia));
          return r;
        }})
        .catch(() => caches.match(CONCHA, {{ ignoreSearch: true }}))
    );
    return;
  }}

  e.respondWith(
    caches.match(e.request, {{ ignoreSearch: true }}).then((g) => g || fetch(e.request))
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
