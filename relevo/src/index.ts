/**
 * Relevo de capturas entre el celular y el punto de venta.
 *
 * Existe por una asimetría de red que no se puede arreglar con código: desde la
 * tienda **se sale** a internet sin problema —la aplicación se actualiza sola—
 * pero **no se entra**. El router aísla a los clientes entre sí, o el teléfono y
 * la computadora están en redes distintas, y ningún permiso de firewall lo
 * cambia. Un buzón al que los dos lados llegan por su cuenta sí funciona.
 *
 * Lo que se guarda aquí está **de paso**. El punto de venta se lo lleva y avisa,
 * y entonces se borra. Si nadie lo recoge, caduca solo: el relevo no puede
 * convertirse en un archivo permanente de las fotos de la tienda ni queriendo.
 *
 * No pasa por aquí nada de ventas, clientes, caja ni inventario. Solo lo que se
 * capturó con el teléfono y todavía no llega a su casa.
 */

interface Env {
	CAPTURAS: KVNamespace;
}

/** Cuánto aguanta una captura sin que nadie la recoja. */
const CADUCIDAD_SEGUNDOS = 30 * 24 * 60 * 60;

/** Cuánto aguanta el catálogo publicado sin que la tienda lo renueve. */
const CADUCIDAD_CATALOGO_SEGUNDOS = 60 * 24 * 60 * 60;

/** Tope de una captura: un producto con sus fotos ronda el medio mega. */
const MAX_BYTES = 8 * 1024 * 1024;

/** Tope del catálogo. Sin fotos, cinco mil prendas caben en menos de uno. */
const MAX_BYTES_CATALOGO = 5 * 1024 * 1024;

const json = (cuerpo: unknown, status = 200): Response =>
	new Response(JSON.stringify(cuerpo), {
		status,
		headers: {
			'content-type': 'application/json; charset=utf-8',
			// Nada de esto debe quedar en ninguna caché intermedia.
			'cache-control': 'no-store',
		},
	});

const error = (status: number, mensaje: string): Response => json({ ok: false, mensaje }, status);

/**
 * La carpeta de una tienda, derivada del secreto que trae la petición.
 *
 * El secreto **no se guarda en ningún lado**: se convierte en su huella y esa es
 * la que nombra la carpeta. Quien no lo tenga no puede ni leer ni escribir, y
 * quien vea este código o el contenido de KV tampoco puede deducirlo.
 */
