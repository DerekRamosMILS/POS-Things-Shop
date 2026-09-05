//! Utilidades para las fotos de producto.
//!
//! Las fotos viven dentro de la base, no en archivos. La 018 las sacó a `fotos/`
//! para que la base quedara chica; el costo apareció en cuanto la tienda empezó
//! a usarla. Un archivo que ninguna fila nombra es basura que hay que barrer, y
//! barrer archivos no tiene vuelta atrás: restaurar un respaldo dejaba sin dueño
//! todo lo fotografiado después. El respaldo tampoco se las llevaba.
//!
//! Aquí solo queda lo que sigue haciendo falta: validar los bytes, convertirlos
//! a data URL y leer los archivos viejos mientras se terminan de incorporar.
//!
//! El redimensionado lo hace quien envía la foto —la app de escritorio o el
//! celular, ambos con canvas— y aquí solo se manejan bytes. Así se evita
//! arrastrar una biblioteca de imágenes al binario.

use std::fs;
use std::path::PathBuf;

/// Tope por archivo. Una foto de catálogo a 1600px ronda los 400 KB; el límite
/// deja margen de sobra y frena un envío absurdo.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;

/// Carpeta donde viven las fotos, dentro del directorio de datos.
pub fn photos_dir() -> PathBuf {
    let dir = crate::db::connection::get_db_dir().join("fotos");
    if let Err(e) = fs::create_dir_all(&dir) {
        log::error!("No se pudo crear la carpeta de fotos {:?}: {}", dir, e);
    }
    dir
}

/// Rechaza cualquier nombre que no sea un archivo simple dentro de `fotos/`.
///
/// El nombre viaja por IPC y podría venir manipulado; sin esta comprobación un
/// `../../` permitiría leer o borrar archivos fuera de la carpeta.
pub fn safe_path(file_name: &str) -> Result<PathBuf, String> {
    let name = file_name.trim();
    if name.is_empty() {
        return Err("Nombre de archivo vacío".to_string());
    }
    let solo_permitidos = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    if !solo_permitidos || name.contains("..") || name.starts_with('.') {
        return Err("Nombre de archivo inválido".to_string());
    }
    if !name.ends_with(".jpg") {
        return Err("Solo se aceptan archivos .jpg".to_string());
    }
    Ok(photos_dir().join(name))
}

/// Comprueba que los bytes sean realmente un JPEG.
///
/// El nombre del archivo no prueba nada: se mira la firma del contenido para no
/// terminar guardando cualquier cosa con extensión de imagen.
pub fn es_jpeg(bytes: &[u8]) -> bool {
    bytes.len() > 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
}

/// Comprueba que un par foto/miniatura sea guardable antes de tocar la base.
pub fn validar(foto: &[u8], miniatura: &[u8]) -> Result<(), String> {
    for (bytes, que) in [(foto, "La foto"), (miniatura, "La miniatura")] {
        if bytes.is_empty() {
            return Err(format!("{} llegó vacía", que));
        }
        if bytes.len() > MAX_BYTES {
            return Err(format!("{} pesa más de {} MB", que, MAX_BYTES / 1024 / 1024));
        }
        if !es_jpeg(bytes) {
            return Err(format!("{} no es una imagen JPEG válida", que));
        }
    }
    Ok(())
}

/// Envuelve bytes JPEG en un data URL para mostrarlos en la interfaz.
pub fn como_data_url(bytes: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", base64_encode(bytes))
}

/// Lee un archivo de la carpeta de fotos. Solo para las que aún no se han
/// incorporado a la base.
pub fn leer(file_name: &str) -> Result<Vec<u8>, String> {
    let ruta = safe_path(file_name)?;
    fs::read(&ruta).map_err(|e| format!("No se pudo leer la foto: {}", e))
}

/// Lee una foto de archivo como data URL. Ídem.
pub fn leer_como_data_url(file_name: &str) -> Result<String, String> {
    Ok(como_data_url(&leer(file_name)?))
}

