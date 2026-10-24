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
        window_background: "#F7F4EF",
        card_background: "#FFFFFF",
        control_background: "#EFE9DF",
        primary_text: "#201C16",
        secondary_text: "#6F6559",
        border: "#E4DACB",
        accent: "#B06514",
        on_accent_text: "#FFFFFF",
        button_hover: "#E8E0D4",
        strong_text: "#1C1813",
    };

    pub const DARK: AppTheme = AppTheme {
        name: "Dark",
        is_dark: true,
        window_background: "#0D1015",
        card_background: "#161B22",
        control_background: "#1F2630",
        primary_text: "#EAEFF6",
        secondary_text: "#98A2B3",
        border: "#27303D",
        accent: "#F2B45C",
        on_accent_text: "#1B1204",
        button_hover: "#232C38",
        strong_text: "#F7F9FC",
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
