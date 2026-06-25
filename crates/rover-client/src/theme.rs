/// Application-wide color constants for the Rover dark theme.
pub mod colors {
    use iced::Color;

    pub const BACKGROUND: Color = Color::from_rgb(0.08, 0.08, 0.12);
    pub const SURFACE: Color = Color::from_rgb(0.12, 0.12, 0.18);
    pub const SURFACE_HOVER: Color = Color::from_rgb(0.16, 0.16, 0.22);
    pub const BORDER: Color = Color::from_rgb(0.2, 0.2, 0.28);
    pub const TEXT: Color = Color::from_rgb(0.9, 0.9, 0.9);
    pub const TEXT_MUTED: Color = Color::from_rgb(0.5, 0.5, 0.55);
    pub const ACCENT: Color = Color::from_rgb(0.3, 0.6, 1.0);
    pub const SUCCESS: Color = Color::from_rgb(0.36, 0.72, 0.35);
    pub const WARNING: Color = Color::from_rgb(0.94, 0.68, 0.31);
    pub const DANGER: Color = Color::from_rgb(0.85, 0.33, 0.31);
    pub const PYTHON: Color = Color::from_rgb(0.42, 0.63, 0.80);
    pub const NODE: Color = Color::from_rgb(0.52, 0.73, 0.37);
    pub const GO: Color = Color::from_rgb(0.0, 0.68, 0.85);
    pub const RUST: Color = Color::from_rgb(0.84, 0.45, 0.24);
}

/// Re-export iced::Theme for use as the app theme type.
pub type RoverTheme = iced::Theme;
