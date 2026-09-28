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
        // "a el proveedor" se contrae en "al". Lo leía así el usuario cada vez
        // que dejaba el nombre en blanco.
        let sujeto = match que.strip_prefix("el ") {
            Some(resto) => format!("al {}", resto),
            None => format!("a {}", que),
        };
        return Err(format!("Ponle nombre {}", sujeto));
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
        assert!(e.contains("proveedor"), "{}", e);
    }

    #[test]
    fn el_mensaje_contrae_la_preposición() {
        // "Ponle nombre a el proveedor" es lo que leía el usuario. La prueba de
        // arriba pasaba igual porque sólo buscaba el sustantivo: comprobaba el
        // alrededor del error, no el error.
        assert_eq!(nombre_requerido("", "el proveedor").unwrap_err(), "Ponle nombre al proveedor");
        assert_eq!(nombre_requerido("", "la categoría").unwrap_err(), "Ponle nombre a la categoría");
        assert_eq!(nombre_requerido("", "el cliente").unwrap_err(), "Ponle nombre al cliente");
    }

    /// Toda función que escribe una columna `name` la comprueba **y** la guarda
    /// recortada.
    ///
    /// Se revisa leyendo el código porque los comandos reciben `State` de Tauri y
    /// no se pueden llamar desde una prueba. La versión anterior partía por
    /// `#[tauri::command]` y aceptaba `name.trim().is_empty()` como suficiente:
    /// así pasaron clientes y productos, que comprobaban recortado y guardaban
    /// crudo. Y en cuanto la escritura se movió a una función aparte, la guarda
    /// dejó de ver clientes y siguió en verde sin revisar nada.
    const CON_NOMBRE: [(&str, &str); 6] = [
        ("categories.rs", include_str!("categories.rs")),
        ("suppliers.rs", include_str!("suppliers.rs")),
        ("customers.rs", include_str!("customers.rs")),
        ("products.rs", include_str!("products.rs")),
        ("promotions.rs", include_str!("promotions.rs")),
        ("capture/producto.rs", include_str!("../capture/producto.rs")),
    ];

    /// El código de producción partido por función: (nombre, cuerpo).
    fn funciones(codigo: &str) -> Vec<(String, String)> {
        let produccion = codigo.split("#[cfg(test)]").next().unwrap_or("");
        let mut out: Vec<(String, String)> = Vec::new();
        for linea in produccion.lines() {
            let t = linea.trim_start();
            let firma = ["pub fn ", "pub(crate) fn ", "fn "].iter().find_map(|p| t.strip_prefix(p));
            if let Some(resto) = firma {
                out.push((resto.split('(').next().unwrap_or(resto).to_string(), String::new()));
            }
            if let Some((_, cuerpo)) = out.last_mut() {
                cuerpo.push_str(linea);
                cuerpo.push('\n');
            }
        }
        out
    }

    /// ¿Escribe la columna `name` en un INSERT o un UPDATE?
    fn escribe_nombre(cuerpo: &str) -> bool {
        let columnas = |lista: &str| lista.split(',').any(|c| c.split('=').next().unwrap_or("").trim() == "name");
        let inserta = cuerpo.match_indices("INSERT INTO").any(|(i, _)| {
            let resto = &cuerpo[i..];
            match (resto.find('('), resto.find(')')) {
                (Some(a), Some(b)) if a < b => columnas(&resto[a + 1..b]),
                _ => false,
            }
        });
        let actualiza = cuerpo.match_indices(" SET ").any(|(i, _)| {
            let resto = &cuerpo[i + 5..];
            columnas(&resto[..resto.find("WHERE").unwrap_or(resto.len())])
        });
        inserta || actualiza
    }

    fn escritores() -> Vec<(String, String)> {
        CON_NOMBRE
            .iter()
            .flat_map(|(archivo, codigo)| {
                funciones(codigo)
                    .into_iter()
                    .filter(|(_, cuerpo)| escribe_nombre(cuerpo))
                    .map(move |(f, cuerpo)| (format!("{}: {}", archivo, f), cuerpo))
            })
            .collect()
    }

    /// Lo que va dentro de cada `params![...]` de la función.
    fn parametros(cuerpo: &str) -> Vec<&str> {
        cuerpo
            .match_indices("params![")
            .map(|(i, _)| {
                let resto = &cuerpo[i + 8..];
                &resto[..resto.find(']').unwrap_or(resto.len())]
            })
            .collect()
    }

    #[test]
    fn todo_nombre_se_comprueba_y_se_guarda_recortado() {
        let mut mal = Vec::new();
        for (cual, cuerpo) in escritores() {
            let crudo = parametros(&cuerpo).iter().any(|p| {
                p.split(',').any(|v| matches!(v.trim(), "data.name" | "entrada.nombre" | "name"))
            });
            if crudo {
                mal.push(format!("{} guarda el nombre sin recortar", cual));
            }
        }
        // La comprobación de "no en blanco" puede vivir en quien llama (la
        // captura valida en `recibir_producto` y guarda en `guardar_producto`),
        // así que se exige por archivo; cada módulo prueba la suya.
        for (archivo, codigo) in CON_NOMBRE {
            let produccion = codigo.split("#[cfg(test)]").next().unwrap_or("");
            let comprueba = ["nombre_requerido(", "validar_producto(", "validar(", ".trim().is_empty()"]
                .iter()
                .any(|p| produccion.contains(p));
            if !comprueba {
                mal.push(format!("{} no comprueba que el nombre no venga en blanco", archivo));
            }
        }
        assert!(mal.is_empty(), "{:#?}", mal);
    }

    #[test]
    fn la_guarda_de_arriba_de_verdad_encuentra_quien_escribe() {
        // Sin esto, un cambio de formato dejaría la lista vacía y la prueba
        // pasaría sin revisar nada, que es justo lo que ya pasó una vez.
        let vistos: Vec<String> = escritores().into_iter().map(|(c, _)| c).collect();
        for esperado in [
            "categories.rs: create_category",
            "suppliers.rs: update_supplier",
            "customers.rs: crear_cliente",
            "customers.rs: actualizar_cliente",
            "products.rs: crear_producto",
            "products.rs: update_product",
            "promotions.rs: create_promotion",
            "capture/producto.rs: guardar_producto",
        ] {
            assert!(vistos.iter().any(|v| v == esperado), "no se vio {}; se vieron {:?}", esperado, vistos);
        }
    }
}
