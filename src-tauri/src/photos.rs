//! Almacenamiento de las fotos de producto como archivos.
//!
//! Las fotos no viven dentro de la base de datos: se guardan en `fotos/` dentro
//! del directorio de datos y la fila solo conserva el nombre del archivo.
//!
//! Tres razones. El respaldo copia la base entera cada vez, y con las fotos
//! adentro treinta respaldos de un catálogo fotografiado son gigabytes. Un
//! producto de ropa necesita más de una foto. Y lo más importante: reducirlas al
//! guardarlas destruye el original, así que el día que se quiera un catálogo
//! impreso o una tienda en línea habría que volver a fotografiar toda la
//! mercancía.
//!
//! El redimensionado lo hace quien envía la foto —la app de escritorio o el
//! celular, ambos con canvas— y aquí solo se escriben bytes. Así se evita
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

/// Par de nombres generado para una foto nueva: original y miniatura.
pub struct NombresFoto {
    pub file_name: String,
    pub thumb_name: String,
}

pub fn nuevos_nombres() -> NombresFoto {
    let id = uuid::Uuid::new_v4().to_string();
    NombresFoto {
        file_name: format!("{}.jpg", id),
        thumb_name: format!("{}_t.jpg", id),
    }
}

/// Escribe una foto y su miniatura, validando ambas.
pub fn guardar(nombres: &NombresFoto, foto: &[u8], miniatura: &[u8]) -> Result<(), String> {
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

    let ruta_foto = safe_path(&nombres.file_name)?;
    let ruta_thumb = safe_path(&nombres.thumb_name)?;

    fs::write(&ruta_foto, foto).map_err(|e| format!("No se pudo guardar la foto: {}", e))?;
    if let Err(e) = fs::write(&ruta_thumb, miniatura) {
        // Sin miniatura la foto quedaría a medias; mejor no dejar el archivo suelto.
        let _ = fs::remove_file(&ruta_foto);
        return Err(format!("No se pudo guardar la miniatura: {}", e));
    }
    Ok(())
}

/// Lee una foto como data URL para mostrarla en la interfaz.
pub fn leer_como_data_url(file_name: &str) -> Result<String, String> {
    let ruta = safe_path(file_name)?;
    let bytes = fs::read(&ruta).map_err(|e| format!("No se pudo leer la foto: {}", e))?;
    Ok(format!("data:image/jpeg;base64,{}", base64_encode(&bytes)))
}

/// Borra una foto y su miniatura. Que ya no existan no es un error.
pub fn borrar(file_name: &str, thumb_name: &str) {
    for nombre in [file_name, thumb_name] {
        if let Ok(ruta) = safe_path(nombre) {
            let _ = fs::remove_file(ruta);
        }
    }
}

/// Días que una foto sin dueño se conserva antes de borrarse.
///
/// Restaurar un respaldo devuelve la base a como estaba, y las fotos tomadas
/// después dejan de tener fila que las nombre. Borrarlas en ese momento es
/// irreversible: la restauración se llevaría por delante justo la mercancía más
/// reciente. Con la espera, quedan ahí el tiempo suficiente para volver a un
/// respaldo más nuevo o para recuperarlas a mano.
const DIAS_DE_GRACIA: u64 = 30;

/// Elimina de la carpeta las fotos que ya ninguna fila referencia y llevan
/// tiempo sin dueño.
///
/// Un producto borrado se lleva sus filas por cascada, pero los archivos se
/// quedarían ocupando disco para siempre. Las recién quedadas sin dueño se
/// respetan: ver `DIAS_DE_GRACIA`.
pub fn limpiar_huerfanas(db: &rusqlite::Connection) -> usize {
    let referenciadas: std::collections::HashSet<String> = match db
        .prepare("SELECT file_name FROM product_images UNION SELECT thumb_name FROM product_images")
    {
        Ok(mut stmt) => match stmt.query_map([], |r| r.get::<_, String>(0)) {
            Ok(rows) => rows.flatten().collect(),
            Err(_) => return 0,
        },
        Err(_) => return 0,
    };

    let gracia = std::time::Duration::from_secs(DIAS_DE_GRACIA * 24 * 60 * 60);
    let Ok(entradas) = fs::read_dir(photos_dir()) else { return 0 };
    let mut borradas = 0;
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name().to_string_lossy().to_string();
        if !nombre.ends_with(".jpg") || referenciadas.contains(&nombre) {
            continue;
        }
        // Ante la duda sobre la antigüedad, se conserva: perder una foto es
        // peor que dejar unos kilobytes ocupados.
        let vieja = entrada
            .metadata()
            .and_then(|m| m.modified())
            .map(|t| t.elapsed().map(|e| e > gracia).unwrap_or(false))
            .unwrap_or(false);
        if vieja && fs::remove_file(entrada.path()).is_ok() {
            borradas += 1;
        }
    }
    if borradas > 0 {
        log::info!("Fotos huérfanas eliminadas: {}", borradas);
    }
    borradas
}

/// Espacio que ocupan las fotos, para mostrarlo en el diagnóstico.
pub fn espacio_usado() -> u64 {
    fs::read_dir(photos_dir())
        .map(|d| {
            d.flatten()
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
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

    #[test]
    fn cada_foto_recibe_un_nombre_irrepetible() {
        let a = nuevos_nombres();
        let b = nuevos_nombres();
        assert_ne!(a.file_name, b.file_name);
        assert!(a.file_name.ends_with(".jpg"));
        assert!(a.thumb_name.ends_with("_t.jpg"));
        assert!(safe_path(&a.file_name).is_ok());
        assert!(safe_path(&a.thumb_name).is_ok());
    }
}
