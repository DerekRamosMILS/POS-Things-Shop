// Guardado de la página para que abra sin la computadora.
const CACHE = 'things-shop-captura-v5';
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
  'así que necesita internet.</p>' +
  '<p style="color:#6e6e76;font-size:13.5px;margin:0">Conéctate a internet, ' +
  'abre la captura y espera unos segundos. Cuando diga que está lista, ya va a abrir sin señal.</p>' +
  '</body></html>';

function paginaSinConexion() {
  return new Response(SIN_NADA, {
    status: 200,
    headers: { 'content-type': 'text/html; charset=utf-8' }
  });
}

self.addEventListener('install', (e) => {
  // Se guarda en cuanto se instala, sin esperar a nada: si el teléfono se sale
  // del WiFi en el siguiente minuto, ya está a salvo.
  //
  // Cada uno por separado y sin rendirse. Con un solo `addAll`, que fallara la
  // descarga de un icono hacía fracasar la instalación entera y el teléfono se
  // quedaba sin nada guardado.
  e.waitUntil((async () => {
    const c = await caches.open(CACHE);
    await Promise.all(TESOROS.map((ruta) => c.add(ruta).catch((err) => {
      console.warn('No se pudo guardar', ruta, err);
    })));
    await self.skipWaiting();
  })());
});

self.addEventListener('activate', (e) => {
  e.waitUntil((async () => {
    // Lo viejo solo se tira cuando lo nuevo ya sirve.
    //
    // Antes se borraba siempre, y combinado con una instalación que nunca falla
    // eso dejaba al teléfono sin NADA: si esta versión se instalaba sin alcanzar
    // la caja, su almacén quedaba vacío y aun así borraba el de la versión
    // anterior, que sí tenía la página. El teléfono perdía lo que ya funcionaba.
    const c = await caches.open(CACHE);
    const listo = await c.match(CONCHA, { ignoreSearch: true });
    if (listo) {
      const claves = await caches.keys();
      await Promise.all(claves.filter((k) => k !== CACHE).map((k) => caches.delete(k)));
    }
    await self.clients.claim();
  })());
});

/** La página guardada, sea de esta versión o de una anterior. */
async function conchaGuardada() {
  const propia = await caches.open(CACHE).then((c) => c.match(CONCHA, { ignoreSearch: true }));
  if (propia) return propia;
  for (const clave of await caches.keys()) {
    const vieja = await caches.open(clave).then((c) => c.match(CONCHA, { ignoreSearch: true }));
    if (vieja) return vieja;
  }
  return null;
}

self.addEventListener('fetch', (e) => {
  const url = new URL(e.request.url);

  // Lo que habla con la caja nunca se guarda. Una respuesta vieja aquí sería
  // peor que un error: el teléfono creería que mandó algo que no mandó.
  if (url.pathname.startsWith('/api/')) return;
  if (e.request.method !== 'GET') return;

  // La página: se intenta pedir fresca y se guarda; si no hay caja, la guardada.
  if (e.request.mode === 'navigate' || url.pathname === CONCHA) {
    e.respondWith((async () => {
      try {
        const r = await fetch(e.request);
        // Solo se guarda lo que salió bien. `cache.put` acepta un 404 o un 500
        // sin chistar, y una vez guardado eso se sirve para siempre: la página
        // quedaría rota sin forma de arreglarla desde el teléfono.
        if (r && r.ok) {
          const copia = r.clone();
          caches.open(CACHE).then((c) => c.put(CONCHA, copia)).catch(() => {});
        }
        return r;
      } catch (err) {
        return (await conchaGuardada()) || paginaSinConexion();
      }
    })());
    return;
  }

  e.respondWith(
    caches.match(e.request, { ignoreSearch: true })
      .then((g) => g || fetch(e.request))
      .catch(() => paginaSinConexion())
  );
});
