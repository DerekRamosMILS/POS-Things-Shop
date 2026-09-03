//! Página que se sirve al celular.
//!
//! Va incrustada en el binario porque no puede depender de internet: la tienda
//! podría no tenerlo, y el punto de todo esto es que funcione en la red local.
//! Por lo mismo no carga tipografías ni bibliotecas externas.
//!
//! Las fotos se reducen en el propio teléfono antes de enviarse: una foto de
//! cámara moderna ronda los 5 MB y mandarla tal cual por WiFi sería lento sin
//! ganar nada. Se envían dos tamaños, uno de catálogo y una miniatura, que es
//! justo lo que espera el almacenamiento de fotos.

pub const HTML: &str = r####"
<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>Capturar producto</title>
<style>
  :root {
    --bg: #0a0716; --card: #14101f; --line: rgba(255,255,255,.10);
    --t1: #f2f0f7; --t2: #b9b4c9; --t3: #7d7791;
    --primary: #8B78F5; --ok: #22D3A0; --bad: #f45270;
  }
  * { box-sizing: border-box; -webkit-tap-highlight-color: transparent; }
  body {
    margin: 0; background: var(--bg); color: var(--t1);
    font: 16px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    padding: 16px 14px calc(24px + env(safe-area-inset-bottom));
  }
  h1 { font-size: 19px; margin: 0 0 2px; }
  .sub { color: var(--t3); font-size: 13px; margin: 0 0 18px; }
  .card { background: var(--card); border: 1px solid var(--line); border-radius: 16px; padding: 16px; margin-bottom: 14px; }
  label { display: block; font-size: 13px; color: var(--t2); margin-bottom: 6px; font-weight: 600; }
  input, textarea {
    width: 100%; padding: 13px 14px; border-radius: 12px; font-size: 17px;
    background: rgba(255,255,255,.05); border: 1px solid var(--line);
    color: var(--t1); font-family: inherit; outline: none;
  }
  input:focus, textarea:focus { border-color: var(--primary); }
  .fila { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .campo { margin-bottom: 14px; }
  button {
    width: 100%; padding: 16px; border-radius: 14px; border: none;
    font-size: 17px; font-weight: 700; font-family: inherit; cursor: pointer;
    background: var(--primary); color: #fff;
  }
  button:disabled { opacity: .5; }
  button.sec { background: rgba(255,255,255,.07); color: var(--t1); border: 1px solid var(--line); }
  .fotos { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; margin-top: 12px; }
  .foto { position: relative; aspect-ratio: 1; border-radius: 12px; overflow: hidden; background: rgba(255,255,255,.05); }
  .foto img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .quitar {
    position: absolute; top: 4px; right: 4px; width: 26px; height: 26px;
    border-radius: 50%; background: rgba(0,0,0,.65); color: #fff;
    border: none; font-size: 15px; line-height: 1; padding: 0;
  }
  .aviso { padding: 12px 14px; border-radius: 12px; font-size: 14px; margin-bottom: 14px; }
  .aviso.ok { background: rgba(34,211,160,.12); color: var(--ok); border: 1px solid rgba(34,211,160,.3); }
  .aviso.bad { background: rgba(244,82,112,.12); color: var(--bad); border: 1px solid rgba(244,82,112,.3); }
  .hint { font-size: 12px; color: var(--t3); margin-top: 6px; }
  .hist { font-size: 13px; color: var(--t3); }
  .hist div { padding: 7px 0; border-bottom: 1px solid var(--line); }
  .hist div:last-child { border-bottom: none; }
  .hist b { color: var(--t2); font-weight: 600; }
</style>
</head>
<body>

<h1>Capturar producto</h1>
<p class="sub">Se guarda directo en la caja. No necesita internet.</p>

<div id="aviso"></div>

<div class="card">
  <div class="campo">
    <label for="nombre">Nombre *</label>
    <input id="nombre" placeholder="Vestido amarillo" autocomplete="off" enterkeyhint="next">
  </div>

  <div class="fila campo">
    <div>
      <label for="precio">Precio de venta</label>
      <input id="precio" type="number" inputmode="decimal" min="0" step="0.01" placeholder="0.00">
    </div>
    <div>
      <label for="existencia">Piezas</label>
      <input id="existencia" type="number" inputmode="numeric" min="0" step="1" placeholder="1">
    </div>
  </div>
  <p class="hint">Sin precio se guarda como borrador y no se puede vender hasta que le pongas uno.</p>
</div>

<div class="card">
  <label>Fotos</label>
  <input id="archivo" type="file" accept="image/*" multiple hidden>
  <button type="button" class="sec" id="tomar">Tomar o elegir fotos</button>
  <div class="fotos" id="galeria"></div>
  <p class="hint" id="contador"></p>
</div>

<div class="card">
  <div class="campo" style="margin:0">
    <label for="notas">Notas (opcional)</label>
    <textarea id="notas" rows="2" placeholder="Talla, color, proveedor..."></textarea>
  </div>
</div>

<button id="guardar">Guardar producto</button>

<div class="card" id="tarjetaHist" style="margin-top:18px; display:none">
  <label>Capturados en esta sesión</label>
  <div class="hist" id="historial"></div>
</div>

<script>
(function () {
  'use strict';

  // El código de emparejamiento viaja en el enlace del QR. Se guarda para que
  // recargar la página no obligue a volver a escanearlo.
  var params = new URLSearchParams(location.search);
  var codigo = params.get('c') || sessionStorage.getItem('codigo') || '';
  if (params.get('c')) {
    sessionStorage.setItem('codigo', codigo);
    history.replaceState(null, '', location.pathname);
  }

  var MAX_FOTOS = 8;
  var fotos = [];
  var enviando = false;

  var $ = function (id) { return document.getElementById(id); };

  function aviso(texto, clase) {
    $('aviso').innerHTML = texto ? '<div class="aviso ' + clase + '">' + texto + '</div>' : '';
    if (texto) window.scrollTo({ top: 0, behavior: 'smooth' });
  }

  // Reduce en el teléfono: mandar la foto original de una cámara moderna serían
  // varios megabytes por WiFi sin ganar calidad aprovechable.
  function reducir(file, maxLado, calidad) {
    return new Promise(function (resolve, reject) {
      var lector = new FileReader();
      lector.onerror = function () { reject(new Error('No se pudo leer la foto')); };
      lector.onload = function () {
        var img = new Image();
        img.onerror = function () { reject(new Error('El archivo no es una imagen')); };
        img.onload = function () {
          var w = img.width, h = img.height;
          if (w >= h && w > maxLado) { h = Math.round(h * maxLado / w); w = maxLado; }
          else if (h > w && h > maxLado) { w = Math.round(w * maxLado / h); h = maxLado; }
          var lienzo = document.createElement('canvas');
          lienzo.width = w; lienzo.height = h;
          lienzo.getContext('2d').drawImage(img, 0, 0, w, h);
          resolve(lienzo.toDataURL('image/jpeg', calidad));
        };
        img.src = lector.result;
      };
      lector.readAsDataURL(file);
    });
  }

  function pintarGaleria() {
    $('galeria').innerHTML = fotos.map(function (f, i) {
      return '<div class="foto"><img src="' + f.thumbnail + '" alt="">' +
             '<button class="quitar" data-i="' + i + '" aria-label="Quitar">&times;</button></div>';
    }).join('');
    $('contador').textContent = fotos.length
      ? fotos.length + ' de ' + MAX_FOTOS + ' fotos'
      : 'La primera foto es la que se ve en el catálogo.';
  }

  $('galeria').addEventListener('click', function (e) {
    var b = e.target.closest('.quitar');
    if (!b) return;
    fotos.splice(Number(b.dataset.i), 1);
    pintarGaleria();
  });

  $('tomar').addEventListener('click', function () { $('archivo').click(); });

  $('archivo').addEventListener('change', async function (e) {
    var archivos = Array.prototype.slice.call(e.target.files || []);
    e.target.value = '';
    if (!archivos.length) return;

    $('tomar').disabled = true;
    $('tomar').textContent = 'Procesando...';
    try {
      for (var i = 0; i < archivos.length; i++) {
        if (fotos.length >= MAX_FOTOS) { aviso('Máximo ' + MAX_FOTOS + ' fotos por producto.', 'bad'); break; }
        var grande = await reducir(archivos[i], 1600, 0.85);
        var chica = await reducir(archivos[i], 320, 0.7);
        fotos.push({ photo: grande, thumbnail: chica });
        pintarGaleria();
      }
    } catch (err) {
      aviso('No se pudo procesar una foto: ' + err.message, 'bad');
    } finally {
      $('tomar').disabled = false;
      $('tomar').textContent = 'Tomar o elegir fotos';
    }
  });

  function agregarAlHistorial(sku, nombre) {
    $('tarjetaHist').style.display = 'block';
    var fila = document.createElement('div');
    fila.innerHTML = '<b>' + sku + '</b> &nbsp; ' + nombre;
    $('historial').prepend(fila);
  }

  function limpiar() {
    $('nombre').value = '';
    $('precio').value = '';
    $('existencia').value = '';
    $('notas').value = '';
    fotos = [];
    pintarGaleria();
  }

  $('guardar').addEventListener('click', async function () {
    if (enviando) return;
    var nombre = $('nombre').value.trim();
    if (!nombre) { aviso('Ponle nombre al producto.', 'bad'); $('nombre').focus(); return; }

    enviando = true;
    $('guardar').disabled = true;
    $('guardar').textContent = 'Guardando...';
    aviso('', '');

    try {
      var r = await fetch('/api/producto', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'X-Codigo': codigo },
        body: JSON.stringify({
          codigo: codigo,
          nombre: nombre,
          precio: parseFloat($('precio').value) || null,
          existencia: parseInt($('existencia').value, 10) || 0,
          notas: $('notas').value.trim() || null,
          fotos: fotos
        })
      });
      var data = await r.json();
      if (r.ok && data.ok) {
        aviso('Guardado como <b>' + data.sku + '</b>', 'ok');
        agregarAlHistorial(data.sku, nombre);
        limpiar();
      } else {
        aviso(data.mensaje || 'No se pudo guardar.', 'bad');
      }
    } catch (err) {
      aviso('Se perdió la conexión con la caja. Revisa que sigas en el mismo WiFi.', 'bad');
    } finally {
      enviando = false;
      $('guardar').disabled = false;
      $('guardar').textContent = 'Guardar producto';
    }
  });

  // Avisa de entrada si el código ya no sirve, en vez de dejar que capture todo
  // un producto para descubrirlo al guardar.
  fetch('/api/verificar', { headers: { 'X-Codigo': codigo } })
    .then(function (r) { return r.json().then(function (d) { return { ok: r.ok, d: d }; }); })
    .then(function (res) {
      if (!res.ok) aviso(res.d.mensaje + ' Vuelve a escanear el código en la caja.', 'bad');
    })
    .catch(function () {
      aviso('No se pudo contactar la caja. Revisa el WiFi.', 'bad');
    });

  pintarGaleria();
})();
</script>
</body>
</html>
"####;
