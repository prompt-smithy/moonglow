//! One active display and the controls the app believes it can drive.
//!
//! Ported from `Models/DisplayInfo.swift`. `native_id` is a platform string
//! (X11 output id, DRM connector, Wayland output name) instead of a CoreGraphics id.

use serde::{Deserialize, Serialize};

/// One active display and its controllable capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayInfo {
    /// Platform-native identifier (X11 output, DRM connector, ...).
    pub native_id: String,
    pub name: String,
    /// Stable identifier used for display and IPC.
    pub identifier: String,
    pub is_primary: bool,
    pub supports_brightness: bool,
    pub supports_color_temperature: bool,
}

impl DisplayInfo {
    pub fn new(
        native_id: impl Into<String>,
        name: impl Into<String>,
        identifier: impl Into<String>,
        is_primary: bool,
        supports_brightness: bool,
        supports_color_temperature: bool,
    ) -> Self {
        Self {
            native_id: native_id.into(),
            name: name.into(),
            identifier: identifier.into(),
            is_primary,
            supports_brightness,
            supports_color_temperature,
        }
    }

    /// Human-readable capability line shown under the display name.
    pub fn capability_summary(&self) -> String {
        let mut capabilities = Vec::new();
        if self.supports_brightness {
            capabilities.push("brightness");
        }
        if self.supports_color_temperature {
            capabilities.push("color temperature");
        }
        if capabilities.is_empty() {
            return "No compatible controls".to_string();
        }
        let prefix = if self.is_primary { "Primary, " } else { "" };
        format!("{prefix}{}", capabilities.join(" and "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_capabilities() {
        let full = DisplayInfo::new("1", "DP-1", "1", true, true, true);
        assert_eq!(
            full.capability_summary(),
            "Primary, brightness and color temperature"
        );
        let temp_only = DisplayInfo::new("2", "HDMI-1", "2", false, false, true);
        assert_eq!(temp_only.capability_summary(), "color temperature");
        let none = DisplayInfo::new("3", "eDP-1", "3", false, false, false);
        assert_eq!(none.capability_summary(), "No compatible controls");
    }
}
