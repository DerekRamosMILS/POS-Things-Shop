//! Dar de alta lo que se capturó con el celular.
//!
//! Esto es lo que queda de la captura por red local cuando se quita la red
//! local: la parte que no sabía por dónde había llegado el producto. Tallas y
//! colores que se cruzan en variantes, piezas por combinación, fotos, y un
//! identificador que hace que reintentar no duplique. Antes lo llamaba el
//! servidor que escuchaba en el WiFi de la tienda; ahora lo llama el relevo, y
//! no le cambia nada.

use rusqlite::params;
use serde::Deserialize;

use crate::commands::product_photos::{agregar_foto, NuevaFotoDto, MAX_POR_PRODUCTO};
use crate::commands::products::siguiente_sku;

/// Tope de variantes que puede generar un cruce de tallas y colores.
const MAX_VARIANTES: usize = 60;

/// Da de alta un producto que llegó del celular, venga por donde venga.
///
/// Las comprobaciones que antes hacía el servidor antes de llamar a
/// `guardar_producto` viven aquí, para que no dependan del camino: un producto
/// sin nombre es un error igual si llegó por el WiFi que por la nube.
pub(crate) fn recibir_producto(
    db: &rusqlite::Connection,
    datos: serde_json::Value,
) -> Result<String, String> {
    let entrada: ProductoDelCelular = serde_json::from_value(datos)
        .map_err(|e| format!("La captura no tiene la forma esperada: {}", e))?;
    if entrada.nombre.trim().is_empty() {
        return Err("Ponle nombre al producto".to_string());
    }
    if entrada.fotos.len() > MAX_POR_PRODUCTO as usize {
        return Err("Demasiadas fotos".to_string());
    }
    guardar_producto(db, entrada)
}

/// Quita espacios, descarta vacíos y elimina repetidos conservando el orden.
pub(crate) fn normalizar(valores: &[String]) -> Vec<String> {
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
pub(crate) fn combinar(tallas: &[String], colores: &[String]) -> Vec<(Option<String>, Option<String>)> {
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

pub(crate) fn qr_svg(url: &str) -> Option<String> {
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

#[derive(Debug, Deserialize)]
pub(crate) struct ProductoDelCelular {
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

/// Da de alta el producto y sus fotos en una sola transacción.
///
/// Sin precio queda inactivo a propósito: así se puede fotografiar la mercancía
/// a un ritmo y ponerle precio a otro, sin que aparezca a medias en el punto de
/// venta.
pub(crate) fn guardar_producto(
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
    fn el_qr_se_genera_para_una_direccion_de_red() {
        let svg = qr_svg("https://things-shop-relevo.derek-papa.workers.dev/#k=un-secreto-de-prueba").unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.len() > 200);
    }

    #[test]
    fn el_qr_escala_a_su_contenedor_en_vez_de_recortarse() {
        // Con ancho y alto en píxeles, el código se cortaba dentro de un
        // recuadro más chico y dejaba de poder escanearse.
        let svg = qr_svg("https://things-shop-relevo.derek-papa.workers.dev/#k=un-secreto-de-prueba").unwrap();
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

    #[test]
    fn recibir_un_producto_sin_nombre_se_rechaza_con_mensaje() {
        let conn = db();
        let err = recibir_producto(&conn, serde_json::json!({ "nombre": "   " })).unwrap_err();
        assert!(err.contains("nombre"), "{}", err);
    }

    #[test]
    fn recibir_algo_con_otra_forma_se_rechaza_en_vez_de_reventar() {
        // Lo que llega por la nube pasó por un teléfono: puede venir de una
        // versión vieja de la página o simplemente mal. No puede tumbar nada.
        let conn = db();
        assert!(recibir_producto(&conn, serde_json::json!({ "nombre": 42 })).is_err());
        assert!(recibir_producto(&conn, serde_json::json!("ni siquiera es un objeto")).is_err());
    }

    #[test]
    fn recibir_un_producto_bien_formado_lo_da_de_alta() {
        let conn = db();
        let sku = recibir_producto(&conn, serde_json::json!({
            "captura_id": "de-la-nube-1", "nombre": "Vestido amarillo", "precio": 499.0
        })).unwrap();
        let nombre: String = conn
            .query_row("SELECT name FROM products WHERE sku = ?1", params![sku], |r| r.get(0))
            .unwrap();
        assert_eq!(nombre, "Vestido amarillo");
    }
}
