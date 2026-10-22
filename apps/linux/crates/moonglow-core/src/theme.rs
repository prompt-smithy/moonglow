//! Named palettes, ported from `Models/AppTheme.swift` and
//! `Services/ThemeCatalog.swift`.
//!
//! Colours stay as hex strings so the catalog is testable without a UI
//! framework; the settings window converts them.

/// One named palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppTheme {
    pub name: &'static str,
    pub is_dark: bool,
    pub window_background: &'static str,
    pub card_background: &'static str,
    pub control_background: &'static str,
    pub primary_text: &'static str,
    pub secondary_text: &'static str,
    pub border: &'static str,
    pub accent: &'static str,
    pub on_accent_text: &'static str,
    pub button_hover: &'static str,
    pub strong_text: &'static str,
}

impl AppTheme {
    /// Every surface colour in the palette, for audit-style tests.
    pub fn all_hex_colors(&self) -> [&'static str; 10] {
        [
            self.window_background,
            self.card_background,
            self.control_background,
            self.primary_text,
            self.secondary_text,
            self.border,
            self.accent,
            self.on_accent_text,
            self.button_hover,
            self.strong_text,
        ]
    }
}

/// The two built-in appearances used by the settings window.
pub struct ThemeCatalog;

impl ThemeCatalog {
    pub const LIGHT: AppTheme = AppTheme {
        name: "Light",
        is_dark: false,
        window_background: "#F5F5F5",
        card_background: "#FFFFFF",
        control_background: "#E8E8E8",
        primary_text: "#1D1D1F",
        secondary_text: "#6E6E73",
        border: "#D2D2D7",
        accent: "#007AFF",
        on_accent_text: "#FFFFFF",
        button_hover: "#E5E5EA",
        strong_text: "#1D1D1F",
    };

    pub const DARK: AppTheme = AppTheme {
        name: "Dark",
        is_dark: true,
        window_background: "#1C1C1E",
        card_background: "#2C2C2E",
        control_background: "#3A3A3C",
        primary_text: "#F5F5F7",
        secondary_text: "#AEAEB2",
        border: "#48484A",
        accent: "#0A84FF",
        on_accent_text: "#FFFFFF",
        button_hover: "#3A3A3C",
        strong_text: "#F5F5F7",
    };
}

/// Parses `#RRGGBB` into 8-bit channels.
pub fn parse_hex(color: &str) -> Option<(u8, u8, u8)> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_palette_color_parses() {
        for theme in [&ThemeCatalog::LIGHT, &ThemeCatalog::DARK] {
            for color in theme.all_hex_colors() {
                assert!(parse_hex(color).is_some(), "{} in {}", color, theme.name);
            }
        }
    }

    #[test]
    fn rejects_malformed_colors() {
        assert_eq!(parse_hex("#FFFFFF"), Some((255, 255, 255)));
        assert_eq!(parse_hex("FFFFFF"), None);
        assert_eq!(parse_hex("#FFF"), None);
        assert_eq!(parse_hex("#GGGGGG"), None);
    }
}
