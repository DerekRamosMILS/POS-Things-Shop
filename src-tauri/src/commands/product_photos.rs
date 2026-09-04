//! Fotos de producto: alta, consulta, orden y borrado.
//!
//! Los listados solo piden miniaturas; la foto a resolución completa se carga
//! cuando alguien abre la ficha. Así el punto de venta sigue siendo ligero
//! aunque el catálogo esté fotografiado a calidad de catálogo.

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

    let nombres = photos::nuevos_nombres();
    photos::guardar(&nombres, &foto, &miniatura)?;

    // La primera foto de un producto queda como principal.
    let posicion = existentes as i32;

    let insertado = db.execute(
        "INSERT INTO product_images (product_id, file_name, thumb_name, position)
         VALUES (?1, ?2, ?3, ?4)",
        params![data.product_id, nombres.file_name, nombres.thumb_name, posicion],
    );

    if let Err(e) = insertado {
        // Sin fila que los referencie, los archivos serían basura invisible.
        photos::borrar(&nombres.file_name, &nombres.thumb_name);
        return Err(e.to_string());
    }

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

    let file_name: String = db
        .query_row(
            "SELECT file_name FROM product_images WHERE id = ?1",
            params![image_id],
            |r| r.get(0),
        )
        .map_err(|_| "La foto no existe".to_string())?;

    photos::leer_como_data_url(&file_name)
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

    let (product_id, file_name, thumb_name): (i64, String, String) = db
        .query_row(
            "SELECT product_id, file_name, thumb_name FROM product_images WHERE id = ?1",
            params![image_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| "La foto no existe".to_string())?;

    db.execute("DELETE FROM product_images WHERE id = ?1", params![image_id])
        .map_err(|e| e.to_string())?;
    photos::borrar(&file_name, &thumb_name);

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
            "SELECT product_id, thumb_name FROM product_images
             WHERE product_id IN ({}) AND position = 0",
            marcadores
        ))
        .map_err(|e| e.to_string())?;

    let filas: Vec<(i64, String)> = stmt
        .query_map(rusqlite::params_from_iter(product_ids.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    // Una miniatura ilegible no debe tumbar el catálogo entero.
    Ok(filas
        .into_iter()
        .filter_map(|(id, nombre)| photos::leer_como_data_url(&nombre).ok().map(|url| (id, url)))
        .collect())
}

/// Mueve a archivos las fotos que quedaron incrustadas en filas antiguas.
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
        log::info!("Fotos migradas de la base a archivos: {}", migradas);
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

    fn limpiar(db: &rusqlite::Connection) {
        let nombres: Vec<(String, String)> = db
            .prepare("SELECT file_name, thumb_name FROM product_images").unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap()
            .collect::<Result<Vec<_>, _>>().unwrap();
        for (f, t) in nombres {
            photos::borrar(&f, &t);
        }
    }

    #[test]
    fn la_primera_foto_queda_como_principal() {
        let db = tienda();
        let img = agregar_foto(&db, &foto(1)).unwrap();
        assert_eq!(img.position, 0);
        limpiar(&db);
    }

    #[test]
    fn las_siguientes_se_acomodan_detras() {
        let db = tienda();
        agregar_foto(&db, &foto(1)).unwrap();
        let segunda = agregar_foto(&db, &foto(1)).unwrap();
        assert_eq!(segunda.position, 1);
        limpiar(&db);
    }

    #[test]
    fn restaurar_un_respaldo_no_se_lleva_las_fotos_recientes() {
        // Al volver a un respaldo, lo fotografiado después se queda sin fila
        // que lo nombre. Borrarlo en ese momento sería irreversible.
        let db = tienda();
        let img = agregar_foto(&db, &foto(1)).unwrap();
        let (archivo, miniatura): (String, String) = db.query_row(
            "SELECT file_name, thumb_name FROM product_images WHERE id = ?1",
            params![img.id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();

        // La restauración deja la base como estaba: sin esa fila.
        db.execute("DELETE FROM product_images", []).unwrap();
        crate::photos::limpiar_huerfanas(&db);

        assert!(crate::photos::safe_path(&archivo).unwrap().exists(),
                "la foto reciente debe seguir ahí");
        assert!(crate::photos::safe_path(&miniatura).unwrap().exists());

        photos::borrar(&archivo, &miniatura);
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
        limpiar(&db);
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
        limpiar(&db);
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
        limpiar(&db);
    }

    #[test]
    fn borrar_una_foto_renumera_las_que_quedan() {
        let db = tienda();
        let a = agregar_foto(&db, &foto(1)).unwrap();
        agregar_foto(&db, &foto(1)).unwrap();

        let (f, t): (String, String) = db.query_row(
            "SELECT file_name, thumb_name FROM product_images WHERE id = ?1",
            params![a.id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        db.execute("DELETE FROM product_images WHERE id = ?1", params![a.id]).unwrap();
        photos::borrar(&f, &t);
        renumerar(&db, 1).unwrap();

        let posicion: i32 = db.query_row(
            "SELECT position FROM product_images WHERE product_id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(posicion, 0, "la que queda debe pasar a principal");
        limpiar(&db);
    }

    #[test]
    fn borrar_el_producto_se_lleva_sus_fotos() {
        let db = tienda();
        agregar_foto(&db, &foto(1)).unwrap();
        limpiar(&db);

        db.execute("DELETE FROM products WHERE id = 1", []).unwrap();

        let cuantas: i64 = db.query_row("SELECT COUNT(*) FROM product_images", [], |r| r.get(0)).unwrap();
        assert_eq!(cuantas, 0, "la cascada debe limpiar las filas");
    }
}
