//! Construcción de tickets en ESC/POS.
//!
//! ESC/POS es el lenguaje que entienden prácticamente todas las impresoras
//! térmicas de tickets (Epson, Bixolon, Star en modo Epson, y la enorme mayoría
//! de las genéricas). Trabajar a este nivel es lo que permite imprimir sin
//! diálogo del sistema y, sobre todo, abrir el cajón de dinero: el cajón se
//! conecta a la impresora, no a la computadora.

/// Ancho en caracteres según el papel: 32 para 58 mm, 48 para 80 mm.
pub const DEFAULT_WIDTH: usize = 32;

const ESC: u8 = 0x1B;
const GS: u8 = 0x1D;

/// Comando por defecto para abrir el cajón: `ESC p 0 25 250` (pin 2).
/// Unos pocos cajones responden al pin 5 (`1B 70 01 19 FA`), por eso es
/// configurable desde Ajustes.
pub const DEFAULT_DRAWER_KICK: &str = "1B 70 00 19 FA";

/// Interpreta una cadena hexadecimal ("1B 70 00 19 FA") como bytes.
/// Acepta espacios, comas, guiones y dos puntos como separadores.
pub fn parse_hex_command(spec: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = spec
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',' && *c != '-' && *c != ':')
        .collect();

    if cleaned.is_empty() {
        return Err("El comando está vacío".to_string());
    }
    if !cleaned.len().is_multiple_of(2) {
        return Err("El comando debe tener un número par de dígitos hexadecimales".to_string());
    }

    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| format!("'{}' no es hexadecimal válido", &cleaned[i..i + 2]))
        })
        .collect()
}

/// Reemplaza acentos y símbolos que la mayoría de las impresoras no traen en su
/// página de códigos por defecto.
///
/// Un ticket legible en cualquier impresora vale más que uno bonito en algunas y
/// con basura en otras; por eso esta es la opción por defecto.
pub fn to_ascii(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'Á' | 'À' | 'Ä' | 'Â' => 'A',
            'É' | 'È' | 'Ë' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
            'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
            'ñ' => 'n',
            'Ñ' => 'N',
            '¿' => '?',
            '¡' => '!',
            '°' => 'o',
            '–' | '—' => '-',
            '\u{201C}' | '\u{201D}' => '"',
            '\u{2018}' | '\u{2019}' => '\'',
            '€' => 'E',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

/// Alinea una etiqueta a la izquierda y un importe a la derecha dentro del ancho
/// del papel. Si no caben juntos, el importe manda y la etiqueta se recorta.
pub fn line_pair(label: &str, value: &str, width: usize) -> String {
    let label = to_ascii(label);
    let value = to_ascii(value);

    if value.len() >= width {
        return value;
    }
    let room = width - value.len();
    if label.len() + 1 > room {
        let cut = room.saturating_sub(1);
        format!("{:<room$}{}", &label[..cut], value, room = room)
    } else {
        format!("{:<room$}{}", label, value, room = room)
    }
}

/// Parte un texto largo en renglones que quepan en el papel, sin cortar palabras
/// cuando se puede evitar.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let text = to_ascii(text);
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if word.len() > width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            for chunk in word.as_bytes().chunks(width) {
                lines.push(String::from_utf8_lossy(chunk).to_string());
            }
            continue;
        }
        if current.is_empty() {
            current = word.to_string();
        } else if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Acumula los bytes de un ticket.
pub struct Builder {
    buf: Vec<u8>,
    width: usize,
}

impl Builder {
    pub fn new(width: usize) -> Self {
        let width = if width == 0 { DEFAULT_WIDTH } else { width };
        // ESC @ reinicia la impresora a un estado conocido; sin esto el ticket
        // hereda el formato (negritas, tamaño) del anterior si algo falló.
        Builder { buf: vec![ESC, b'@'], width }
    }

    pub fn align_left(&mut self) -> &mut Self { self.raw(&[ESC, b'a', 0]) }
    pub fn align_center(&mut self) -> &mut Self { self.raw(&[ESC, b'a', 1]) }
    pub fn bold(&mut self, on: bool) -> &mut Self { self.raw(&[ESC, b'E', on as u8]) }

