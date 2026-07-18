/// Simple ANSI escape code parser for shell output.
/// Converts raw terminal output containing ANSI sequences into styled text segments.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Style {
    Reset,
    Bold,
    Fg(u8), // 0-7 basic, 8-15 bright
    Bg(u8),
}

#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub fg: Option<u8>,
    pub bg: Option<u8>,
    pub bold: bool,
}

/// Parse a line of shell output and split it into styled segments.
/// ANSI escape sequences are consumed; only visible text remains.
pub fn parse_ansi_line(line: &str) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut current_fg = None;
    let mut current_bg = None;
    let mut current_bold = false;
    let bytes: &[u8] = line.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Flush current text before applying style
            if !current_text.is_empty() {
                spans.push(StyledSpan {
                    text: std::mem::take(&mut current_text),
                    fg: current_fg,
                    bg: current_bg,
                    bold: current_bold,
                });
            }
            // Skip ESC[
            i += 2;
            // Parse parameters until 'm'
            let start = i;
            while i < bytes.len() && bytes[i] != b'm' {
                i += 1;
            }
            let params = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
            if i < bytes.len() {
                i += 1; // skip 'm'
            }
            // Apply SGR parameters
            for param in params.split(';') {
                if let Ok(n) = param.parse::<u8>() {
                    match n {
                        0 => {
                            current_fg = None;
                            current_bg = None;
                            current_bold = false;
                        }
                        1 => current_bold = true,
                        22 => current_bold = false,
                        30..=37 => current_fg = Some(n - 30),
                        40..=47 => current_bg = Some(n - 40),
                        90..=97 => current_fg = Some(n - 90 + 8),
                        100..=107 => current_bg = Some(n - 100 + 8),
                        39 => current_fg = None,
                        49 => current_bg = None,
                        _ => {} // unsupported, ignore
                    }
                }
            }
        } else {
            current_text.push(bytes[i] as char);
            i += 1;
        }
    }

    // Flush remaining text
    if !current_text.is_empty() {
        spans.push(StyledSpan {
            text: current_text,
            fg: current_fg,
            bg: current_bg,
            bold: current_bold,
        });
    }

    spans
}

/// Convert a StyledSpan to an Iced color.
pub fn span_fg_color(fg: Option<u8>, bold: bool) -> iced::Color {
    match fg {
        Some(0) => iced::Color::from_rgb(0.0, 0.0, 0.0), // black
        Some(1) => iced::Color::from_rgb(0.8, 0.2, 0.2), // red
        Some(2) => iced::Color::from_rgb(0.2, 0.8, 0.2), // green
        Some(3) => iced::Color::from_rgb(0.8, 0.7, 0.2), // yellow
        Some(4) => iced::Color::from_rgb(0.2, 0.2, 0.8), // blue
        Some(5) => iced::Color::from_rgb(0.6, 0.2, 0.6), // magenta
        Some(6) => iced::Color::from_rgb(0.2, 0.6, 0.6), // cyan
        Some(7) => iced::Color::from_rgb(0.75, 0.75, 0.75), // white
        Some(8) => iced::Color::from_rgb(0.5, 0.5, 0.5), // bright black
        Some(9) => iced::Color::from_rgb(1.0, 0.3, 0.3), // bright red
        Some(10) => iced::Color::from_rgb(0.3, 1.0, 0.3), // bright green
        Some(11) => iced::Color::from_rgb(1.0, 0.9, 0.3), // bright yellow
        Some(12) => iced::Color::from_rgb(0.4, 0.4, 1.0), // bright blue
        Some(13) => iced::Color::from_rgb(0.8, 0.3, 0.8), // bright magenta
        Some(14) => iced::Color::from_rgb(0.3, 0.8, 0.8), // bright cyan
        Some(15) => iced::Color::from_rgb(1.0, 1.0, 1.0), // bright white
        _ => {
            if bold {
                iced::Color::from_rgb(1.0, 1.0, 1.0)
            } else {
                iced::Color::from_rgb(0.82, 0.83, 0.88)
            }
        }
    }
}

pub fn span_bg_color(bg: Option<u8>) -> iced::Color {
    match bg {
        Some(0) => iced::Color::from_rgb(0.0, 0.0, 0.0),
        Some(1) => iced::Color::from_rgb(0.55, 0.15, 0.15),
        Some(2) => iced::Color::from_rgb(0.15, 0.55, 0.15),
        Some(3) => iced::Color::from_rgb(0.55, 0.45, 0.15),
        Some(4) => iced::Color::from_rgb(0.15, 0.15, 0.55),
        Some(5) => iced::Color::from_rgb(0.4, 0.15, 0.4),
        Some(6) => iced::Color::from_rgb(0.15, 0.4, 0.4),
        Some(7) => iced::Color::from_rgb(0.55, 0.55, 0.55),
        _ => iced::Color::TRANSPARENT,
    }
}
