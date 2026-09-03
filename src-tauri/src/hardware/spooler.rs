//! Envío de bytes crudos a una impresora instalada en el sistema.
//!
//! En Windows se usa la API de spooler con datatype "RAW", que entrega los bytes
//! tal cual al puerto sin que el driver los reinterprete. Eso funciona con
//! cualquier impresora que tenga driver instalado —USB, serie o de red— sin
//! depender del modelo, y sin abrir el diálogo de impresión.

/// Lista las impresoras instaladas para que el usuario elija una en Ajustes.
pub fn list_printers() -> Result<Vec<String>, String> {
    platform::list_printers()
}

/// Manda `data` a `printer` sin pasar por el driver gráfico.
pub fn print_raw(printer: &str, job_name: &str, data: &[u8]) -> Result<(), String> {
    if printer.trim().is_empty() {
        return Err("No hay impresora configurada. Elígela en Ajustes.".to_string());
    }
    if data.is_empty() {
        return Err("No hay nada que imprimir".to_string());
    }
    platform::print_raw(printer.trim(), job_name, data)
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    type Handle = *mut c_void;

    #[repr(C)]
    struct DocInfo1W {
        p_doc_name: *mut u16,
        p_output_file: *mut u16,
        p_datatype: *mut u16,
    }

    #[repr(C)]
    struct PrinterInfo4W {
        p_printer_name: *mut u16,
        p_server_name: *mut u16,
        attributes: u32,
    }

    #[link(name = "winspool")]
    extern "system" {
        fn OpenPrinterW(name: *mut u16, handle: *mut Handle, defaults: *mut c_void) -> i32;
        fn ClosePrinter(handle: Handle) -> i32;
        fn StartDocPrinterW(handle: Handle, level: u32, info: *mut u8) -> u32;
        fn EndDocPrinter(handle: Handle) -> i32;
        fn StartPagePrinter(handle: Handle) -> i32;
        fn EndPagePrinter(handle: Handle) -> i32;
        fn WritePrinter(handle: Handle, buf: *mut c_void, len: u32, written: *mut u32) -> i32;
        fn EnumPrintersW(
            flags: u32,
            name: *mut u16,
            level: u32,
            buf: *mut u8,
            buf_size: u32,
            needed: *mut u32,
            returned: *mut u32,
        ) -> i32;
    }

    const PRINTER_ENUM_LOCAL: u32 = 0x0000_0002;
    const PRINTER_ENUM_CONNECTIONS: u32 = 0x0000_0004;

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(once(0)).collect()
    }

    unsafe fn from_wide(ptr: *const u16) -> String {
        if ptr.is_null() {
            return String::new();
        }
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
    }

    pub fn list_printers() -> Result<Vec<String>, String> {
        unsafe {
            let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
            let mut needed = 0u32;
            let mut returned = 0u32;

            // Primera llamada: sirve solo para saber cuánto buffer hace falta.
            EnumPrintersW(flags, ptr::null_mut(), 4, ptr::null_mut(), 0, &mut needed, &mut returned);
            if needed == 0 {
                return Ok(Vec::new());
            }

            let mut buf = vec![0u8; needed as usize];
            let ok = EnumPrintersW(
                flags, ptr::null_mut(), 4,
                buf.as_mut_ptr(), needed, &mut needed, &mut returned,
            );
            if ok == 0 {
                return Err("No se pudo listar las impresoras del sistema".to_string());
            }

            let entries = buf.as_ptr() as *const PrinterInfo4W;
            Ok((0..returned as usize)
                .map(|i| from_wide((*entries.add(i)).p_printer_name))
                .filter(|n| !n.is_empty())
                .collect())
        }
    }

    pub fn print_raw(printer: &str, job_name: &str, data: &[u8]) -> Result<(), String> {
        unsafe {
            let mut name = wide(printer);
            let mut handle: Handle = ptr::null_mut();

            if OpenPrinterW(name.as_mut_ptr(), &mut handle, ptr::null_mut()) == 0 {
                return Err(format!(
                    "No se pudo abrir la impresora '{}'. Verifica que esté instalada y encendida.",
                    printer
                ));
            }

            // Guarda para que el handle se cierre pase lo que pase.
            struct Printer(Handle);
            impl Drop for Printer {
                fn drop(&mut self) {
                    unsafe { ClosePrinter(self.0) };
                }
            }
            let _guard = Printer(handle);

            let mut doc = wide(job_name);
            let mut datatype = wide("RAW");
            let mut info = DocInfo1W {
                p_doc_name: doc.as_mut_ptr(),
                p_output_file: ptr::null_mut(),
                p_datatype: datatype.as_mut_ptr(),
            };

            if StartDocPrinterW(handle, 1, &mut info as *mut _ as *mut u8) == 0 {
                return Err("La impresora rechazó el trabajo de impresión".to_string());
            }
            if StartPagePrinter(handle) == 0 {
                EndDocPrinter(handle);
                return Err("La impresora rechazó la página".to_string());
            }

            let mut written = 0u32;
            let ok = WritePrinter(
                handle,
                data.as_ptr() as *mut c_void,
                data.len() as u32,
                &mut written,
            );

            EndPagePrinter(handle);
            EndDocPrinter(handle);

            if ok == 0 {
                return Err("Falló el envío de datos a la impresora".to_string());
            }
            if written as usize != data.len() {
                return Err(format!(
                    "La impresora solo aceptó {} de {} bytes",
                    written,
                    data.len()
                ));
            }
            Ok(())
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use std::io::Write;
    use std::process::{Command, Stdio};

    /// En macOS y Linux se usa CUPS (`lpstat` / `lp`). Sirve para desarrollar y
    /// probar el ticket; el destino de producción es Windows.
    pub fn list_printers() -> Result<Vec<String>, String> {
        let out = Command::new("lpstat")
            .arg("-a")
            .output()
            .map_err(|e| format!("No se pudo consultar CUPS: {}", e))?;

        Ok(String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| l.split_whitespace().next())
            .map(|s| s.to_string())
            .collect())
    }

    pub fn print_raw(printer: &str, job_name: &str, data: &[u8]) -> Result<(), String> {
        let mut child = Command::new("lp")
            .args(["-d", printer, "-t", job_name, "-o", "raw"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| format!("No se pudo ejecutar lp: {}", e))?;

        child
            .stdin
            .as_mut()
            .ok_or("No se pudo escribir al proceso de impresión")?
            .write_all(data)
            .map_err(|e| e.to_string())?;

        let status = child.wait().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("lp terminó con error al imprimir en '{}'", printer))
        }
    }
}
