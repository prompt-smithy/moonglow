//! Persisted application settings.
//!
//! Ported from `Models/AppSettings.swift`. Range constants and defaults are
//! preserved exactly. The on-disk representation is camelCase JSON, and decoding
//! is lenient: any missing or malformed value falls back to its default rather
//! than failing the whole load.

use crate::shortcut::{Shortcut, ShortcutAction};
use crate::TimeOfDay;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Brightness range, in percent.
pub const BRIGHTNESS_MIN: i32 = 1;
pub const BRIGHTNESS_MAX: i32 = 100;
/// Colour temperature range in kelvin. Lower is warmer.
pub const KELVIN_MIN: i32 = 900;
pub const KELVIN_MAX: i32 = 6600;
/// Slider step and schedule rounding step.
pub const KELVIN_STEP: i32 = 100;

/// Persisted application settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_day")]
    pub day_color_temp_k: i32,
    #[serde(default = "default_night")]
    pub night_color_temp_k: i32,
    #[serde(default = "default_morning_start")]
    pub morning_start: TimeOfDay,
    #[serde(default = "default_morning_end")]
    pub morning_end: TimeOfDay,
    #[serde(default = "default_evening_start")]
    pub evening_start: TimeOfDay,
    #[serde(default = "default_evening_end")]
    pub evening_end: TimeOfDay,
    #[serde(default = "default_auto")]
    pub auto_color_temperature: bool,
    #[serde(default = "default_brightness")]
    pub manual_brightness: i32,
    #[serde(default = "default_manual_temp")]
    pub manual_color_temp_k: i32,
    /// Whether the user wants the app to start at login.
    ///
    /// This records intent only. The actual registration lives outside the
    /// settings file, so it is re-asserted at launch rather than trusted.
    #[serde(default)]
    pub launch_at_login: bool,
    /// Global shortcuts, stored as `"Ctrl+Alt+Up"` strings. `None` disables
    /// that action without clearing the others.
    #[serde(default = "default_increase_brightness_shortcut")]
    pub increase_brightness_shortcut: Option<Shortcut>,
    #[serde(default = "default_decrease_brightness_shortcut")]
    pub decrease_brightness_shortcut: Option<Shortcut>,
    #[serde(default = "default_increase_temperature_shortcut")]
    pub increase_temperature_shortcut: Option<Shortcut>,
    #[serde(default = "default_decrease_temperature_shortcut")]
    pub decrease_temperature_shortcut: Option<Shortcut>,
}

fn default_day() -> i32 {
    6500
}
fn default_night() -> i32 {
    1200
}
fn default_morning_start() -> TimeOfDay {
    TimeOfDay::new(6, 0)
}
fn default_morning_end() -> TimeOfDay {
    TimeOfDay::new(7, 0)
}
fn default_evening_start() -> TimeOfDay {
    TimeOfDay::new(17, 30)
}
fn default_evening_end() -> TimeOfDay {
    TimeOfDay::new(18, 30)
}
fn default_auto() -> bool {
    true
}
fn default_brightness() -> i32 {
    80
}
fn default_manual_temp() -> i32 {
    6500
}
fn shortcut_of(settings: &AppSettings, action: ShortcutAction) -> Option<Shortcut> {
    match action {
        ShortcutAction::IncreaseBrightness => settings.increase_brightness_shortcut,
        ShortcutAction::DecreaseBrightness => settings.decrease_brightness_shortcut,
        ShortcutAction::IncreaseTemperature => settings.increase_temperature_shortcut,
        ShortcutAction::DecreaseTemperature => settings.decrease_temperature_shortcut,
    }
}

fn set_shortcut(settings: &mut AppSettings, action: ShortcutAction, value: Option<Shortcut>) {
    match action {
        ShortcutAction::IncreaseBrightness => settings.increase_brightness_shortcut = value,
        ShortcutAction::DecreaseBrightness => settings.decrease_brightness_shortcut = value,
        ShortcutAction::IncreaseTemperature => settings.increase_temperature_shortcut = value,
        ShortcutAction::DecreaseTemperature => settings.decrease_temperature_shortcut = value,
    }
}

fn default_increase_brightness_shortcut() -> Option<Shortcut> {
    Some(ShortcutAction::IncreaseBrightness.default_shortcut())
}
fn default_decrease_brightness_shortcut() -> Option<Shortcut> {
    Some(ShortcutAction::DecreaseBrightness.default_shortcut())
}
fn default_increase_temperature_shortcut() -> Option<Shortcut> {
    Some(ShortcutAction::IncreaseTemperature.default_shortcut())
}
fn default_decrease_temperature_shortcut() -> Option<Shortcut> {
    Some(ShortcutAction::DecreaseTemperature.default_shortcut())
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            day_color_temp_k: default_day(),
            night_color_temp_k: default_night(),
            morning_start: default_morning_start(),
            morning_end: default_morning_end(),
            evening_start: default_evening_start(),
            evening_end: default_evening_end(),
            auto_color_temperature: default_auto(),
            manual_brightness: default_brightness(),
            manual_color_temp_k: default_manual_temp(),
            launch_at_login: false,
            increase_brightness_shortcut: default_increase_brightness_shortcut(),
            decrease_brightness_shortcut: default_decrease_brightness_shortcut(),
            increase_temperature_shortcut: default_increase_temperature_shortcut(),
            decrease_temperature_shortcut: default_decrease_temperature_shortcut(),
        }
    }
}

