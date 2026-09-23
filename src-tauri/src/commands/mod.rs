pub mod backup;
pub mod cash_register;
pub mod categories;
pub mod config;
pub mod customers;
pub mod diagnostics;
pub mod expenses;
pub mod fiscal;
pub mod inventory;
pub mod layaways;
pub mod notifications;
pub mod product_photos;
pub mod products;
pub mod promotions;
pub mod reports;
pub mod returns;
pub mod sales;
pub mod seed;
pub mod suppliers;
pub mod users;
pub mod variants;

/// Nombre obligatorio, ya recortado.
///
/// Un nombre en blanco se cuela hasta el catálogo y desde la migración
/// `026_nada_se_borra` **no se puede borrar**: queda un renglón vacío en los
/// desplegables para siempre, y solo se le puede dar de baja. Los productos y los
/// clientes ya lo comprobaban; las categorías y los proveedores no.
pub(crate) fn nombre_requerido(valor: &str, que: &str) -> Result<String, String> {
    let limpio = valor.trim();
    if limpio.is_empty() {
        return Err(format!("Ponle nombre a {}", que));
    }
    Ok(limpio.to_string())
}

#[cfg(test)]
mod tests {
    use super::nombre_requerido;

    #[test]
    fn un_nombre_en_blanco_se_rechaza() {
        for vacio in ["", "   ", "\t", "\n  "] {
            assert!(nombre_requerido(vacio, "la categoría").is_err(), "pasó {:?}", vacio);
        }
    }

    #[test]
    fn un_nombre_con_espacios_de_sobra_se_recorta() {
        assert_eq!(nombre_requerido("  Ropa de dama  ", "la categoría").unwrap(), "Ropa de dama");
    }

    #[test]
    fn el_mensaje_dice_de_qué_se_habla() {
        let e = nombre_requerido("", "el proveedor").unwrap_err();
        assert!(e.contains("el proveedor"), "{}", e);
    }

    /// Todo comando que da de alta o renombra algo del catálogo tiene que pasar
    /// por la comprobación.
    ///
    /// Se revisa leyendo el código porque los comandos reciben `State` de Tauri y
    /// no se pueden llamar desde una prueba. Lo que importa no es el ayudante
    /// —ese ya tiene sus casos— sino que nadie agregue un `create_` nuevo que
    /// escriba un nombre sin comprobarlo: en blanco se cuela al catálogo y desde
    /// la 026 ya no se puede borrar.
    #[test]
    fn todo_comando_que_escribe_un_nombre_lo_comprueba() {
        let archivos = [
            ("categories.rs", include_str!("categories.rs")),
            ("suppliers.rs", include_str!("suppliers.rs")),
            ("customers.rs", include_str!("customers.rs")),
        ];

        let mut sin_comprobar = Vec::new();
        for (nombre, codigo) in archivos {
            for bloque in codigo.split("#[tauri::command]").skip(1) {
                let firma = bloque.lines().find(|l| l.contains("pub fn")).unwrap_or("");
                let es_alta_o_edicion = ["create_", "update_"].iter().any(|p| firma.contains(p));
                if !es_alta_o_edicion {
                    continue;
                }
                // Solo los que de verdad escriben la columna `name`.
                let cuerpo = bloque.split("#[tauri::command]").next().unwrap_or("");
                let escribe_nombre = cuerpo.contains("(name,") || cuerpo.contains("SET name=");
                if !escribe_nombre {
                    continue;
                }
                let comprueba = cuerpo.contains("nombre_requerido")
                    || cuerpo.contains("name.trim().is_empty()");
                if !comprueba {
                    let cual = firma.trim().trim_start_matches("pub fn ");
                    sin_comprobar.push(format!("{}: {}", nombre, cual.split('(').next().unwrap_or(cual)));
                }
            }
        }

        assert!(
            sin_comprobar.is_empty(),
            "estos comandos escriben un nombre sin comprobar que no venga en blanco: {:?}",
            sin_comprobar
        );
    }

    #[test]
    fn la_guarda_de_arriba_de_verdad_encuentra_los_comandos() {
        // Sin esto, un cambio de formato dejaría la lista vacía y la prueba
        // pasaría sin revisar nada.
        let codigo = include_str!("suppliers.rs");
        let altas = codigo
            .split("#[tauri::command]")
            .skip(1)
            .filter(|b| {
                let firma = b.lines().find(|l| l.contains("pub fn")).unwrap_or("");
                firma.contains("create_") || firma.contains("update_")
            })
            .count();
        assert!(altas >= 2, "se esperaban al menos create y update, se vieron {}", altas);
    }
}
