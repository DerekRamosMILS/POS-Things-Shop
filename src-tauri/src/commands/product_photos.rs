//! Fotos de producto: alta, consulta, orden y borrado.
//!
//! Las fotos viven dentro de la base, en la misma fila que las nombra. Es más
//! pesado que dejarlas en archivos, y a cambio son imposibles de perder: entran
//! y salen con la misma transacción que su producto, un respaldo se las lleva
//! siempre, y no queda ningún código que borre archivos por su cuenta.
//!
//! Los listados solo piden miniaturas; la foto a resolución de catálogo se carga
//! cuando alguien abre la ficha. Así el punto de venta sigue siendo ligero
//! aunque el catálogo esté fotografiado entero.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::connection::DbState;
use crate::photos;
use crate::session::{require_admin, require_auth, SessionState};

/// Tope de fotos por producto. Suficiente para frente, espalda y detalles, sin
/// dejar que un producto se coma el disco.
pub const MAX_POR_PRODUCTO: i64 = 8;

#[derive(Debug, Serialize, Clone)]
pub struct ProductImage {
    pub id: i64,
    pub product_id: i64,
    pub position: i32,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct NuevaFotoDto {
    pub product_id: i64,
    /// Foto a resolución de catálogo, como data URL JPEG.
    pub photo: String,
    /// Miniatura para los listados, como data URL JPEG.
    pub thumbnail: String,
}

/// Registra una foto: escribe los archivos y deja la fila que los referencia.
///
/// Se usa tanto desde la app de escritorio como desde la captura por celular.
pub fn agregar_foto(
    db: &rusqlite::Connection,
    data: &NuevaFotoDto,
) -> Result<ProductImage, String> {
    let existentes: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM product_images WHERE product_id = ?1",
            params![data.product_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if existentes >= MAX_POR_PRODUCTO {
        return Err(format!("Un producto admite hasta {} fotos", MAX_POR_PRODUCTO));
    }

    let existe: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM products WHERE id = ?1",
            params![data.product_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if existe == 0 {
        return Err("El producto no existe".to_string());
    }

    let foto = photos::bytes_de_data_url(&data.photo)?;
    let miniatura = photos::bytes_de_data_url(&data.thumbnail)?;
    photos::validar(&foto, &miniatura)?;

    // La primera foto de un producto queda como principal.
    let posicion = existentes as i32;

    // Los bytes van en la fila. No hay archivo suelto que pueda quedar sin
    // dueño ni fila que pueda quedar apuntando a un archivo que ya no está:
    // o entra todo o no entra nada.
    db.execute(
        "INSERT INTO product_images (product_id, file_name, thumb_name, position, photo, thumbnail, en_la_base)
         VALUES (?1, '', '', ?2, ?3, ?4, 1)",
        params![data.product_id, posicion, foto, miniatura],
    ).map_err(|e| e.to_string())?;

    let id = db.last_insert_rowid();
    db.execute(
        "UPDATE products SET updated_at = datetime('now','localtime') WHERE id = ?1",
        params![data.product_id],
    ).ok();

    Ok(ProductImage { id, product_id: data.product_id, position: posicion, created_at: String::new() })
}

#[tauri::command]
pub fn add_product_image(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    data: NuevaFotoDto,
) -> Result<ProductImage, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();
    agregar_foto(&db, &data)
}

/// Fotos de un producto, sin sus bytes: solo los identificadores y el orden.
#[tauri::command]
pub fn get_product_image_list(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    product_id: i64,
) -> Result<Vec<ProductImage>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

    let mut stmt = db
        .prepare(
            "SELECT id, product_id, position, created_at FROM product_images
             WHERE product_id = ?1 ORDER BY position, id",
        )
        .map_err(|e| e.to_string())?;

    let out = stmt
        .query_map(params![product_id], |r| {
            Ok(ProductImage {
                id: r.get(0)?,
                product_id: r.get(1)?,
                position: r.get(2)?,
                created_at: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(out)
}

/// Una foto a resolución completa, para la ficha del producto.
#[tauri::command]
pub fn get_product_photo(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    image_id: i64,
) -> Result<String, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

    leer_foto(&db, image_id)
}

/// La foto grande de una fila, como data URL.
fn leer_foto(db: &rusqlite::Connection, image_id: i64) -> Result<String, String> {
    let (bytes, file_name): (Option<Vec<u8>>, String) = db
        .query_row(
            "SELECT photo, file_name FROM product_images WHERE id = ?1",
            params![image_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "La foto no existe".to_string())?;

    match bytes {
        Some(b) if !b.is_empty() => Ok(photos::como_data_url(&b)),
        // Una fila que todavía no se ha pasado a la base: se lee su archivo.
        _ => photos::leer_como_data_url(&file_name),
    }
}

#[tauri::command]
pub fn delete_product_image(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    image_id: i64,
) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    let product_id: i64 = db
        .query_row(
            "SELECT product_id FROM product_images WHERE id = ?1",
            params![image_id],
            |r| r.get(0),
        )
        .map_err(|_| "La foto no existe".to_string())?;

    db.execute("DELETE FROM product_images WHERE id = ?1", params![image_id])
        .map_err(|e| e.to_string())?;

    // Renumerar para que no queden huecos y la primera siga siendo la principal.
    renumerar(&db, product_id)?;
    Ok(())
}

/// Cambia el orden; la primera de la lista pasa a ser la foto principal.
#[tauri::command]
pub fn reorder_product_images(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    product_id: i64,
    image_ids: Vec<i64>,
) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    for (posicion, id) in image_ids.iter().enumerate() {
        db.execute(
            "UPDATE product_images SET position = ?1 WHERE id = ?2 AND product_id = ?3",
            params![posicion as i32, id, product_id],
        )
        .map_err(|e| e.to_string())?;
    }
    renumerar(&db, product_id)
}

fn renumerar(db: &rusqlite::Connection, product_id: i64) -> Result<(), String> {
    let ids: Vec<i64> = db
        .prepare("SELECT id FROM product_images WHERE product_id = ?1 ORDER BY position, id")
        .and_then(|mut s| {
            s.query_map(params![product_id], |r| r.get(0))?
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|e| e.to_string())?;

    for (posicion, id) in ids.iter().enumerate() {
        db.execute(
            "UPDATE product_images SET position = ?1 WHERE id = ?2",
            params![posicion as i32, id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Miniaturas de los productos visibles, por lote.
///
/// Sustituye al comando anterior que devolvía las fotos incrustadas en la fila.
#[tauri::command]
pub fn get_product_images(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    product_ids: Vec<i64>,
) -> Result<Vec<(i64, String)>, String> {
    require_auth(&sessions, &token)?;
    if product_ids.is_empty() {
        return Ok(Vec::new());
    }
    if product_ids.len() > 60 {
        return Err("Demasiadas imágenes en una sola petición".to_string());
    }

    let db = state.conn();
    let marcadores = vec!["?"; product_ids.len()].join(",");

    // Solo la principal de cada producto: los listados muestran una.
    let mut stmt = db
        .prepare(&format!(
            "SELECT product_id, thumbnail, thumb_name FROM product_images
             WHERE product_id IN ({}) AND position = 0",
            marcadores
        ))
        .map_err(|e| e.to_string())?;

    let filas: Vec<(i64, Option<Vec<u8>>, String)> = stmt
        .query_map(rusqlite::params_from_iter(product_ids.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    // Una miniatura ilegible no debe tumbar el catálogo entero.
    Ok(filas
        .into_iter()
        .filter_map(|(id, bytes, nombre)| match bytes {
            Some(b) if !b.is_empty() => Some((id, photos::como_data_url(&b))),
            _ => photos::leer_como_data_url(&nombre).ok().map(|url| (id, url)),
        })
        .collect())
}

/// Mete a la base las fotos que quedaron incrustadas en filas antiguas.
///
/// Se ejecuta una vez al arrancar. Conserva la columna original hasta confirmar
/// que la copia quedó bien, para que un fallo a medias no borre nada.
pub fn migrar_fotos_incrustadas(db: &rusqlite::Connection) {
    let pendientes: Vec<(i64, String)> = match db.prepare(
        "SELECT id, image_url FROM products
         WHERE images_migradas = 0 AND image_url IS NOT NULL AND image_url != ''",
    ) {
        Ok(mut stmt) => match stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))) {
            Ok(rows) => rows.flatten().collect(),
            Err(e) => {
                log::warn!("No se pudieron leer las fotos por migrar: {}", e);
                return;
            }
        },
        Err(e) => {
            log::warn!("No se pudieron leer las fotos por migrar: {}", e);
            return;
        }
    };

    if pendientes.is_empty() {
        return;
    }

    let mut migradas = 0;
    for (product_id, data_url) in pendientes {
        // La foto antigua ya venía reducida: sirve igual como original y como
        // miniatura hasta que alguien la reemplace por una mejor.
        let dto = NuevaFotoDto {
            product_id,
            photo: data_url.clone(),
            thumbnail: data_url,
        };
        match agregar_foto(db, &dto) {
            Ok(_) => {
                db.execute(
                    "UPDATE products SET images_migradas = 1 WHERE id = ?1",
                    params![product_id],
                ).ok();
                migradas += 1;
            }
            Err(e) => log::warn!("No se pudo migrar la foto del producto {}: {}", product_id, e),
        }
    }

    if migradas > 0 {
        log::info!("Fotos antiguas incorporadas a la base: {}", migradas);
    }
}

/// Mete a la base las fotos que la 018 dejó como archivos en `fotos/`.
///
/// Se ejecuta al arrancar hasta que no quede ninguna. El archivo no se borra:
/// queda de respaldo por si algo saliera mal, y la carpeta se puede tirar a
/// mano cuando la tienda esté tranquila. Perder una foto es peor que dejar unos
/// megabytes ocupados.
pub fn incorporar_fotos_en_archivos(db: &rusqlite::Connection) {
    let pendientes: Vec<(i64, String, String)> = match db.prepare(
        "SELECT id, file_name, thumb_name FROM product_images
         WHERE en_la_base = 0 AND file_name != ''",
    ) {
        Ok(mut stmt) => match stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))) {
            Ok(rows) => rows.flatten().collect(),
            Err(e) => {
                log::warn!("No se pudieron leer las fotos por incorporar: {}", e);
                return;
            }
        },
        Err(e) => {
            log::warn!("No se pudieron leer las fotos por incorporar: {}", e);
            return;
        }
    };

    if pendientes.is_empty() {
        return;
    }

    let mut hechas = 0;
    let mut faltantes = 0;
    for (id, file_name, thumb_name) in pendientes {
        let (Ok(foto), Ok(miniatura)) = (photos::leer(&file_name), photos::leer(&thumb_name)) else {
            faltantes += 1;
            log::warn!("La foto {} no se encontró en disco; la fila se conserva", id);
            continue;
        };

        match db.execute(
            "UPDATE product_images SET photo = ?1, thumbnail = ?2, en_la_base = 1 WHERE id = ?3",
            params![foto, miniatura, id],
        ) {
            Ok(_) => hechas += 1,
            Err(e) => log::warn!("No se pudo incorporar la foto {}: {}", id, e),
        }
    }

    if hechas > 0 {
        log::info!("Fotos incorporadas a la base: {}", hechas);
    }
    if faltantes > 0 {
        log::warn!("Fotos que ya no estaban en disco: {}", faltantes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tienda() -> rusqlite::Connection {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        db.execute(
            "INSERT INTO products (id, sku, name, purchase_price, sale_price, stock)
             VALUES (1, 'CAM', 'Camisa', 10.0, 20.0, 5)", [],
        ).unwrap();
        db
    }

    /// JPEG mínimo válido: la firma es lo que se comprueba.
    fn jpeg() -> String {
        format!("data:image/jpeg;base64,{}", photos::base64_encode(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]))
    }

    fn foto(product_id: i64) -> NuevaFotoDto {
        NuevaFotoDto { product_id, photo: jpeg(), thumbnail: jpeg() }
    }

    #[test]
    fn la_primera_foto_queda_como_principal() {
        let db = tienda();
        let img = agregar_foto(&db, &foto(1)).unwrap();
        assert_eq!(img.position, 0);
    }

    #[test]
    fn las_siguientes_se_acomodan_detras() {
        let db = tienda();
        agregar_foto(&db, &foto(1)).unwrap();
        let segunda = agregar_foto(&db, &foto(1)).unwrap();
        assert_eq!(segunda.position, 1);
    }

    #[test]
    fn la_foto_viaja_dentro_de_la_fila() {
        // Es lo que hace imposible perderla: no hay archivo aparte que pueda
        // quedarse sin dueño ni fila que pueda apuntar a un archivo que ya no
        // está. Un respaldo de la base se lleva las fotos.
        let db = tienda();
        let img = agregar_foto(&db, &foto(1)).unwrap();

        let (bytes, en_la_base): (Vec<u8>, i64) = db.query_row(
            "SELECT photo, en_la_base FROM product_images WHERE id = ?1",
            params![img.id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();

        assert!(crate::photos::es_jpeg(&bytes), "los bytes deben estar en la fila");
        assert_eq!(en_la_base, 1);
    }

    #[test]
    fn borrar_el_producto_se_lleva_sus_fotos_y_nada_mas() {
        let db = tienda();
        agregar_foto(&db, &foto(1)).unwrap();
        db.execute("DELETE FROM products WHERE id = 1", []).unwrap();

        let quedan: i64 = db.query_row(
            "SELECT COUNT(*) FROM product_images", [], |r| r.get(0)).unwrap();
        assert_eq!(quedan, 0, "la cascada se lleva la fila con sus bytes");
    }

    #[test]
    fn una_foto_de_archivo_se_incorpora_a_la_base() {
        // Las tiendas que ya venían de la versión anterior tienen sus fotos en
        // `fotos/`. Al arrancar se meten a la base sin borrar el archivo.
        let db = tienda();
        let bytes = [0xFFu8, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        let nombre = format!("{}.jpg", uuid::Uuid::new_v4());
        let thumb = nombre.replace(".jpg", "_t.jpg");
        std::fs::write(crate::photos::safe_path(&nombre).unwrap(), bytes).unwrap();
        std::fs::write(crate::photos::safe_path(&thumb).unwrap(), bytes).unwrap();

        db.execute(
            "INSERT INTO product_images (product_id, file_name, thumb_name, position, en_la_base)
             VALUES (1, ?1, ?2, 0, 0)",
            params![nombre, thumb],
        ).unwrap();

        incorporar_fotos_en_archivos(&db);

        let (guardados, en_la_base): (Vec<u8>, i64) = db.query_row(
            "SELECT photo, en_la_base FROM product_images WHERE file_name = ?1",
            params![nombre], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(guardados, bytes.to_vec());
        assert_eq!(en_la_base, 1);
        assert!(crate::photos::safe_path(&nombre).unwrap().exists(),
                "el archivo se conserva por si acaso");

        std::fs::remove_file(crate::photos::safe_path(&nombre).unwrap()).ok();
        std::fs::remove_file(crate::photos::safe_path(&thumb).unwrap()).ok();
    }

    #[test]
    fn no_se_aceptan_fotos_de_un_producto_inexistente() {
        let db = tienda();
        assert!(agregar_foto(&db, &foto(999)).is_err());
    }

    #[test]
    fn se_rechaza_lo_que_no_sea_jpeg() {
        let db = tienda();
        let png = format!("data:image/png;base64,{}", photos::base64_encode(&[0x89, 0x50, 0x4E, 0x47]));
        let dto = NuevaFotoDto { product_id: 1, photo: png.clone(), thumbnail: png };
        assert!(agregar_foto(&db, &dto).is_err());
    }

    #[test]
    fn hay_un_tope_de_fotos_por_producto() {
        let db = tienda();
        for _ in 0..MAX_POR_PRODUCTO {
            agregar_foto(&db, &foto(1)).unwrap();
        }
        assert!(agregar_foto(&db, &foto(1)).is_err());
    }

    #[test]
    fn una_foto_rechazada_no_deja_archivos_sueltos() {
        let db = tienda();
        let antes = std::fs::read_dir(photos::photos_dir()).map(|d| d.count()).unwrap_or(0);

        let dto = NuevaFotoDto {
            product_id: 1,
            photo: jpeg(),
            thumbnail: "data:image/jpeg;base64,bm8gc295IGpwZWc=".to_string(),
        };
        assert!(agregar_foto(&db, &dto).is_err());

        let despues = std::fs::read_dir(photos::photos_dir()).map(|d| d.count()).unwrap_or(0);
        assert_eq!(antes, despues, "no debe quedar basura en la carpeta");
    }

    #[test]
    fn las_fotos_incrustadas_antiguas_se_migran_a_archivos() {
        let db = tienda();
        db.execute("UPDATE products SET image_url = ?1 WHERE id = 1", params![jpeg()]).unwrap();

        migrar_fotos_incrustadas(&db);

        let cuantas: i64 = db.query_row(
            "SELECT COUNT(*) FROM product_images WHERE product_id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(cuantas, 1, "la foto vieja debe conservarse como archivo");

        let marcado: i64 = db.query_row(
            "SELECT images_migradas FROM products WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(marcado, 1);
    }

    #[test]
    fn migrar_dos_veces_no_duplica_las_fotos() {
        let db = tienda();
        db.execute("UPDATE products SET image_url = ?1 WHERE id = 1", params![jpeg()]).unwrap();

        migrar_fotos_incrustadas(&db);
        migrar_fotos_incrustadas(&db);

        let cuantas: i64 = db.query_row(
            "SELECT COUNT(*) FROM product_images WHERE product_id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(cuantas, 1);
    }

    #[test]
    fn borrar_una_foto_renumera_las_que_quedan() {
        let db = tienda();
        let a = agregar_foto(&db, &foto(1)).unwrap();
        agregar_foto(&db, &foto(1)).unwrap();

        db.execute("DELETE FROM product_images WHERE id = ?1", params![a.id]).unwrap();
        renumerar(&db, 1).unwrap();

        let posicion: i32 = db.query_row(
            "SELECT position FROM product_images WHERE product_id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(posicion, 0, "la que queda debe pasar a principal");
    }

    #[test]
    fn borrar_el_producto_se_lleva_sus_fotos() {
        let db = tienda();
        agregar_foto(&db, &foto(1)).unwrap();

        db.execute("DELETE FROM products WHERE id = 1", []).unwrap();

        let cuantas: i64 = db.query_row("SELECT COUNT(*) FROM product_images", [], |r| r.get(0)).unwrap();
        assert_eq!(cuantas, 0, "la cascada debe limpiar las filas");
    }
}
