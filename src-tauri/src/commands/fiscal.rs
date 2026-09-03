//! Datos fiscales para facturación electrónica (CFDI 4.0, México).
//!
//! Este módulo **no timbra**: emitir un CFDI exige un contrato con un PAC y los
//! certificados de sello digital del negocio, que no viven aquí. Lo que sí
//! resuelve es el problema práctico del mostrador: capturar los datos correctos
//! en el momento de la venta —cuando el cliente todavía está enfrente— y poder
//! entregárselos después al PAC o al contador en un archivo limpio.
//!
//! Cuando se contrate un PAC, la ruta de integración es corta: los datos ya
//! están validados y guardados por venta; solo falta el paso de sellado.

use rusqlite::params;
use tauri::State;

use crate::db::connection::DbState;
use crate::session::{require_admin, SessionState};

/// Régimen fiscal del receptor (catálogo `c_RegimenFiscal` del SAT).
/// Solo los aplicables a un cliente de mostrador.
pub const REGIMENES: &[(&str, &str)] = &[
    ("601", "General de Ley Personas Morales"),
    ("603", "Personas Morales con Fines no Lucrativos"),
    ("605", "Sueldos y Salarios e Ingresos Asimilados a Salarios"),
    ("606", "Arrendamiento"),
    ("608", "Demás ingresos"),
    ("612", "Personas Físicas con Actividades Empresariales y Profesionales"),
    ("614", "Ingresos por intereses"),
    ("616", "Sin obligaciones fiscales"),
    ("621", "Incorporación Fiscal"),
    ("626", "Régimen Simplificado de Confianza"),
];

/// Uso del CFDI (catálogo `c_UsoCFDI` del SAT).
pub const USOS_CFDI: &[(&str, &str)] = &[
    ("G01", "Adquisición de mercancías"),
    ("G02", "Devoluciones, descuentos o bonificaciones"),
    ("G03", "Gastos en general"),
    ("D01", "Honorarios médicos y gastos hospitalarios"),
    ("D10", "Pagos por servicios educativos"),
    ("S01", "Sin efectos fiscales"),
    ("CP01", "Pagos"),
];

/// Valida un RFC mexicano.
///
/// Persona moral: 3 letras + 6 dígitos de fecha + 3 de homoclave (12).
/// Persona física: 4 letras + 6 dígitos de fecha + 3 de homoclave (13).
/// Se valida forma y fecha; la existencia real solo la confirma el SAT.
pub fn validar_rfc(rfc: &str) -> Result<String, String> {
    let rfc = rfc.trim().to_uppercase().replace(['-', ' '], "");

    if rfc.is_empty() {
        return Err("El RFC está vacío".to_string());
    }
    // RFC genérico para el público en general; es válido y de uso corriente.
    if rfc == "XAXX010101000" || rfc == "XEXX010101000" {
        return Ok(rfc);
    }
    if rfc.len() != 12 && rfc.len() != 13 {
        return Err("El RFC debe tener 12 dígitos (empresa) o 13 (persona física)".to_string());
    }

    let letras = if rfc.len() == 12 { 3 } else { 4 };
    let chars: Vec<char> = rfc.chars().collect();

    if !chars[..letras].iter().all(|c| c.is_ascii_alphabetic() || *c == '&') {
        return Err("El RFC debe empezar con las letras del nombre".to_string());
    }
    let fecha: String = chars[letras..letras + 6].iter().collect();
    if !fecha.chars().all(|c| c.is_ascii_digit()) {
        return Err("El RFC debe llevar la fecha en formato AAMMDD".to_string());
    }
    let mes: u32 = fecha[2..4].parse().unwrap_or(0);
    let dia: u32 = fecha[4..6].parse().unwrap_or(0);
    if !(1..=12).contains(&mes) || !(1..=31).contains(&dia) {
        return Err("La fecha dentro del RFC no es válida".to_string());
    }
    if !chars[letras + 6..].iter().all(|c| c.is_ascii_alphanumeric()) {
        return Err("La homoclave del RFC no es válida".to_string());
    }

    Ok(rfc)
}

/// Valida un código postal mexicano de 5 dígitos.
pub fn validar_cp(cp: &str) -> Result<String, String> {
    let cp = cp.trim();
    if cp.len() != 5 || !cp.chars().all(|c| c.is_ascii_digit()) {
        return Err("El código postal debe tener 5 dígitos".to_string());
    }
    Ok(cp.to_string())
}

