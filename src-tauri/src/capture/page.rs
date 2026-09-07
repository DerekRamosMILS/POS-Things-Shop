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
<link rel="manifest" href="/manifest.webmanifest">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<meta name="theme-color" content="#000000">
<meta name="apple-mobile-web-app-capable" content="yes">
<meta name="apple-mobile-web-app-status-bar-style" content="black">
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
  .pend {
    background: rgba(139,120,245,.12); border: 1px solid rgba(139,120,245,.4);
    border-radius: 14px; padding: 14px 15px; margin-bottom: 16px;
  }
  .pend p { margin: 0 0 10px; font-size: 14px; color: var(--t1); }
  .pend button { width: 100%; }
  /* Las dos cosas que se hacen con el teléfono */
  .modos { display: flex; gap: 6px; margin-bottom: 18px; }
  .modos button {
    flex: 1; padding: 12px; border-radius: 12px; font: inherit; font-size: 15px;
    border: 1px solid var(--line); background: transparent; color: var(--t3); cursor: pointer;
  }
  .modos button[aria-selected="true"] {
    background: rgba(139,120,245,.15); border-color: var(--primary);
    color: var(--t1); font-weight: 700;
  }
  .renglon {
    display: flex; align-items: center; gap: 12px;
    padding: 12px 0; border-bottom: 1px solid var(--line);
  }
  .renglon:last-child { border-bottom: none; }
  .renglon .que { flex: 1; min-width: 0; }
  .renglon .que b { display: block; font-size: 15px; color: var(--t1); font-weight: 600;
                    overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .renglon .que span { font-size: 12.5px; color: var(--t3); }
  .renglon input { width: 78px; text-align: center; padding: 11px 6px; font-size: 17px; }
  .vacio { color: var(--t3); font-size: 14px; text-align: center; padding: 26px 0; }
  .hist { font-size: 13px; color: var(--t3); }
  .hist div { padding: 7px 0; border-bottom: 1px solid var(--line); }
  .hist div:last-child { border-bottom: none; }
  .hist b { color: var(--t2); font-weight: 600; }
</style>
</head>
<body>

<h1 id="titulo">Capturar producto</h1>
<p class="sub" id="subtitulo">Se guarda directo en la computadora de la tienda.</p>

<div class="modos" role="tablist">
  <button role="tab" id="modoAlta" aria-selected="true" onclick="verModo('alta')">Dar de alta</button>
  <button role="tab" id="modoConteo" aria-selected="false" onclick="verModo('conteo')">Contar</button>
</div>

<div id="aviso"></div>

<div class="pend" id="pendientes" style="display:none">
  <p id="pendientesTexto"></p>
  <button type="button" class="sec" id="sincronizar">Mandar ahora</button>
</div>

<div id="pantallaAlta">

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

</div><!-- /pantallaAlta -->

<div id="pantallaConteo" style="display:none">
  <div class="card">
    <label for="buscar">Busca la prenda</label>
    <input id="buscar" placeholder="Vestido, o el código" autocomplete="off" enterkeyhint="search">
    <p class="hint" id="estadoCatalogo"></p>
  </div>

  <div class="card" id="tarjetaResultados" style="display:none">
    <div id="resultados"></div>
  </div>

  <div class="card" id="tarjetaContar" style="display:none">
    <label id="contandoQue"></label>
    <p class="hint">Anota cuántas piezas ves. Si mientras tanto se vende algo en la
       tienda, la computadora lo toma en cuenta al recibir el conteo.</p>
    <div id="lineasConteo"></div>
    <button type="button" id="guardarConteo" style="margin-top:16px">Guardar conteo</button>
    <button type="button" class="sec" id="cancelarConteo" style="margin-top:10px">Cancelar</button>
  </div>
</div>

<script>
(function () {
  'use strict';

  // El código de emparejamiento viaja en el enlace del QR. Se guarda para que
  // recargar la página no obligue a volver a escanearlo.
  var params = new URLSearchParams(location.search);
  var codigo = params.get('c') || localStorage.getItem('codigo') || '';
  if (params.get('c')) {
    // En almacenamiento permanente y no de sesión: la aplicación se abre desde
    // la pantalla de inicio días después y tiene que seguir emparejada.
    try { localStorage.setItem('codigo', codigo); } catch (e) {}
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

  function agregarAlHistorial(izquierda, derecha, movido) {
    $('tarjetaHist').style.display = 'block';
    var fila = document.createElement('div');
    // Escapado: el nombre lo teclea quien captura, y aquí se está armando HTML.
    var texto = '<b>' + escapar(izquierda) + '</b> &nbsp; ' + escapar(derecha);
    if (movido) {
      // Lo que se vendió o devolvió entre el conteo y su llegada. Verlo es lo
      // que permite confiar en el número: si no, parecería que la caja
      // "corrigió" el conteo por su cuenta.
      texto += ' <span style="color:var(--t3)">(' +
        (movido < 0 ? 'se vendieron ' + Math.abs(movido) : 'entraron ' + movido) +
        ' mientras tanto)</span>';
    }
    fila.innerHTML = texto;
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


  // ── La cola: lo capturado vive en el teléfono hasta que la caja lo recibe ──
  //
  // Todo pasa por aquí, con o sin conexión. Tener un solo camino es lo que hace
  // que esto sea confiable: si "guardar" hiciera una cosa con WiFi y otra sin
  // él, el caso raro —justo el que importa— sería el que nadie prueba.
  //
  // Cada producto lleva un identificador que se le pone al capturarlo, no al
  // mandarlo. Por eso reintentar es seguro: si la conexión se cae después de
  // que la caja lo recibió pero antes de que conteste, el siguiente intento
  // lleva el mismo identificador y la caja devuelve el código que ya le dio en
  // vez de dar de alta la prenda otra vez.

  var BD = null;

  function abrirBD() {
    if (BD) return Promise.resolve(BD);
    return new Promise(function (resolve, reject) {
      var req = indexedDB.open('things-shop-captura', 1);
      req.onupgradeneeded = function () {
        req.result.createObjectStore('pendientes', { keyPath: 'id' });
      };
      req.onsuccess = function () { BD = req.result; resolve(BD); };
      req.onerror = function () { reject(req.error || new Error('sin almacenamiento')); };
    });
  }

  function conTienda(modo, fn) {
    return abrirBD().then(function (bd) {
      return new Promise(function (resolve, reject) {
        var tx = bd.transaction('pendientes', modo);
        var pedido = fn(tx.objectStore('pendientes'));
        tx.oncomplete = function () { resolve(pedido && pedido.result); };
        tx.onerror = function () { reject(tx.error); };
        tx.onabort = function () { reject(tx.error); };
      });
    });
  }

  function encolar(producto) { return conTienda('readwrite', function (t) { return t.put(producto); }); }
  function sacarDeLaCola(id) { return conTienda('readwrite', function (t) { return t.delete(id); }); }
  function verCola() {
    return conTienda('readonly', function (t) { return t.getAll(); }).then(function (cola) {
      // `getAll` devuelve por clave, y la clave es un identificador al azar: sin
      // ordenar, lo capturado sale en desorden y los códigos de producto no
      // siguen el orden en que se fotografió la mercancía.
      return (cola || []).sort(function (a, b) {
        return (a.capturado - b.capturado) || (a.id < b.id ? -1 : 1);
      });
    });
  }

  function idNuevo() {
    if (window.crypto && crypto.randomUUID) return crypto.randomUUID();
    // Los navegadores viejos no lo traen; con la hora y dos números al azar
    // basta para que dos capturas del mismo teléfono no choquen.
    return 'c-' + Date.now().toString(36) + '-' +
           Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2);
  }

  var sincronizando = false;
  var faltaOtraVuelta = false;

  /**
   * Manda lo que haya en la cola. Devuelve cuántos quedaron sin mandar.
   *
   * Solo corre una a la vez, pero si llega otra petición mientras tanto no se
   * descarta: se apunta y se da otra vuelta al terminar. Descartarla dejaba sin
   * mandar lo capturado cuando dos prendas se guardaban una detrás de otra, que
   * es exactamente el ritmo al que se trabaja.
   */
  async function sincronizar(silencioso) {
    if (sincronizando) { faltaOtraVuelta = true; return -1; }
    sincronizando = true;
    try {
      var cola = await verCola();
      if (!cola.length) { pintarPendientes(0); return 0; }

      var enviados = 0;
      var ultimoError = '';
      for (var i = 0; i < cola.length; i++) {
        var item = cola[i];
        try {
          var esConteo = item.tipo === 'conteo';
          var r = await fetch(esConteo ? '/api/conteo' : '/api/producto', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json', 'X-Codigo': codigo },
            body: JSON.stringify(item.datos)
          });
          var data = await r.json();
          if (r.ok && data.ok) {
            // Solo entonces sale de la cola. Si el teléfono se apaga entre la
            // respuesta y esta línea, el reintento no duplica: el identificador
            // ya es conocido por la caja.
            await sacarDeLaCola(item.id);
            if (esConteo) {
              var c = data.conteo || {};
              agregarAlHistorial(
                String(c.despues),
                c.etiqueta || item.datos.sku,
                c.movido_mientras
              );
            } else {
              agregarAlHistorial(data.sku, item.datos.nombre);
            }
            enviados++;
          } else {
            // La caja contestó que no. Reintentar no va a cambiar nada: se
            // queda en la cola y se avisa, para que no desaparezca en silencio.
            ultimoError = data.mensaje || 'La caja no lo aceptó.';
            break;
          }
        } catch (err) {
          // Sin conexión. Se queda todo como está y se intenta más tarde.
          break;
        }
      }

      var quedan = (await verCola()).length;
      pintarPendientes(quedan);

      if (ultimoError) {
        aviso(ultimoError, 'bad');
      } else if (enviados > 0) {
        aviso(enviados === 1
          ? 'Se mandó 1 a la caja.'
          : 'Se mandaron ' + enviados + ' a la caja.', 'ok');
      } else if (quedan > 0 && !silencioso) {
        aviso('La caja no contesta. Lo capturado está guardado aquí y se manda solo cuando la prendan.', 'bad');
      }
      return quedan;
    } finally {
      sincronizando = false;
      if (faltaOtraVuelta) {
        faltaOtraVuelta = false;
        await sincronizar(true);
      }
    }
  }

  function pintarPendientes(cuantos) {
    var caja = $('pendientes');
    if (!cuantos) { caja.style.display = 'none'; return; }
    caja.style.display = 'block';
    // "Cosas" y no "productos": aquí caben altas y conteos.
    $('pendientesTexto').textContent = cuantos === 1
      ? 'Falta 1 por mandar a la computadora'
      : 'Faltan ' + cuantos + ' por mandar a la computadora';
  }


  // ── Contar la mercancía ───────────────────────────────────────────────────
  //
  // El catálogo se descarga cuando hay caja y se guarda en el teléfono, para
  // poder buscar una prenda caminando la tienda con la computadora apagada. Solo
  // trae lo justo para reconocerla: nombre, código y cuántas dice la caja que
  // hay. Ni precios de compra ni ventas.
  //
  // Lo contado va a la misma cola que las altas y se manda igual. Lo que la
  // vuelve segura es que se apunta *cuándo* se contó: la caja le suma lo que se
  // vendió después, así que contar por la mañana y sincronizar por la tarde no
  // deshace la venta del día.

  var catalogo = [];
  var contando = null;

  function guardarCatalogo(productos) {
    catalogo = productos;
    try {
      localStorage.setItem('catalogo', JSON.stringify({ cuando: Date.now(), productos: productos }));
    } catch (e) { /* si no cabe, se sigue con el de esta sesión */ }
  }

  function cargarCatalogoGuardado() {
    try {
      var crudo = localStorage.getItem('catalogo');
      if (!crudo) return null;
      return JSON.parse(crudo);
    } catch (e) { return null; }
  }

  function pintarEstadoCatalogo(guardado, recienBajado) {
    var el = $('estadoCatalogo');
    if (!catalogo.length) {
      el.textContent = 'Todavía no hay catálogo en este teléfono. Conéctate una vez con la computadora prendida y se descarga solo.';
      return;
    }
    if (recienBajado) {
      el.textContent = catalogo.length + ' prendas, recién actualizadas.';
      return;
    }
    var dias = guardado ? Math.floor((Date.now() - guardado.cuando) / 86400000) : 0;
    el.textContent = catalogo.length + ' prendas' +
      (dias >= 1 ? ' (lista de hace ' + dias + (dias === 1 ? ' día' : ' días') + ')' : '');
  }

  function refrescarCatalogo() {
    return fetch('/api/catalogo', { headers: { 'X-Codigo': codigo } })
      .then(function (r) { return r.json(); })
      .then(function (d) {
        if (!d.ok || !d.productos) return false;
        guardarCatalogo(d.productos);
        pintarEstadoCatalogo(null, true);
        return true;
      })
      .catch(function () { return false; });
  }

  function buscarPrendas(texto) {
    var q = texto.trim().toLowerCase();
    if (q.length < 2) return [];
    return catalogo.filter(function (p) {
      return p.nombre.toLowerCase().indexOf(q) >= 0 || p.sku.toLowerCase().indexOf(q) >= 0;
    }).slice(0, 12);
  }

  $('buscar').addEventListener('input', function () {
    var encontradas = buscarPrendas(this.value);
    var caja = $('tarjetaResultados');
    if (!encontradas.length) { caja.style.display = 'none'; return; }
    caja.style.display = 'block';
    $('resultados').innerHTML = encontradas.map(function (p, i) {
      return '<div class="renglon"><div class="que">' +
             '<b>' + escapar(p.nombre) + '</b>' +
             '<span>' + escapar(p.sku) + ' · ' + p.stock + ' según la caja</span>' +
             '</div><button type="button" class="sec" data-i="' + i + '">Contar</button></div>';
    }).join('');
    $('resultados').dataset.encontradas = JSON.stringify(encontradas);
  });

  $('resultados').addEventListener('click', function (e) {
    var b = e.target.closest('button[data-i]');
    if (!b) return;
    var encontradas = JSON.parse(this.dataset.encontradas || '[]');
    abrirConteo(encontradas[Number(b.dataset.i)]);
  });

  function abrirConteo(prenda) {
    if (!prenda) return;
    contando = prenda;
    $('contandoQue').textContent = prenda.nombre;
    var lineas = (prenda.variantes && prenda.variantes.length)
      ? prenda.variantes
      : [{ id: null, etiqueta: 'Todas', stock: prenda.stock }];

    $('lineasConteo').innerHTML = lineas.map(function (v, i) {
      return '<div class="renglon"><div class="que">' +
             '<b>' + escapar(v.etiqueta || 'Todas') + '</b>' +
             '<span>la caja dice ' + v.stock + '</span>' +
             '</div><input type="number" inputmode="numeric" min="0" step="1" ' +
             'placeholder="0" data-i="' + i + '"></div>';
    }).join('');
    $('lineasConteo').dataset.lineas = JSON.stringify(lineas);
    $('tarjetaContar').style.display = 'block';
    $('tarjetaResultados').style.display = 'none';
    $('tarjetaContar').scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  function cerrarConteo() {
    contando = null;
    $('tarjetaContar').style.display = 'none';
    $('buscar').value = '';
    $('tarjetaResultados').style.display = 'none';
  }

  $('cancelarConteo').addEventListener('click', cerrarConteo);

  $('guardarConteo').addEventListener('click', async function () {
    if (!contando) return;
    var lineas = JSON.parse($('lineasConteo').dataset.lineas || '[]');
    var casillas = $('lineasConteo').querySelectorAll('input');
    // La hora en que se contó, que es lo que permite a la caja respetar lo que
    // se venda de aquí a que reciba esto.
    var ahora = new Date();
    var cuando = ahora.getFullYear() + '-' +
      String(ahora.getMonth() + 1).padStart(2, '0') + '-' +
      String(ahora.getDate()).padStart(2, '0') + ' ' +
      String(ahora.getHours()).padStart(2, '0') + ':' +
      String(ahora.getMinutes()).padStart(2, '0') + ':' +
      String(ahora.getSeconds()).padStart(2, '0');

    var puestos = 0;
    for (var i = 0; i < casillas.length; i++) {
      var crudo = casillas[i].value.trim();
      // Una casilla vacía no es un cero: es "esta no la conté".
      if (crudo === '') continue;
      var cantidad = parseInt(crudo, 10);
      if (!isFinite(cantidad) || cantidad < 0) continue;

      var id = idNuevo();
      await encolar({
        id: id,
        capturado: Date.now() + i,
        tipo: 'conteo',
        datos: {
          conteo_id: id,
          sku: contando.sku,
          variant_id: lineas[i].id,
          contado: cantidad,
          contado_en: cuando
        }
      });
      puestos++;
    }

    if (!puestos) { aviso('No anotaste ninguna cantidad.', 'bad'); return; }
    cerrarConteo();
    aviso(puestos === 1 ? 'Conteo guardado.' : puestos + ' conteos guardados.', 'ok');
    sincronizar(true);
    // El resultado se apunta en la lista de la otra pantalla, que es donde se
    // ve el historial de lo que se ha mandado.
  });

  function verModo(cual) {
    var esAlta = cual === 'alta';
    $('pantallaAlta').style.display = esAlta ? '' : 'none';
    $('pantallaConteo').style.display = esAlta ? 'none' : '';
    $('modoAlta').setAttribute('aria-selected', String(esAlta));
    $('modoConteo').setAttribute('aria-selected', String(!esAlta));
    $('titulo').textContent = esAlta ? 'Capturar producto' : 'Contar mercancía';
    $('subtitulo').textContent = esAlta
      ? 'Se guarda directo en la computadora de la tienda.'
      : 'Anota lo que ves en el perchero. Se manda cuando prendas la computadora.';
    if (!esAlta) {
      var guardado = cargarCatalogoGuardado();
      if (guardado && guardado.productos) catalogo = guardado.productos;
      pintarEstadoCatalogo(guardado, false);
      refrescarCatalogo();
    }
  }
  window.verModo = verModo;

  $('guardar').addEventListener('click', async function () {
    if (enviando) return;
    var nombre = $('nombre').value.trim();
    if (!nombre) { aviso('Ponle nombre al producto.', 'bad'); $('nombre').focus(); return; }

    enviando = true;
    $('guardar').disabled = true;
    $('guardar').textContent = 'Guardando...';
    aviso('', '');

    var producto = {
      id: idNuevo(),
      capturado: Date.now(),
      datos: {
        captura_id: null,
        nombre: nombre,
        precio: parseFloat($('precio').value) || null,
        existencia: parseInt($('existencia').value, 10) || 0,
        notas: $('notas').value.trim() || null,
        tallas: tallas.slice(),
        colores: colores.slice(),
        // Solo cuando hubo algo que repartir. Con una sola combinación el
        // reparto está vacío y mandarlo pondría cero piezas, ignorando las
        // que se pusieron arriba.
        piezas: piezasCapturadas(),
        fotos: fotos.slice()
      }
    };
    producto.datos.captura_id = producto.id;

    try {
      // Primero al teléfono. Aunque la caja esté encendida: si se guarda aquí,
      // ya no se puede perder pase lo que pase con el WiFi.
      await encolar(producto);
    } catch (err) {
      aviso('El teléfono no dejó guardar la captura. Revisa que le quede espacio.', 'bad');
      soltarBoton();
      return;
    }

    // En cuanto está a salvo se libera el botón, antes de contar la cola y antes
    // de intentar mandarla. Dejarlo bloqueado durante esas dos esperas hacía que
    // el siguiente toque no hiciera nada: quien captura rápido perdía la prenda
    // sin que nada se lo dijera.
    limpiar(true);
    soltarBoton();

    // El contador lo pinta solo `sincronizar`, que lee la cola al terminar.
    // Contarla también aquí abría una carrera: la cuenta de antes de mandar
    // podía llegar después de la de después, y el aviso reaparecía con un
    // número viejo cuando ya no quedaba nada.
    sincronizar(true);
  });

  function soltarBoton() {
    enviando = false;
    $('guardar').disabled = false;
    $('guardar').textContent = 'Guardar producto';
  }

  $('sincronizar').addEventListener('click', function () { sincronizar(false); });

  // Se intenta cuando el teléfono recupera la red y al volver a la aplicación,
  // que son los dos momentos en que la caja pudo haberse encendido.
  window.addEventListener('online', function () { sincronizar(true); });
  document.addEventListener('visibilitychange', function () {
    if (!document.hidden) sincronizar(true);
  });

  // Avisa de entrada si el código ya no sirve, en vez de dejar que capture todo
  // un producto para descubrirlo al guardar.
  $('limpiarTodo').addEventListener('click', function () { limpiar(false); });

  // Guarda la aplicación en el teléfono. Es lo que la deja abrir con la
  // computadora apagada; si el navegador no lo permite, todo lo demás sigue
  // funcionando mientras haya conexión.
  if ('serviceWorker' in navigator) {
    navigator.serviceWorker.register('/sw.js').catch(function (e) {
      console.warn('Sin guardado sin conexión:', e);
    });
  }

  fetch('/api/verificar', { headers: { 'X-Codigo': codigo } })
    .then(function (r) { return r.json().then(function (d) { return { ok: r.ok, d: d }; }); })
    .then(function (res) {
      if (!res.ok) aviso(res.d.mensaje + ' Vuelve a apuntar la cámara al código.', 'bad');
    })
    .catch(function () {
      // Sin caja no se avisa de nada: capturar sin conexión es lo normal ahora,
      // y lo pendiente ya se ve en su propio recuadro.
    });

  pintarGaleria();
  pintarResumen();
  // Lo que quedó de la última vez: se enseña y se intenta mandar.
  verCola()
    .then(function (cola) {
      pintarPendientes(cola.length);
      if (cola.length) sincronizar(true);
    })
    .catch(function () { /* sin almacenamiento: se sigue igual, solo en línea */ });
})();
</script>
</body>
</html>
"####;
