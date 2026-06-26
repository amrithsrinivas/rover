/// Color constants and theme helpers for the Rover dark-only client.
pub mod colors {
    use iced::Color;

    pub const SURFACE: Color = Color::from_rgb(0.067, 0.067, 0.094);
    pub const ELEVATED: Color = Color::from_rgb(0.094, 0.094, 0.125);
    pub const BORDER: Color = Color::from_rgb(0.118, 0.118, 0.165);
    pub const TEXT: Color = Color::from_rgb(0.894, 0.894, 0.925);
    pub const TEXT_MUTED: Color = Color::from_rgb(0.545, 0.545, 0.620);
    pub const ACCENT: Color = Color::from_rgb(0.388, 0.400, 0.945);
    pub const SUCCESS: Color = Color::from_rgb(0.133, 0.773, 0.369);
    pub const WARNING: Color = Color::from_rgb(0.961, 0.620, 0.043);
    pub const DANGER: Color = Color::from_rgb(0.937, 0.267, 0.267);
}

/// Returns the custom dark theme.
pub fn rover_theme() -> iced::Theme {
    iced::Theme::custom(
        "Rover Dark".into(),
        iced::theme::Palette {
            background: colors::SURFACE,
            text: colors::TEXT,
            primary: colors::ACCENT,
            success: colors::SUCCESS,
            danger: colors::DANGER,
        },
    )
}

/// Return a translucent version of a color (for badge backgrounds).
pub fn with_alpha(color: iced::Color, alpha: f32) -> iced::Color {
    iced::Color { a: alpha, ..color }
}

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