impl AppSettings {
    pub const fn default_morning_start() -> TimeOfDay {
        TimeOfDay::new(6, 0)
    }
    pub const fn default_morning_end() -> TimeOfDay {
        TimeOfDay::new(7, 0)
    }
    pub const fn default_evening_start() -> TimeOfDay {
        TimeOfDay::new(17, 30)
    }
    pub const fn default_evening_end() -> TimeOfDay {
        TimeOfDay::new(18, 30)
    }

    pub const fn clamp_brightness(value: i32) -> i32 {
        if value < BRIGHTNESS_MIN {
            BRIGHTNESS_MIN
        } else if value > BRIGHTNESS_MAX {
            BRIGHTNESS_MAX
        } else {
            value
        }
    }

    pub const fn clamp_kelvin(value: i32) -> i32 {
        if value < KELVIN_MIN {
            KELVIN_MIN
        } else if value > KELVIN_MAX {
            KELVIN_MAX
        } else {
            value
        }
    }

    /// Returns a copy with every value inside its supported range and every
    /// schedule time well formed.
    pub fn normalized(&self) -> Self {
        let mut copy = self.clone();
        copy.manual_brightness = Self::clamp_brightness(copy.manual_brightness);
        copy.manual_color_temp_k =
            crate::temperature_range::TemperatureRange::snap(copy.manual_color_temp_k);
        let targets = crate::temperature_range::TemperatureRange::from_settings(&copy);
        copy.day_color_temp_k = targets.day();
        copy.night_color_temp_k = targets.night();
        if !copy.morning_start.valid() {
            copy.morning_start = Self::default_morning_start();
        }
        if !copy.morning_end.valid() {
            copy.morning_end = Self::default_morning_end();
        }
        if !copy.evening_start.valid() {
            copy.evening_start = Self::default_evening_start();
        }
        if !copy.evening_end.valid() {
            copy.evening_end = Self::default_evening_end();
        }
        if !(copy.morning_start < copy.morning_end
            && copy.morning_end <= copy.evening_start
            && copy.evening_start < copy.evening_end)
        {
            copy.morning_start = Self::default_morning_start();
            copy.morning_end = Self::default_morning_end();
            copy.evening_start = Self::default_evening_start();
            copy.evening_end = Self::default_evening_end();
        }
        // One key combination must never fire two actions, even if the file
        // was edited by hand. Later assignments lose to earlier ones.
        let mut seen: Vec<Shortcut> = Vec::new();
        for action in ShortcutAction::ALL {
            let Some(shortcut) = shortcut_of(&copy, action) else {
                continue;
            };
            if seen.contains(&shortcut) {
                set_shortcut(&mut copy, action, None);
            } else {
                seen.push(shortcut);
            }
        }
        copy
    }
}

// Lenient decoding: `#[serde(default)]` covers missing keys, but a present yet
// malformed value would still fail the whole load. Decoding through a generic
// map and converting each value independently reproduces the Swift behaviour
// where every bad value falls back to its default.
impl<'de> Deserialize<'de> for AppSettings {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let map = HashMap::<String, serde_json::Value>::deserialize(deserializer)?;
        let defaults = AppSettings::default();
        let get = |key: &str| map.get(key);

        fn convert<T>(value: Option<&serde_json::Value>, fallback: T) -> T
        where
            T: Clone + for<'a> Deserialize<'a>,
        {
            value
                .and_then(|v| serde_json::from_value::<T>(v.clone()).ok())
                .unwrap_or(fallback)
        }

        // `null` deliberately clears a shortcut, so a present null must not
        // fall back to the default the way a malformed value does.
        fn shortcut(
            value: Option<&serde_json::Value>,
            fallback: Option<Shortcut>,
        ) -> Option<Shortcut> {
            match value {
                None => fallback,
                Some(serde_json::Value::Null) => None,
                Some(v) => serde_json::from_value::<Shortcut>(v.clone())
                    .ok()
                    .or(fallback),
            }
        }