    /// Doble alto y doble ancho, para el total.
    pub fn double_size(&mut self, on: bool) -> &mut Self {
        self.raw(&[GS, b'!', if on { 0x11 } else { 0x00 }])
    }

    pub fn raw(&mut self, bytes: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(bytes);
        self
    }

    pub fn text(&mut self, text: &str) -> &mut Self {
        let rendered = to_ascii(text);
        self.buf.extend_from_slice(rendered.as_bytes());
        self
    }

    pub fn line(&mut self, text: &str) -> &mut Self {
        self.text(text);
        self.raw(b"\n")
    }

    /// Renglón de etiqueta + importe justificado al ancho del papel.
    pub fn pair(&mut self, label: &str, value: &str) -> &mut Self {
        let rendered = line_pair(label, value, self.width);
        self.buf.extend_from_slice(rendered.as_bytes());
        self.raw(b"\n")
    }

    pub fn separator(&mut self) -> &mut Self {
        let rule = "-".repeat(self.width);
        self.line(&rule)
    }

    pub fn feed(&mut self, lines: u8) -> &mut Self {
        self.raw(&[ESC, b'd', lines])
    }

    /// Corte parcial del papel. Las impresoras sin cortador lo ignoran.
    pub fn cut(&mut self) -> &mut Self {
        self.raw(&[GS, b'V', 66, 0])
    }

    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_commands_accept_the_usual_separators() {
        let expected = vec![0x1B, 0x70, 0x00, 0x19, 0xFA];
        assert_eq!(parse_hex_command("1B 70 00 19 FA").unwrap(), expected);
        assert_eq!(parse_hex_command("1b7000 19fa").unwrap(), expected);
        assert_eq!(parse_hex_command("1B,70,00,19,FA").unwrap(), expected);
        assert_eq!(parse_hex_command("1B-70-00-19-FA").unwrap(), expected);
    }

    #[test]
    fn malformed_hex_commands_are_rejected_instead_of_sent() {
        assert!(parse_hex_command("").is_err());
        assert!(parse_hex_command("1B 7").is_err());
        assert!(parse_hex_command("ZZ 70").is_err());
    }

    #[test]
    fn the_default_kick_is_the_pin_2_command() {
        assert_eq!(
            parse_hex_command(DEFAULT_DRAWER_KICK).unwrap(),
            vec![0x1B, 0x70, 0x00, 0x19, 0xFA]
        );
    }

    #[test]
    fn spanish_text_survives_a_printer_without_accents() {
        assert_eq!(to_ascii("Camisa Niña"), "Camisa Nina");
        assert_eq!(to_ascii("¡Gracias por su compra!"), "!Gracias por su compra!");
        assert_eq!(to_ascii("Pantalón Café"), "Pantalon Cafe");
    }

    #[test]
    fn totals_line_up_on_the_right_edge() {
        let line = line_pair("Subtotal", "$249.00", 32);
        assert_eq!(line.len(), 32);
        assert!(line.ends_with("$249.00"));
    }

    #[test]
    fn a_long_product_name_is_trimmed_but_the_amount_survives() {
        let line = line_pair("Sudadera con capucha extra grande", "$1,299.00", 32);
        assert_eq!(line.len(), 32);
        assert!(line.ends_with("$1,299.00"));
    }

    #[test]
    fn wrapping_never_exceeds_the_paper_width() {
        for line in wrap("Gracias por su compra, vuelva pronto a Things Shop", 32) {
            assert!(line.len() <= 32, "renglón demasiado largo: {}", line);
        }
    }

    #[test]
    fn wrapping_splits_words_longer_than_the_paper() {
        let lines = wrap("ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789", 10);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| l.len() <= 10));
    }

    #[test]
    fn every_ticket_starts_by_resetting_the_printer() {
        let bytes = Builder::new(32).finish();
        assert_eq!(&bytes[..2], &[0x1B, b'@']);
    }

    #[test]
    fn a_zero_width_falls_back_to_the_58mm_default() {
        let mut b = Builder::new(0);
        b.separator();
        let rendered = String::from_utf8_lossy(&b.finish()).to_string();
        assert!(rendered.contains(&"-".repeat(DEFAULT_WIDTH)));
    }
}
