//! La página que entrega el certificado, sin cifrar.
//!
//! Es el huevo y la gallina: para confiar en la conexión cifrada el teléfono
//! necesita antes el certificado de la tienda, y no puede pedirlo por una
//! conexión en la que todavía no confía. Por aquí no pasa nada de la tienda:
//! solo el certificado —que es público por definición— y las instrucciones.
//!
//! Las instrucciones están escritas para quien nunca ha instalado un
//! certificado, que es el caso. iPhone lleva un paso extra escondido en un menú
//! que nadie encuentra solo, y por eso está deletreado.

use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;

/// El certificado se descarga con extensión `.crt`: es la que hace que Android
/// ofrezca instalarlo en vez de abrirlo como texto.
async fn certificado(
    axum::extract::State(ca_pem): axum::extract::State<String>,
) -> impl IntoResponse {
    (
        [
            ("content-type", "application/x-x509-ca-cert"),
            ("content-disposition", "attachment; filename=\"things-shop.crt\""),
        ],
        ca_pem,
    )
}

pub fn pagina(ip: &str) -> String {
    HTML.replace("{{IP}}", ip)
}

pub fn router(ca_pem: String, ip: String) -> Router {
    let instrucciones = pagina(&ip);
    Router::new()
        .route("/", get(move || async move { Html(instrucciones) }))
        .route("/things-shop.crt", get(certificado))
        .with_state(ca_pem)
}