        // Unknown keys (older theme keys, removed clock keys) are ignored.
        Ok(AppSettings {
            day_color_temp_k: convert(get("dayColorTempK"), defaults.day_color_temp_k),
            night_color_temp_k: convert(get("nightColorTempK"), defaults.night_color_temp_k),
            morning_start: convert(get("morningStart"), defaults.morning_start),
            morning_end: convert(get("morningEnd"), defaults.morning_end),
            evening_start: convert(get("eveningStart"), defaults.evening_start),
            evening_end: convert(get("eveningEnd"), defaults.evening_end),
            auto_color_temperature: convert(
                get("autoColorTemperature"),
                defaults.auto_color_temperature,
            ),
            manual_brightness: convert(get("manualBrightness"), defaults.manual_brightness),
            manual_color_temp_k: convert(get("manualColorTempK"), defaults.manual_color_temp_k),
            launch_at_login: convert(get("launchAtLogin"), defaults.launch_at_login),
            increase_brightness_shortcut: shortcut(
                get("increaseBrightnessShortcut"),
                defaults.increase_brightness_shortcut,
            ),
            decrease_brightness_shortcut: shortcut(
                get("decreaseBrightnessShortcut"),
                defaults.decrease_brightness_shortcut,
            ),
            increase_temperature_shortcut: shortcut(
                get("increaseTemperatureShortcut"),
                defaults.increase_temperature_shortcut,
            ),
            decrease_temperature_shortcut: shortcut(
                get("decreaseTemperatureShortcut"),
                defaults.decrease_temperature_shortcut,
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_brightness_and_kelvin() {
        assert_eq!(AppSettings::clamp_brightness(0), 1);
        assert_eq!(AppSettings::clamp_brightness(101), 100);
        assert_eq!(AppSettings::clamp_kelvin(800), 900);
        assert_eq!(AppSettings::clamp_kelvin(6700), 6600);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn normalizes_out_of_range_values() {
        let mut bad = AppSettings::default();
        bad.manual_brightness = 500;
        bad.day_color_temp_k = 100;
        bad.morning_start = TimeOfDay::new(25, 0);
        let fixed = bad.normalized();
        assert_eq!(fixed.manual_brightness, 100);
        assert_eq!(fixed.day_color_temp_k, 1200);
        assert_eq!(fixed.night_color_temp_k, 900);
        assert_eq!(fixed.morning_start, AppSettings::default_morning_start());
    }

    #[test]
    fn unparseable_schedule_time_falls_back_to_default() {
        let json = r#"{
            "morningStart": "not-a-time",
            "morningEnd": "07:00",
            "eveningStart": "17:30",
            "eveningEnd": "18:30"
        }"#;
        let decoded: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(decoded.morning_start, AppSettings::default_morning_start());
        assert_eq!(decoded.morning_end, TimeOfDay::new(7, 0));
    }

    #[test]
    fn ignores_unknown_keys_and_wrong_types() {
        let json = r#"{
            "manualBrightness": "loud",
            "use24HourClock": true,
            "theme": "sunset",
            "dayColorTempK": 5000
        }"#;
        let decoded: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(decoded.manual_brightness, default_brightness());
        assert_eq!(decoded.day_color_temp_k, 5000);
    }

    #[test]
    fn shortcuts_decode_leniently_and_null_clears() {
        let json = r#"{
            "increaseBrightnessShortcut": "Ctrl+Shift+F1",
            "decreaseBrightnessShortcut": null,
            "increaseTemperatureShortcut": "not a shortcut",
            "decreaseTemperatureShortcut": 42
        }"#;
        let decoded: AppSettings = serde_json::from_str(json).unwrap();
        assert_eq!(
            decoded.increase_brightness_shortcut,
            Some("Ctrl+Shift+F1".parse().unwrap())
        );
        assert_eq!(decoded.decrease_brightness_shortcut, None);
        // Malformed values fall back to the default instead of failing the load.
        assert_eq!(
            decoded.increase_temperature_shortcut,
            Some(ShortcutAction::IncreaseTemperature.default_shortcut())
        );
        assert_eq!(
            decoded.decrease_temperature_shortcut,
            Some(ShortcutAction::DecreaseTemperature.default_shortcut())
        );
        // A missing key falls back to the default too.
        let missing: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(
            missing.increase_brightness_shortcut,
            Some(ShortcutAction::IncreaseBrightness.default_shortcut())
        );
    }

    #[test]
    fn duplicate_shortcuts_are_resolved_on_normalize() {
        let mut settings = AppSettings::default();
        let duplicate: Shortcut = "Ctrl+Shift+F10".parse().unwrap();
        settings.increase_brightness_shortcut = Some(duplicate);
        settings.decrease_temperature_shortcut = Some(duplicate);
        let fixed = settings.normalized();
        assert_eq!(fixed.increase_brightness_shortcut, Some(duplicate));
        assert_eq!(fixed.decrease_temperature_shortcut, None);
    }

    #[test]
    fn round_trips_through_json() {
        let settings = AppSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, settings);
        assert!(json.contains("morningStart"));
    }
}
