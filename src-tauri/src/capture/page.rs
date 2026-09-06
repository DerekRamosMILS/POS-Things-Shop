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
    /* Los mismos negros y grises neutros que la computadora, para que se vea
       como la misma aplicación y no como una página aparte. */
    --bg: #000000; --card: #0F0F0F; --line: rgba(255,255,255,.11);
    --t1: #f4f4f5; --t2: #a8a8ad; --t3: #6e6e76;
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
  .chips { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 10px; }
  .chip {
    display: inline-flex; align-items: center; gap: 7px;
    padding: 7px 10px 7px 12px; border-radius: 999px; font-size: 14px;
    background: rgba(139,120,245,.16); color: var(--t1);
    border: 1px solid rgba(139,120,245,.35);
  }
  .chip button {
    width: 18px; height: 18px; padding: 0; border-radius: 50%;
    background: rgba(255,255,255,.14); color: var(--t1);
    font-size: 13px; line-height: 1; border: none;
  }
  .agregar { display: flex; gap: 8px; }
  .agregar input { flex: 1; }
  .agregar button { width: auto; padding: 0 18px; font-size: 15px; }
  .dos { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .sugerencias { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; }
  .sug {
    padding: 6px 11px; border-radius: 999px; font-size: 13px;
    background: rgba(255,255,255,.05); color: var(--t2);
    border: 1px solid var(--line); width: auto;
  }
  .reparto { margin-top: 14px; }
  .reparto .linea {
    display: flex; align-items: center; gap: 10px;
    padding: 9px 0; border-bottom: 1px solid var(--line);
  }
  .reparto .linea:last-child { border-bottom: none; }
  .reparto .que { flex: 1; font-size: 15px; }
  .reparto input { width: 84px; text-align: center; padding: 10px 6px; font-size: 17px; }
  .total { display: flex; justify-content: space-between; margin-top: 12px; font-size: 14px; }
  .total b { color: var(--t1); }
  .hist { font-size: 13px; color: var(--t3); }
  .hist div { padding: 7px 0; border-bottom: 1px solid var(--line); }
  .hist div:last-child { border-bottom: none; }
  .hist b { color: var(--t2); font-weight: 600; }
</style>
</head>
<body>

<h1>Capturar producto</h1>
<p class="sub">Se guarda directo en la computadora de la tienda.</p>

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
  <p class="hint">Si no sabes el precio, déjalo vacío y se lo pones después en la computadora.</p>
</div>

<div class="card">
  <label for="tallaInput">Tallas</label>
  <div class="agregar">
    <input id="tallaInput" placeholder="M" autocomplete="off" enterkeyhint="done">
    <button type="button" class="sec" id="addTalla">Agregar</button>
  </div>
  <div class="sugerencias" id="sugTallas"></div>
  <div class="chips" id="chipsTallas"></div>

  <label for="colorInput" style="margin-top:18px">Colores</label>
  <div class="agregar">
    <input id="colorInput" placeholder="Rojo" autocomplete="off" enterkeyhint="done">
    <button type="button" class="sec" id="addColor">Agregar</button>
  </div>
  <div class="chips" id="chipsColores"></div>

  <p class="hint" id="resumenVariantes">Sin tallas ni colores se guarda como una sola pieza.</p>

  <div class="reparto" id="reparto" style="display:none">
    <label style="margin-top:6px">¿Cuántas piezas de cada una?</label>
    <div id="lineasReparto"></div>
    <div class="total"><span>Total</span><b id="totalPiezas">0</b></div>
  </div>
</div>

<div class="card">
  <label>Fotos</label>
  <input id="camara" type="file" accept="image/*" capture="environment" hidden>
  <input id="galeriaInput" type="file" accept="image/*" multiple hidden>
  <div class="dos">
    <button type="button" class="sec" id="btnCamara">Tomar foto</button>
    <button type="button" class="sec" id="btnGaleria">De la galería</button>
  </div>
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
  <button type="button" class="sec" id="limpiarTodo" style="margin-top:14px">
    Limpiar tallas y colores
  </button>
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
  var MAX_VARIANTES = 60;
  var fotos = [];
  var tallas = [];
  var colores = [];
  // Piezas por combinación, indexadas por talla|color.
  var reparto = {};
  var enviando = false;

  // Las más usadas en ropa, para no teclearlas una por una.
  var SUGERENCIAS = ['XCH', 'CH', 'M', 'G', 'XG', 'Unitalla'];

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

  // ── Tallas y colores ──────────────────────────────────────────────────────

  function pintarChips(lista, contenedor, tipo) {
    $(contenedor).innerHTML = lista.map(function (v, i) {
      return '<span class="chip">' + escapar(v) +
             '<button data-tipo="' + tipo + '" data-i="' + i + '" aria-label="Quitar">&times;</button></span>';
    }).join('');
    pintarResumen();
  }

  function escapar(t) {
    return String(t).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }

  // Cada pareja de talla y color que hay que contar por separado.
  function listaCombinaciones() {
    if (!tallas.length && !colores.length) return [];
    if (!colores.length) return tallas.map(function (t) { return { talla: t, color: null }; });
    if (!tallas.length) return colores.map(function (c) { return { talla: null, color: c }; });
    var out = [];
    tallas.forEach(function (t) {
      colores.forEach(function (c) { out.push({ talla: t, color: c }); });
    });
    return out;
  }

  function etiqueta(c) {
    return [c.talla, c.color].filter(Boolean).join(' · ');
  }

  function claveDe(c) {
    return (c.talla || '') + '|' + (c.color || '');
  }

  function pintarResumen() {
    var lista = listaCombinaciones();

    if (lista.length === 0) {
      $('resumenVariantes').textContent = 'Sin tallas ni colores se guarda como una sola pieza.';
    } else if (lista.length === 1) {
      $('resumenVariantes').textContent =
        'Una sola combinación: se usan las piezas que pusiste arriba.';
    } else {
      $('resumenVariantes').textContent =
        lista.length + ' combinaciones. Pon cuántas piezas tienes de cada una.';
    }

    // Con una sola combinación no hay nada que repartir; preguntarlo sobraría.
    if (lista.length > 1) {
      $('reparto').style.display = 'block';
      $('lineasReparto').innerHTML = lista.map(function (c) {
        var k = claveDe(c);
        var valor = reparto[k] !== undefined ? reparto[k] : '';
        return '<div class="linea"><span class="que">' + escapar(etiqueta(c)) + '</span>' +
               '<input type="number" inputmode="numeric" min="0" step="1" placeholder="0" ' +
               'data-clave="' + escapar(k) + '" value="' + escapar(valor) + '"></div>';
      }).join('');
      actualizarTotal();
    } else {
      $('reparto').style.display = 'none';
    }

    pintarSugerencias();
  }

  function actualizarTotal() {
    var suma = 0;
    Array.prototype.forEach.call($('lineasReparto').querySelectorAll('input'), function (i) {
      suma += parseInt(i.value, 10) || 0;
    });
    $('totalPiezas').textContent = suma + (suma === 1 ? ' pieza' : ' piezas');
  }

  $('lineasReparto').addEventListener('input', function (e) {
    if (e.target.tagName !== 'INPUT') return;
    reparto[e.target.dataset.clave] = e.target.value;
    actualizarTotal();
  });

  function piezasCapturadas() {
    var lista = listaCombinaciones();
    if (lista.length < 2) return [];
    return lista.map(function (c) {
      return {
        talla: c.talla,
        color: c.color,
        cantidad: parseInt(reparto[claveDe(c)], 10) || 0
      };
    });
  }

  function agregar(lista, valor, contenedor, tipo) {
    valor = (valor || '').trim();
    if (!valor) return false;
    var repetida = lista.some(function (v) { return v.toLowerCase() === valor.toLowerCase(); });
    if (repetida) { aviso('"' + valor + '" ya está en la lista.', 'bad'); return false; }

    var futuras = tipo === 'talla'
      ? (colores.length ? (tallas.length + 1) * colores.length : tallas.length + 1)
      : (tallas.length ? tallas.length * (colores.length + 1) : colores.length + 1);
    if (futuras > MAX_VARIANTES) {
      aviso('Serían ' + futuras + ' combinaciones; el máximo es ' + MAX_VARIANTES + '.', 'bad');
      return false;
    }

    lista.push(valor);
    pintarChips(lista, contenedor, tipo);
    return true;
  }

  function pintarSugerencias() {
    var faltantes = SUGERENCIAS.filter(function (t) {
      return !tallas.some(function (v) { return v.toLowerCase() === t.toLowerCase(); });
    });
    $('sugTallas').innerHTML = faltantes.map(function (t) {
      return '<button type="button" class="sug" data-sug="' + t + '">' + t + '</button>';
    }).join('');
  }

  $('sugTallas').addEventListener('click', function (e) {
    var b = e.target.closest('.sug');
    if (b) agregar(tallas, b.dataset.sug, 'chipsTallas', 'talla');
  });

  function conectarAgregar(inputId, botonId, lista, contenedor, tipo) {
    var meter = function () {
      if (agregar(lista, $(inputId).value, contenedor, tipo)) {
        $(inputId).value = '';
      }
      $(inputId).focus();
    };
    $(botonId).addEventListener('click', meter);
    $(inputId).addEventListener('keydown', function (e) {
      if (e.key === 'Enter') { e.preventDefault(); meter(); }
    });
  }

  conectarAgregar('tallaInput', 'addTalla', tallas, 'chipsTallas', 'talla');
  conectarAgregar('colorInput', 'addColor', colores, 'chipsColores', 'color');

  document.addEventListener('click', function (e) {
    var b = e.target.closest('.chip button');
    if (!b) return;
    var lista = b.dataset.tipo === 'talla' ? tallas : colores;
    lista.splice(Number(b.dataset.i), 1);
    pintarChips(lista, b.dataset.tipo === 'talla' ? 'chipsTallas' : 'chipsColores', b.dataset.tipo);
  });

  // ── Fotos ─────────────────────────────────────────────────────────────────

  // Dos entradas distintas: `capture` abre la cámara directo, y sin ese atributo
  // el teléfono ofrece la galería. Un solo botón dejaría al sistema decidir, y
  // decide distinto en cada modelo.
  $('btnCamara').addEventListener('click', function () { $('camara').click(); });
  $('btnGaleria').addEventListener('click', function () { $('galeriaInput').click(); });

  async function recibirFotos(e) {
    var archivos = Array.prototype.slice.call(e.target.files || []);
    e.target.value = '';
    if (!archivos.length) return;

    $('btnCamara').disabled = true;
    $('btnGaleria').disabled = true;
    $('btnCamara').textContent = 'Procesando...';
    $('btnGaleria').textContent = '...';
    try {
      for (var i = 0; i < archivos.length; i++) {
        if (fotos.length >= MAX_FOTOS) { aviso('Máximo ' + MAX_FOTOS + ' fotos por producto.', 'bad'); break; }
        var grande = await reducir(archivos[i], 1280, 0.75);
        var chica = await reducir(archivos[i], 320, 0.7);
        fotos.push({ photo: grande, thumbnail: chica });
        pintarGaleria();
      }
    } catch (err) {
      aviso('No se pudo procesar una foto: ' + err.message, 'bad');
    } finally {
      $('btnCamara').disabled = false;
      $('btnGaleria').disabled = false;
      $('btnCamara').textContent = 'Tomar foto';
      $('btnGaleria').textContent = 'De la galería';
    }
  }

  $('camara').addEventListener('change', recibirFotos);
  $('galeriaInput').addEventListener('change', recibirFotos);

  function agregarAlHistorial(sku, nombre) {
    $('tarjetaHist').style.display = 'block';
    var fila = document.createElement('div');
    fila.innerHTML = '<b>' + sku + '</b> &nbsp; ' + nombre;
    $('historial').prepend(fila);
  }

  // Se conserva lo que suele repetirse entre prendas seguidas —tallas y
  // colores— para no volver a capturarlo en cada producto.
  function limpiar(conservarVariantes) {
    $('nombre').value = '';
    $('precio').value = '';
    $('notas').value = '';
    fotos = [];
    // El reparto es de esa prenda: la siguiente tendrá otras cantidades.
    reparto = {};
    if (!conservarVariantes) {
      tallas.length = 0;
      colores.length = 0;
      $('existencia').value = '';
      pintarChips(tallas, 'chipsTallas', 'talla');
      pintarChips(colores, 'chipsColores', 'color');
    }
    pintarResumen();
    pintarGaleria();
    $('nombre').focus();
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
          nombre: nombre,
          precio: parseFloat($('precio').value) || null,
          existencia: parseInt($('existencia').value, 10) || 0,
          notas: $('notas').value.trim() || null,
          tallas: tallas,
          colores: colores,
          // Solo cuando hubo algo que repartir. Con una sola combinación el
          // reparto está vacío y mandarlo pondría cero piezas, ignorando las
          // que se pusieron arriba.
          piezas: piezasCapturadas(),
          fotos: fotos
        })
      });
      var data = await r.json();
      if (r.ok && data.ok) {
        aviso('Guardado como <b>' + data.sku + '</b>. Ya puedes capturar el siguiente.', 'ok');
        agregarAlHistorial(data.sku, nombre);
        limpiar(true);
      } else {
        aviso(data.mensaje || 'No se pudo guardar.', 'bad');
      }
    } catch (err) {
      aviso('Se perdió la conexión. Revisa que sigas en el WiFi de la tienda.', 'bad');
    } finally {
      enviando = false;
      $('guardar').disabled = false;
      $('guardar').textContent = 'Guardar producto';
    }
  });

  // Avisa de entrada si el código ya no sirve, en vez de dejar que capture todo
  // un producto para descubrirlo al guardar.
  $('limpiarTodo').addEventListener('click', function () { limpiar(false); });

  fetch('/api/verificar', { headers: { 'X-Codigo': codigo } })
    .then(function (r) { return r.json().then(function (d) { return { ok: r.ok, d: d }; }); })
    .then(function (res) {
      if (!res.ok) aviso(res.d.mensaje + ' Vuelve a apuntar la cámara al código.', 'bad');
    })
    .catch(function () {
      aviso('No se pudo conectar. Revisa que estés en el WiFi de la tienda.', 'bad');
    });

  pintarGaleria();
  pintarResumen();
})();
</script>
</body>
</html>
"####;