/// Espacio que ocupan las fotos dentro de la base, para el diagnóstico.
pub fn espacio_usado(db: &rusqlite::Connection) -> u64 {
    db.query_row(
        "SELECT COALESCE(SUM(LENGTH(photo)) + SUM(LENGTH(thumbnail)), 0) FROM product_images",
        [],
        |r| r.get::<_, i64>(0),
    )
    .unwrap_or(0)
    .max(0) as u64
}

// ─── base64 ──────────────────────────────────────────────────────────────────
// Se implementa aquí en lugar de traer una dependencia: son unas líneas y se
// usa en un solo sentido.

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn base64_decode(texto: &str) -> Result<Vec<u8>, String> {
    let limpio: Vec<u8> = texto.bytes().filter(|b| !b.is_ascii_whitespace() && *b != b'=').collect();
    let valor = |b: u8| -> Result<u32, String> {
        match b {
            b'A'..=b'Z' => Ok((b - b'A') as u32),
            b'a'..=b'z' => Ok((b - b'a') as u32 + 26),
            b'0'..=b'9' => Ok((b - b'0') as u32 + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err("El contenido no es base64 válido".to_string()),
        }
    };

    let mut out = Vec::with_capacity(limpio.len() * 3 / 4);
    for chunk in limpio.chunks(4) {
        if chunk.len() < 2 {
            return Err("El contenido base64 está truncado".to_string());
        }
        let mut n = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            n |= valor(*b)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 { out.push((n >> 8) as u8); }
        if chunk.len() > 3 { out.push(n as u8); }
    }
    Ok(out)
}

/// Extrae los bytes de un data URL de imagen (`data:image/jpeg;base64,...`).
pub fn bytes_de_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let coma = data_url.find(',').ok_or("El data URL no tiene contenido")?;
    let cabecera = &data_url[..coma];
    if !cabecera.contains("base64") {
        return Err("Solo se aceptan data URL en base64".to_string());
    }
    base64_decode(&data_url[coma + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_ida_y_vuelta() {
        for caso in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0xFF, 0xD8, 0xFF, 0x00, 0x01][..]] {
            let texto = base64_encode(caso);
            assert_eq!(base64_decode(&texto).unwrap(), caso, "falló con {:?}", caso);
        }
    }

    #[test]
    fn base64_coincide_con_la_codificacion_estandar() {
        assert_eq!(base64_encode(b"hola"), "aG9sYQ==");
        assert_eq!(base64_encode(b"Things Shop"), "VGhpbmdzIFNob3A=");
        assert_eq!(base64_decode("aG9sYQ==").unwrap(), b"hola");
    }

    #[test]
    fn base64_invalido_se_rechaza() {
        assert!(base64_decode("!!!!").is_err());
        assert!(base64_decode("a").is_err());
    }

    #[test]
    fn un_data_url_entrega_sus_bytes() {
        let url = format!("data:image/jpeg;base64,{}", base64_encode(&[0xFF, 0xD8, 0xFF, 0x42]));
        assert_eq!(bytes_de_data_url(&url).unwrap(), vec![0xFF, 0xD8, 0xFF, 0x42]);
    }

    #[test]
    fn un_data_url_sin_base64_se_rechaza() {
        assert!(bytes_de_data_url("data:image/jpeg,%FF%D8").is_err());
        assert!(bytes_de_data_url("no soy un data url").is_err());
    }

    #[test]
    fn solo_se_acepta_contenido_jpeg_real() {
        assert!(es_jpeg(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(!es_jpeg(b"<html>"));
        assert!(!es_jpeg(&[0x89, 0x50, 0x4E, 0x47]), "un PNG no debe pasar por JPEG");
        assert!(!es_jpeg(&[0xFF, 0xD8]));
    }

    #[test]
    fn un_nombre_no_puede_salirse_de_la_carpeta() {
        for malicioso in ["../secreto.jpg", "..\\secreto.jpg", "sub/dir.jpg", ".oculto.jpg", ""] {
            assert!(safe_path(malicioso).is_err(), "debió rechazar {:?}", malicioso);
        }
    }

    #[test]
    fn solo_se_aceptan_nombres_jpg() {
        assert!(safe_path("foto.png").is_err());
        assert!(safe_path("script.js").is_err());
        assert!(safe_path("abc123-def_t.jpg").is_ok());
    }

}
