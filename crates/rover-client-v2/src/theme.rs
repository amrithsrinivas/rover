/// Design language: Humanist Technical Editorial
///
/// Colors behave like a limited set of physical inks on a technical-paper
/// substrate. The system combines a light notebook background with restrained
/// semantic colors, using borders and geometry for hierarchy.
use iced::{Border, Color, Shadow, Vector};

// ── Core ink system ──────────────────────────────────────────────────────────

/// Primary ink — near-black with blue/slate undertone. Used for primary text,
/// diagrams, strong borders, key structural elements.
pub const INK_PRIMARY: Color = Color::from_rgb(
    0x13 as f32 / 255.0,
    0x21 as f32 / 255.0,
    0x3C as f32 / 255.0,
);

/// Secondary ink — muted slate blue-gray. Used for supporting text, metadata,
/// inactive navigation, secondary descriptions.
pub const INK_SECONDARY: Color = Color::from_rgb(
    0x54 as f32 / 255.0,
    0x60 as f32 / 255.0,
    0x7A as f32 / 255.0,
);

/// Interaction blue — strong, intelligent blue. Used for links, selected
/// states, active relationships, key diagram paths, primary interactive
/// highlights.
pub const BLUE: Color = Color::from_rgb(
    0x1B as f32 / 255.0,
    0x3F as f32 / 255.0,
    0xA0 as f32 / 255.0,
);

/// Annotation red — restrained crimson. Used sparingly for annotations,
/// section indices, active diagram points, deployment markers, warnings.
pub const RED: Color = Color::from_rgb(
    0xD6 as f32 / 255.0,
    0x43 as f32 / 255.0,
    0x5B as f32 / 255.0,
);

// ── Surface system ───────────────────────────────────────────────────────────

/// Warm cream paper background — easy on eyes, not stark white.
pub const PAPER: Color = Color::from_rgb(
    0xF0 as f32 / 255.0,
    0xED as f32 / 255.0,
    0xE6 as f32 / 255.0,
);

/// Slightly elevated surface (panels, sidebars).
pub const PANEL: Color = Color::from_rgb(
    0xE8 as f32 / 255.0,
    0xE5 as f32 / 255.0,
    0xDE as f32 / 255.0,
);

/// Dark machine surface (terminal, logs, code).
pub const MACHINE: Color = Color::from_rgb(
    0x0E as f32 / 255.0,
    0x1A as f32 / 255.0,
    0x30 as f32 / 255.0,
);

/// Text on dark machine surface.
pub const MACHINE_TEXT: Color = Color::from_rgb(
    0xD0 as f32 / 255.0,
    0xD4 as f32 / 255.0,
    0xE0 as f32 / 255.0,
);

/// Muted text on dark machine surface.
pub const MACHINE_MUTED: Color = Color::from_rgb(
    0x6A as f32 / 255.0,
    0x72 as f32 / 255.0,
    0x8A as f32 / 255.0,
);

/// Graph-paper grid line color — barely visible.
pub const GRID_LINE: Color = Color::from_rgb(
    0xDD as f32 / 255.0,
    0xE0 as f32 / 255.0,
    0xE8 as f32 / 255.0,
);

/// Subtle structural border.
pub const BORDER: Color = Color::from_rgb(
    0xD0 as f32 / 255.0,
    0xD4 as f32 / 255.0,
    0xDC as f32 / 255.0,
);

/// Slightly stronger border for emphasis.
pub const BORDER_STRONG: Color = Color::from_rgb(
    0xB0 as f32 / 255.0,
    0xB8 as f32 / 255.0,
    0xC4 as f32 / 255.0,
);

// ── Semantic colors ──────────────────────────────────────────────────────────

pub const SUCCESS: Color = Color::from_rgb(
    0x27 as f32 / 255.0,
    0x8B as f32 / 255.0,
    0x5A as f32 / 255.0,
);

pub const WARNING: Color = Color::from_rgb(
    0xC4 as f32 / 255.0,
    0x85 as f32 / 255.0,
    0x1A as f32 / 255.0,
);

pub const DANGER: Color = Color::from_rgb(
    0xC0 as f32 / 255.0,
    0x3B as f32 / 255.0,
    0x3B as f32 / 255.0,
);

// ── Corner radii ─────────────────────────────────────────────────────────────

pub const RADIUS_SM: f32 = 3.0;
pub const RADIUS_MD: f32 = 5.0;
pub const RADIUS_LG: f32 = 7.0;

// ── Spacing scale ────────────────────────────────────────────────────────────

pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 20.0;
pub const SPACE_XL: f32 = 32.0;
pub const SPACE_2XL: f32 = 48.0;

pub const SIDEBAR_WIDTH: f32 = 200.0;
pub const STATUS_BAR_HEIGHT: f32 = 32.0;

// ── Type scale ───────────────────────────────────────────────────────────────

pub const TEXT_XS: u16 = 10;
pub const TEXT_SM: u16 = 12;
pub const TEXT_BASE: u16 = 14;
pub const TEXT_LG: u16 = 16;
pub const TEXT_XL: u16 = 20;
pub const TEXT_2XL: u16 = 26;
pub const TEXT_3XL: u16 = 34;

// ── Styling helpers ──────────────────────────────────────────────────────────

/// A 1px border with the standard border color.
pub fn border_1() -> Border {
    Border {
        color: BORDER,
        width: 1.0,
        radius: RADIUS_MD.into(),
    }
}

/// A 1px border with a specific radius.
pub fn border_with_radius(radius: f32) -> Border {
    Border {
        color: BORDER,
        width: 1.0,
        radius: radius.into(),
    }
}

/// A border with strong color for active/selected states.
pub fn border_active() -> Border {
    Border {
        color: BLUE,
        width: 1.5,
        radius: RADIUS_MD.into(),
    }
}

/// Subtle shadow for overlays.
pub fn shadow_overlay() -> Shadow {
    Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.12),
        offset: Vector::new(0.0, 4.0),
        blur_radius: 24.0,
    }
}

/// Return a translucent version of a color (for badge backgrounds).
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

/// Build the Iced theme from our palette.
pub fn rover_theme() -> iced::Theme {
    iced::Theme::custom(
        "Rover Notebook".into(),
        iced::theme::Palette {
            background: PAPER,
            text: INK_PRIMARY,
            primary: BLUE,
            success: SUCCESS,
            danger: DANGER,
        },
    )
}

// ── Formatting utilities ─────────────────────────────────────────────────────

/// Human-readable uptime string from seconds.
pub fn format_uptime(seconds: u32) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

/// Format bytes into human-readable form.
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Format a timestamp millis value into HH:MM:SS.
pub fn format_timestamp(millis: i64) -> String {
    let secs = millis / 1000;
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

/// Truncate a path to fit within a maximum character count, preserving both ends.
pub fn truncate_path(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(3) / 2;
    let start: String = value.chars().take(keep).collect();
    let end: String = value
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{start}...{end}")
}
