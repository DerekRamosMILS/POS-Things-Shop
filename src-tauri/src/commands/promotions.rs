use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::models::promotion::{CreatePromotionDto, Promotion};
use crate::session::{require_admin, require_auth, SessionState};

/// Comprobaciones comunes al crear y editar.
///
/// La pantalla del punto de venta calcula el descuento con su propia fórmula y
/// el cobro lo recalcula en el servidor recortando lo que no tiene sentido. Con
/// una promoción imposible —un cien por ciento y medio, o un rango de fechas al
/// revés— las dos cuentas dejan de coincidir y la cajera ve un total y cobra
/// otro. Se rechaza al guardarla, que es donde se puede explicar.
fn validar(
    nombre: &str,
    tipo: &str,
    valor: f64,
    inicio: &str,
    fin: &str,
    alcance: &str,
    destino: Option<i64>,
) -> Result<(), String> {
    match alcance {
        "all" => {}
        "category" | "product" if destino.is_none() => {
            return Err(if alcance == "category" {
                "Elige a qué categoría aplica la promoción".to_string()
            } else {
                "Elige a qué producto aplica la promoción".to_string()
            });
        }
        "category" | "product" => {}
        otro => return Err(format!("Alcance de promoción desconocido: {}", otro)),
    }
    // El único tipo que se reconoce por nombre es "percentage"; al cobrar,
    // cualquier otro se cobra como monto fijo. Un tipo escrito de otra forma no
    // falla: descuenta la cantidad equivocada. La tabla trae un CHECK que lo
    // impide, pero ya se perdió un CHECK así antes (migración 017), y aquí el
    // mensaje además es legible.
    if tipo != "percentage" && tipo != "fixed" {
        return Err(format!("Tipo de descuento desconocido: {}", tipo));
    }
    if nombre.trim().is_empty() {
        return Err("Ponle nombre a la promoción".to_string());
    }
    if !valor.is_finite() || valor <= 0.0 {
        return Err("El descuento tiene que ser mayor a cero".to_string());
    }
    if tipo == "percentage" && valor > 100.0 {
        return Err("Un descuento por porcentaje no puede pasar de 100%".to_string());
    }
    if fin < inicio {
        return Err("La fecha de fin no puede ser anterior a la de inicio".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn get_promotions(state: State<DbState>, sessions: State<SessionState>, token: String) -> Result<Vec<Promotion>, String> {
    require_auth(&sessions, &token)?;
    let db = state.conn();

    let mut stmt = db.prepare(
        "SELECT id, name, description, discount_type, discount_value, start_date, end_date, is_active, applies_to, target_id, created_at
         FROM promotions ORDER BY created_at DESC"
    ).map_err(|e| e.to_string())?;

    let promotions = stmt.query_map([], |row| {
        Ok(Promotion {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            discount_type: row.get(3)?,
            discount_value: row.get(4)?,
            start_date: row.get(5)?,
            end_date: row.get(6)?,
            is_active: row.get(7)?,
            applies_to: row.get(8)?,
            target_id: row.get(9)?,
            created_at: row.get(10)?,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok(promotions)
}

#[tauri::command]
pub fn create_promotion(state: State<DbState>, sessions: State<SessionState>, token: String, data: CreatePromotionDto) -> Result<Promotion, String> {
    require_admin(&sessions, &token)?;
    validar(&data.name, &data.discount_type, data.discount_value, &data.start_date, &data.end_date, &data.applies_to, data.target_id)?;
    let db = state.conn();

    db.execute(
        "INSERT INTO promotions (name, description, discount_type, discount_value, start_date, end_date, is_active, applies_to, target_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)",
        params![data.name.trim(), data.description, data.discount_type, data.discount_value, data.start_date, data.end_date, data.applies_to, data.target_id],
    ).map_err(|e| e.to_string())?;

    let id = db.last_insert_rowid();
    db.query_row(
        "SELECT id, name, description, discount_type, discount_value, start_date, end_date, is_active, applies_to, target_id, created_at FROM promotions WHERE id = ?1",
        params![id],
        |row| Ok(Promotion {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            discount_type: row.get(3)?,
            discount_value: row.get(4)?,
            start_date: row.get(5)?,
            end_date: row.get(6)?,
            is_active: row.get(7)?,
            applies_to: row.get(8)?,
            target_id: row.get(9)?,
            created_at: row.get(10)?,
        })
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_promotion(state: State<DbState>, sessions: State<SessionState>, token: String, data: Promotion) -> Result<(), String> {
    require_admin(&sessions, &token)?;
    validar(&data.name, &data.discount_type, data.discount_value, &data.start_date, &data.end_date, &data.applies_to, data.target_id)?;
    let db = state.conn();

    db.execute(
        "UPDATE promotions SET name = ?1, description = ?2, discount_type = ?3, discount_value = ?4,
         start_date = ?5, end_date = ?6, is_active = ?7, applies_to = ?8, target_id = ?9
         WHERE id = ?10",
        params![data.name.trim(), data.description, data.discount_type, data.discount_value,
                data.start_date, data.end_date, data.is_active, data.applies_to, data.target_id, data.id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn delete_promotion(state: State<DbState>, sessions: State<SessionState>, token: String, id: i64) -> Result<String, String> {
    require_admin(&sessions, &token)?;
    let db = state.conn();

    // Las ventas guardan con qué promoción se cobraron. Borrarla rompía la
    // llave foránea y salía un error de base de datos que nadie entiende; y si
    // pasara, los tickets de esos días perderían el porqué de su descuento.
    let usada: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sales WHERE promotion_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;

    if usada > 0 {
        db.execute(
            "UPDATE promotions SET is_active = 0 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
        return Ok(format!(
            "Se usó en {} venta(s), así que se desactivó en vez de borrarse: deja de aplicarse pero los tickets siguen cuadrando.",
            usada
        ));
    }

    db.execute("DELETE FROM promotions WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok("Promoción eliminada".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_promocion_sin_destino_se_rechaza_al_guardarla() {
        let ok = |alcance, destino| validar("Enero", "percentage", 10.0, "2026-01-01", "2026-12-31", alcance, destino);
        assert!(ok("category", None).is_err());
        assert!(ok("product", None).is_err());
        assert!(ok("marca", Some(1)).is_err());
        assert!(ok("category", Some(3)).is_ok());
        assert!(ok("product", Some(3)).is_ok());
        assert!(ok("all", None).is_ok());
    }

    #[test]
    fn una_promocion_imposible_se_rechaza_al_guardarla() {
        assert!(validar("", "percentage", 10.0, "2026-01-01", "2026-12-31", "all", None).is_err());
        assert!(validar("Enero", "percentage", 0.0, "2026-01-01", "2026-12-31", "all", None).is_err());
        assert!(validar("Enero", "percentage", -5.0, "2026-01-01", "2026-12-31", "all", None).is_err());
        assert!(validar("Enero", "percentage", 150.0, "2026-01-01", "2026-12-31", "all", None).is_err());
        assert!(validar("Enero", "fixed", 150.0, "2026-01-01", "2026-12-31", "all", None).is_ok(),
                "un descuento fijo de 150 pesos sí existe");
        assert!(validar("Enero", "percentage", 10.0, "2026-12-31", "2026-01-01", "all", None).is_err(),
                "un rango al revés nunca se aplicaría");
        assert!(validar("Enero", "percentage", 100.0, "2026-01-01", "2026-01-01", "all", None).is_ok());
    }

    #[test]
    fn un_tipo_de_descuento_desconocido_se_rechaza_al_guardarlo() {
        // Al cobrar, cualquier tipo que no sea "percentage" se trata como monto
        // fijo. Una promo guardada como "porcentaje" descontaría diez pesos
        // donde debía descontar el diez por ciento: en un ticket de dos mil, son
        // mil novecientos noventa de diferencia, sin una sola señal en pantalla.
        for tipo in ["porcentaje", "PERCENTAGE", "", "monto_fijo"] {
            assert!(
                validar("Enero", tipo, 10.0, "2026-01-01", "2026-12-31", "all", None).is_err(),
                "se aceptó el tipo {:?}",
                tipo
            );
        }
        assert!(validar("Enero", "percentage", 10.0, "2026-01-01", "2026-12-31", "all", None).is_ok());
        assert!(validar("Enero", "fixed", 10.0, "2026-01-01", "2026-12-31", "all", None).is_ok());
    }

    #[test]
    fn la_base_tambien_niega_un_tipo_de_descuento_inventado() {
        // `promotion_discount` ya no reinterpreta un tipo raro, pero además no
        // debería poder existir uno. La migración 001 lo restringe; la 017 está
        // ahí porque una restricción así se perdió al reconstruir una tabla. Si
        // alguien reconstruye `promotions` y se le olvida el CHECK, esto lo dice.
        let db = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::run_migrations(&db).unwrap();
        let insertar = |tipo: &str| {
            db.execute(
                "INSERT INTO promotions (name, discount_type, discount_value, start_date, end_date, applies_to)
                 VALUES ('Enero', ?1, 10.0, '2026-01-01', '2026-12-31', 'all')",
                rusqlite::params![tipo],
            )
        };
        assert!(insertar("porcentaje").is_err(), "la base aceptó un tipo inventado");
        assert!(insertar("percentage").is_ok());
        assert!(insertar("fixed").is_ok());
    }
}