pub fn regimen_valido(clave: &str) -> bool {
    REGIMENES.iter().any(|(k, _)| *k == clave)
}

pub fn uso_valido(clave: &str) -> bool {
    USOS_CFDI.iter().any(|(k, _)| *k == clave)
}

/// Revisa los datos fiscales de un cliente antes de guardarlos.
/// Los campos vacíos se aceptan: no todo cliente pide factura.
pub fn validar_datos_fiscales(
    rfc: &Option<String>,
    regimen: &Option<String>,
    cp: &Option<String>,
    uso: &Option<String>,
) -> Result<(), String> {
    if let Some(v) = rfc.as_deref().filter(|v| !v.trim().is_empty()) {
        validar_rfc(v)?;
    }
    if let Some(v) = cp.as_deref().filter(|v| !v.trim().is_empty()) {
        validar_cp(v)?;
    }
    if let Some(v) = regimen.as_deref().filter(|v| !v.trim().is_empty()) {
        if !regimen_valido(v) {
            return Err(format!("El régimen fiscal '{}' no está en el catálogo del SAT", v));
        }
    }
    if let Some(v) = uso.as_deref().filter(|v| !v.trim().is_empty()) {
        if !uso_valido(v) {
            return Err(format!("El uso de CFDI '{}' no está en el catálogo del SAT", v));
        }
    }
    Ok(())
}

/// Catálogos para que la interfaz ofrezca las claves válidas.
#[tauri::command]
pub fn get_catalogos_fiscales() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "regimenes": REGIMENES.iter().map(|(k, v)| serde_json::json!({ "clave": k, "nombre": v })).collect::<Vec<_>>(),
        "usos_cfdi": USOS_CFDI.iter().map(|(k, v)| serde_json::json!({ "clave": k, "nombre": v })).collect::<Vec<_>>(),
    }))
}

fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Genera el CSV de ventas pendientes de facturar.
///
/// Una fila por venta con todo lo que un PAC o un contador necesita. Se entrega
/// como archivo en lugar de enviarse a ningún lado: a quién se le manda la
/// información fiscal de la tienda lo decide su dueño.
#[tauri::command]
pub fn exportar_pendientes_factura(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    path: String,
    desde: Option<String>,
    hasta: Option<String>,
) -> Result<usize, String> {
    require_admin(&sessions, &token)?;
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut out = String::from(
        "folio,fecha,rfc,razon_social,regimen_fiscal,cp_fiscal,uso_cfdi,\
         subtotal,descuento,impuesto,total,forma_pago\n",
    );

    let mut stmt = db
        .prepare(
            "SELECT folio, created_at, fiscal_rfc, fiscal_razon_social, fiscal_regimen,
                    fiscal_cp, fiscal_uso_cfdi, subtotal, discount_total, tax, total, payment_method
             FROM sales
             WHERE requiere_factura = 1
               AND uuid_fiscal IS NULL
               AND status = 'completed'
               AND (?1 IS NULL OR date(created_at) >= date(?1))
               AND (?2 IS NULL OR date(created_at) <= date(?2))
             ORDER BY created_at",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![desde, hasta], |r| {
            Ok(vec![
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                r.get::<_, Option<String>>(5)?.unwrap_or_default(),
                r.get::<_, Option<String>>(6)?.unwrap_or_default(),
                format!("{:.2}", r.get::<_, f64>(7)?),
                format!("{:.2}", r.get::<_, f64>(8)?),
                format!("{:.2}", r.get::<_, f64>(9)?),
                format!("{:.2}", r.get::<_, f64>(10)?),
                r.get::<_, String>(11)?,
            ])
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);

    let count = rows.len();
    for row in rows {
        out.push_str(&row.iter().map(|f| csv_field(f)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }

    std::fs::write(&path, out).map_err(|e| format!("No se pudo guardar el archivo: {}", e))?;
    log::info!("Exportadas {} ventas pendientes de facturar a {}", count, path);

    Ok(count)
}

/// Registra el folio fiscal que devolvió el PAC, para no volver a exportarla.
#[tauri::command]
pub fn marcar_facturada(
    state: State<DbState>,
    sessions: State<SessionState>,
    token: String,
    sale_id: i64,
    uuid: String,
) -> Result<(), String> {
    require_admin(&sessions, &token)?;

    let uuid = uuid.trim().to_uppercase();
    // El folio fiscal es un UUID: 8-4-4-4-12 caracteres hexadecimales.
    let partes: Vec<&str> = uuid.split('-').collect();
    let formato_ok = partes.len() == 5
        && [8, 4, 4, 4, 12] == [partes[0].len(), partes[1].len(), partes[2].len(), partes[3].len(), partes[4].len()]
        && partes.iter().all(|p| p.chars().all(|c| c.is_ascii_hexdigit()));
    if !formato_ok {
        return Err("El folio fiscal (UUID) no tiene el formato correcto".to_string());
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;
    let changed = db
        .execute(
            "UPDATE sales SET uuid_fiscal = ?1, facturada_at = datetime('now','localtime')
             WHERE id = ?2 AND uuid_fiscal IS NULL",
            params![uuid, sale_id],
        )
        .map_err(|e| e.to_string())?;

    if changed == 0 {
        return Err("La venta no existe o ya estaba facturada".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_well_formed_company_rfc() {
        assert_eq!(validar_rfc("ABC010101AB1").unwrap(), "ABC010101AB1");
    }

    #[test]
    fn accepts_a_well_formed_personal_rfc() {
        assert_eq!(validar_rfc("RAMD900215H24").unwrap(), "RAMD900215H24");
    }

    #[test]
    fn normalises_lowercase_and_separators() {
        assert_eq!(validar_rfc(" ramd900215h24 ").unwrap(), "RAMD900215H24");
        assert_eq!(validar_rfc("RAMD-900215-H24").unwrap(), "RAMD900215H24");
    }

    #[test]
    fn accepts_the_generic_public_rfc() {
        // El que se usa cuando la venta es a público en general.
        assert!(validar_rfc("XAXX010101000").is_ok());
        assert!(validar_rfc("XEXX010101000").is_ok());
    }

    #[test]
    fn accepts_the_ampersand_some_company_names_carry() {
        assert!(validar_rfc("A&B010101AB1").is_ok());
    }

    #[test]
    fn rejects_rfcs_of_the_wrong_length() {
        assert!(validar_rfc("ABC").is_err());
        assert!(validar_rfc("ABC010101AB12").is_err());
        assert!(validar_rfc("").is_err());
    }

    #[test]
    fn rejects_an_impossible_date_inside_the_rfc() {
        assert!(validar_rfc("ABC019901AB1").is_err(), "mes 99");
        assert!(validar_rfc("ABC010199AB1").is_err(), "día 99");
        assert!(validar_rfc("ABC0101XXAB1").is_err(), "fecha no numérica");
    }

    #[test]
    fn rejects_a_homoclave_with_symbols() {
        assert!(validar_rfc("ABC010101A#1").is_err());
    }

    #[test]
    fn postal_codes_must_be_five_digits() {
        assert_eq!(validar_cp("64000").unwrap(), "64000");
        assert!(validar_cp("6400").is_err());
        assert!(validar_cp("ABCDE").is_err());
        assert!(validar_cp("").is_err());
    }

    #[test]
    fn catalog_keys_are_checked_against_the_sat_list() {
        assert!(regimen_valido("626"));
        assert!(!regimen_valido("999"));
        assert!(uso_valido("G03"));
        assert!(!uso_valido("ZZ9"));
    }

    #[test]
    fn empty_fiscal_data_is_allowed_because_not_every_sale_is_invoiced() {
        assert!(validar_datos_fiscales(&None, &None, &None, &None).is_ok());
        assert!(validar_datos_fiscales(
            &Some("  ".into()), &Some("".into()), &Some("".into()), &None
        ).is_ok());
    }

    #[test]
    fn partial_fiscal_data_is_still_validated() {
        assert!(validar_datos_fiscales(&Some("NOSOYRFC".into()), &None, &None, &None).is_err());
        assert!(validar_datos_fiscales(&None, &Some("999".into()), &None, &None).is_err());
        assert!(validar_datos_fiscales(&None, &None, &Some("123".into()), &None).is_err());
        assert!(validar_datos_fiscales(&None, &None, &None, &Some("ZZ9".into())).is_err());
    }

    #[test]
    fn csv_fields_with_commas_are_quoted() {
        assert_eq!(csv_field("Ropa y Mas, S.A."), "\"Ropa y Mas, S.A.\"");
        assert_eq!(csv_field("Simple"), "Simple");
        assert_eq!(csv_field("Dijo \"hola\""), "\"Dijo \"\"hola\"\"\"");
    }
}