const HTML: &str = r####"
<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Preparar el teléfono</title>
<style>
  :root {
    --bg: #000000; --card: #0F0F0F; --line: rgba(255,255,255,.11);
    --t1: #f4f4f5; --t2: #a8a8ad; --t3: #6e6e76; --primary: #8B78F5;
  }
  * { box-sizing: border-box; -webkit-tap-highlight-color: transparent; }
  body {
    margin: 0; background: var(--bg); color: var(--t1);
    font: 16px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    padding: 20px 16px calc(32px + env(safe-area-inset-bottom));
  }
  h1 { font-size: 21px; margin: 0 0 6px; }
  .sub { color: var(--t3); font-size: 14px; margin: 0 0 22px; }
  .card { background: var(--card); border: 1px solid var(--line); border-radius: 16px; padding: 18px 16px; margin-bottom: 14px; }
  .card h2 { font-size: 15px; margin: 0 0 10px; display: flex; align-items: center; gap: 9px; }
  .n { display: inline-flex; align-items: center; justify-content: center;
       width: 24px; height: 24px; border-radius: 50%; background: var(--primary);
       color: #fff; font-size: 13px; font-weight: 800; flex-shrink: 0; }
  ol { margin: 0; padding-left: 22px; color: var(--t2); font-size: 14px; }
  li { margin-bottom: 7px; }
  li:last-child { margin-bottom: 0; }
  b { color: var(--t1); }
  a.boton {
    display: block; text-align: center; text-decoration: none;
    background: var(--primary); color: #fff; font-weight: 700; font-size: 16px;
    padding: 15px; border-radius: 13px; margin: 4px 0 2px;
  }
  .hint { color: var(--t3); font-size: 12.5px; margin-top: 12px; line-height: 1.5; }
  .listo { border-color: rgba(139,120,245,.45); }
  .listo a.boton { background: transparent; border: 1px solid var(--primary); color: var(--primary); }
  .tabs { display: flex; gap: 6px; margin-bottom: 12px; }
  .tabs button {
    flex: 1; padding: 9px; border-radius: 10px; font: inherit; font-size: 14px;
    border: 1px solid var(--line); background: transparent; color: var(--t3); cursor: pointer;
  }
  .tabs button[aria-selected="true"] { background: rgba(139,120,245,.14); border-color: var(--primary); color: var(--t1); font-weight: 700; }
</style>
</head>
<body>

<h1>Preparar el teléfono</h1>
<p class="sub">Es una sola vez. Después vas a poder capturar productos aunque la computadora esté apagada.</p>

<div class="card">
  <h2><span class="n">1</span> Descarga el permiso</h2>
  <a class="boton" href="/things-shop.crt" download>Descargar</a>
  <p class="hint">Es un archivo chiquito que le dice a tu teléfono que puede confiar en la computadora de la tienda. No sirve para nada más.</p>
</div>

<div class="card">
  <h2><span class="n">2</span> Instálalo</h2>
  <div class="tabs" role="tablist">
    <button role="tab" id="tab-android" aria-selected="true" onclick="ver('android')">Android</button>
    <button role="tab" id="tab-iphone" aria-selected="false" onclick="ver('iphone')">iPhone</button>
  </div>

  <ol id="pasos-android">
    <li>Abre el archivo que se acaba de descargar.</li>
    <li>Si te pregunta el nombre, escribe <b>Things Shop</b>.</li>
    <li>Si te pregunta para qué es, elige <b>VPN y aplicaciones</b>.</li>
    <li>Puede pedirte el PIN o la huella del teléfono. Es normal.</li>
  </ol>

  <ol id="pasos-iphone" style="display:none">
    <li>Toca <b>Permitir</b> cuando pregunte si quieres descargar un perfil.</li>
    <li>Ve a <b>Ajustes</b>. Arriba del todo va a aparecer <b>Perfil descargado</b>: tócalo e instálalo.</li>
    <li>Ahora falta el paso que nadie encuentra. En <b>Ajustes</b> entra a
        <b>General</b> → <b>Información</b> → hasta abajo,
        <b>Ajustes de confianza en certificados</b>.</li>
    <li>Prende el interruptor que dice <b>Things Shop</b>.</li>
  </ol>
</div>

<div class="card listo">
  <h2><span class="n">3</span> Listo</h2>
  <a class="boton" id="abrir" href="https://{{IP}}:7423/">Abrir la captura</a>
  <p class="hint"><b>Esta página ya no la necesitas.</b> Toca el botón: la captura se
     abre y ahí mismo te ofrece instalarla en la pantalla de inicio. Guarda <b>esa</b>,
     que es la que funciona con la computadora apagada — esta de aquí no.</p>
</div>

<script>
  // El código de emparejamiento viaja desde el QR hasta la aplicación, para que
  // quien ya instaló el permiso solo tenga que tocar el botón de abajo.
  (function () {
    var c = new URLSearchParams(location.search).get('c');
    if (c) document.getElementById('abrir').href = 'https://{{IP}}:7423/?c=' + encodeURIComponent(c);
  })();

  function ver(cual) {
    var esAndroid = cual === 'android';
    document.getElementById('pasos-android').style.display = esAndroid ? '' : 'none';
    document.getElementById('pasos-iphone').style.display = esAndroid ? 'none' : '';
    document.getElementById('tab-android').setAttribute('aria-selected', String(esAndroid));
    document.getElementById('tab-iphone').setAttribute('aria-selected', String(!esAndroid));
  }
  // Arranca en la pestaña del teléfono que la está abriendo, para que quien no
  // sepa qué tiene no tenga que elegir.
  if (/iPhone|iPad|iPod/.test(navigator.userAgent)) ver('iphone');
</script>

</body>
</html>
"####;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_pagina_lleva_la_direccion_de_esta_caja() {
        let html = pagina("192.168.0.55");
        assert!(html.contains("https://192.168.0.55:7423/"));
        assert!(!html.contains("{{IP}}"), "no debe quedar el hueco sin llenar");
    }

    #[test]
    fn el_codigo_viaja_del_qr_a_la_captura() {
        // Quien ya instaló el permiso no debería tener que escanear otra vez ni
        // teclear el código: solo tocar el último botón.
        let html = pagina("192.168.0.55");
        assert!(html.contains("location.search).get('c')"));
        assert!(html.contains("'https://192.168.0.55:7423/?c=' + encodeURIComponent(c)"));
    }

    #[test]
    fn no_invita_a_guardar_esta_pagina_sino_la_otra() {
        // Guardar esta página en la pantalla de inicio no sirve de nada: va sin
        // cifrar, no tiene service worker y con la computadora apagada no abre.
        // El "guárdala" de antes se leía como si hablara de esta, que es la que
        // se tiene enfrente al leerlo.
        let html = pagina("192.168.0.55");
        assert!(html.contains("Esta página ya no la necesitas"));
        assert!(html.contains("esta de aquí no"));
    }

    #[test]
    fn estan_los_dos_instructivos() {
        let html = pagina("10.0.0.2");
        assert!(html.contains("pasos-android"));
        assert!(html.contains("pasos-iphone"));
        // El paso que la gente no encuentra sola tiene que estar deletreado.
        assert!(html.contains("Ajustes de confianza en certificados"));
    }
}