async function huellaDe(secreto: string): Promise<string> {
	const bytes = new TextEncoder().encode(secreto);
	const huella = await crypto.subtle.digest('SHA-256', bytes);
	return [...new Uint8Array(huella)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

/** El secreto que trae la petición, o null si no trae uno usable. */
function secretoDe(req: Request): string | null {
	const cabecera = req.headers.get('authorization') ?? '';
	const [tipo, valor] = cabecera.split(' ');
	if (tipo?.toLowerCase() !== 'bearer' || !valor) return null;
	// Un secreto corto sería una contraseña, y esto está expuesto a internet.
	// El punto de venta emite 32 bytes al azar; se rechaza cualquier cosa menor.
	if (valor.length < 40) return null;
	return valor;
}

export default {
	async fetch(req: Request, env: Env): Promise<Response> {
		const url = new URL(req.url);
		const ruta = url.pathname;

		// Comprobación de vida. Sin secreto a propósito: sirve para que el punto
		// de venta distinga "el relevo no responde" de "mi secreto no sirve".
		if (ruta === '/api/salud') {
			return json({ ok: true, servicio: 'relevo de capturas' });
		}

		const secreto = secretoDe(req);
		if (!secreto) return error(401, 'Falta el código de emparejamiento');
		const huella = await huellaDe(secreto);
		// Las capturas y el catálogo viven en prefijos distintos: así el listado
		// de lo pendiente nunca incluye el catálogo por accidente.
		const carpeta = `t/${huella}`;
		const claveCatalogo = `c/${huella}`;

		// Que el secreto sirve solo se puede comprobar usándolo: aquí no hay
		// registro de tiendas ni lista de secretos válidos. Cualquier secreto
		// tiene su propia carpeta, y sin el correcto no se ve la de nadie más.
		if (ruta === '/api/verificar') {
			return json({ ok: true });
		}

		if (ruta === '/api/subir' && req.method === 'POST') {
			const largo = Number(req.headers.get('content-length') ?? '0');
			if (largo > MAX_BYTES) return error(413, 'La captura es demasiado grande');

			let entrada: { captura_id?: string; tipo?: string; datos?: unknown };
			try {
				entrada = await req.json();
			} catch {
				return error(400, 'El cuerpo no es JSON válido');
			}

			const id = String(entrada.captura_id ?? '').trim();
			// El identificador lo pone el teléfono al capturar, no al mandar: es lo
			// que hace que reintentar no duplique, igual que en la red local.
			if (!id || id.length > 120 || !/^[A-Za-z0-9_-]+$/.test(id)) {
				return error(400, 'Identificador de captura inválido');
			}
			const tipo = entrada.tipo === 'conteo' ? 'conteo' : 'producto';
			if (entrada.datos === undefined) return error(400, 'La captura viene vacía');

			const clave = `${carpeta}/${id}`;
			// Antes de subir se contaban las pendientes con un listado, para frenar
			// a un teléfono en bucle. Cada listado gasta de un cupo diario pequeño
			// que el punto de venta necesita para recoger; el freno ya lo pone el
			// cupo de escrituras, y la cola del teléfono no repite identificadores.
			const yaEstaba = await env.CAPTURAS.get(clave, 'stream');

			await env.CAPTURAS.put(clave, JSON.stringify({ tipo, datos: entrada.datos }), {
				expirationTtl: CADUCIDAD_SEGUNDOS,
				metadata: { tipo, cuando: new Date().toISOString() },
			});

			return json({ ok: true, captura_id: id, repetida: Boolean(yaEstaba) });
		}

		// El catálogo para contar: nombres, códigos y tallas, sin existencias.
		//
		// Sin existencias a propósito. Lo que la caja cree que hay es inventario,
		// y el inventario no sale de la tienda: el conteo funciona igual sin él,
		// porque quien está frente al perchero ve la verdad y la caja le suma
		// después lo que se vendió mientras tanto.
		if (ruta === '/api/catalogo' && req.method === 'PUT') {
			const largo = Number(req.headers.get('content-length') ?? '0');
			if (largo > MAX_BYTES_CATALOGO) return error(413, 'El catálogo es demasiado grande');

			let entrada: { productos?: unknown };
			try {
				entrada = await req.json();
			} catch {
				return error(400, 'El cuerpo no es JSON válido');
			}
			if (!Array.isArray(entrada.productos)) return error(400, 'Falta la lista de productos');

			const cuando = new Date().toISOString();
			await env.CAPTURAS.put(claveCatalogo, JSON.stringify({ productos: entrada.productos, cuando }), {
				expirationTtl: CADUCIDAD_CATALOGO_SEGUNDOS,
			});
			return json({ ok: true, productos: entrada.productos.length, cuando });
		}

		if (ruta === '/api/catalogo' && req.method === 'GET') {
			const guardado = await env.CAPTURAS.get<{ productos: unknown[]; cuando: string }>(claveCatalogo, 'json');
			return json({ ok: true, productos: guardado?.productos ?? [], cuando: guardado?.cuando ?? null });
		}

		// Lo que el punto de venta todavía no se ha llevado, sin el contenido:
		// así puede enseñar cuántas hay sin bajarse los megas de las fotos.
		if (ruta === '/api/pendientes' && req.method === 'GET') {
			const { keys, list_complete, cursor } = await env.CAPTURAS.list({
				prefix: `${carpeta}/`,
				limit: 200,
				cursor: url.searchParams.get('cursor') ?? undefined,
			});
			return json({
				ok: true,
				pendientes: keys.map((k) => ({
					captura_id: k.name.slice(carpeta.length + 1),
					...(k.metadata as Record<string, unknown> | undefined),
				})),
				completo: list_complete,
				cursor: list_complete ? null : cursor,
			});
		}

		if (ruta.startsWith('/api/pendiente/') && req.method === 'GET') {
			const id = decodeURIComponent(ruta.slice('/api/pendiente/'.length));
			if (!id || id.includes('/')) return error(400, 'Identificador inválido');
			const cuerpo = await env.CAPTURAS.get(`${carpeta}/${id}`);
			if (cuerpo === null) return error(404, 'Esa captura ya no está');
			return new Response(cuerpo, {
				headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' },
			});
		}

		// El punto de venta confirma que ya la tiene en su base y aquí se borra.
		// El borrado va **después** de que se guardó del otro lado, nunca antes:
		// entre el relevo y la tienda, la copia que importa es la de la tienda.
		if (ruta === '/api/recibido' && req.method === 'POST') {
			let entrada: { ids?: unknown };
			try {
				entrada = await req.json();
			} catch {
				return error(400, 'El cuerpo no es JSON válido');
			}
			const ids = Array.isArray(entrada.ids) ? entrada.ids : [];
			if (ids.length === 0) return error(400, 'No dijiste qué borrar');
			if (ids.length > 200) return error(400, 'Demasiadas de una vez');

			let borradas = 0;
			for (const bruto of ids) {
				const id = String(bruto);
				if (!id || id.includes('/')) continue;
				await env.CAPTURAS.delete(`${carpeta}/${id}`);
				borradas++;
			}
			return json({ ok: true, borradas });
		}

		return error(404, 'Esa ruta no existe');
	},
} satisfies ExportedHandler<Env>;
