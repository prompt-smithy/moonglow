//! Persisted application settings.
//!
//! Ported from `Models/AppSettings.swift`. Range constants and defaults are
//! preserved exactly. The on-disk representation is camelCase JSON, and decoding
//! is lenient: any missing or malformed value falls back to its default rather
//! than failing the whole load.

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
        copy.manual_color_temp_k = Self::clamp_kelvin(copy.manual_color_temp_k);
        copy.day_color_temp_k = Self::clamp_kelvin(copy.day_color_temp_k);
        copy.night_color_temp_k = Self::clamp_kelvin(copy.night_color_temp_k);
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
    fn normalizes_out_of_range_values() {
        let mut bad = AppSettings::default();
        bad.manual_brightness = 500;
        bad.day_color_temp_k = 100;
        bad.morning_start = TimeOfDay::new(25, 0);
        let fixed = bad.normalized();
        assert_eq!(fixed.manual_brightness, 100);
        assert_eq!(fixed.day_color_temp_k, 900);
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
    fn round_trips_through_json() {
        let settings = AppSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, settings);
        assert!(json.contains("morningStart"));
    }
}
