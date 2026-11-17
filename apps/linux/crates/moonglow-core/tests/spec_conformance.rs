//! Conformance against the shared contract in `spec/vectors`.
//!
//! These vectors are the only thing the three platform implementations share. If one of
//! these fails, this crate has drifted from the contract — fix the implementation, or change
//! the spec and update every platform together.

use std::path::PathBuf;

use moonglow_core::brightness_plan;
use moonglow_core::kelvin;
use moonglow_core::schedule::ScheduleCalculator;
use moonglow_core::settings::{AppSettings, SOFTWARE_BRIGHTNESS_FLOOR};
use moonglow_core::temperature_range::TemperatureRange;
use moonglow_core::time_of_day::TimeOfDay;
use serde::Deserialize;

fn vectors_dir() -> PathBuf {
    // crates/moonglow-core -> crates -> linux -> apps -> repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("crate lives inside the monorepo")
        .join("spec/vectors")
}

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = vectors_dir().join(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()))
}

// MARK: - Kelvin

#[derive(Deserialize)]
struct KelvinDocument {
    vectors: Vec<KelvinRow>,
}

#[derive(Deserialize)]
struct KelvinRow {
    kelvin: i32,
    red: f64,
    green: f64,
    blue: f64,
}

#[test]
fn kelvin_vectors_match_within_1e_6() {
    let document: KelvinDocument = load("kelvin.json");
    assert!(!document.vectors.is_empty(), "no kelvin vectors");

    for row in document.vectors {
        let scale = kelvin::rgb_scale(row.kelvin);
        assert!(
            (scale.red - row.red).abs() <= 1e-6,
            "red at {} K: {} != {}",
            row.kelvin,
            scale.red,
            row.red
        );
        assert!(
            (scale.green - row.green).abs() <= 1e-6,
            "green at {} K: {} != {}",
            row.kelvin,
            scale.green,
            row.green
        );
        assert!(
            (scale.blue - row.blue).abs() <= 1e-6,
            "blue at {} K: {} != {}",
            row.kelvin,
            scale.blue,
            row.blue
        );
    }
}

// MARK: - Schedule

#[derive(Deserialize)]
struct ScheduleDocument {
    settings: std::collections::HashMap<String, Profile>,
    cases: Vec<ScheduleCase>,
}

#[derive(Deserialize)]
struct ScheduleCase {
    settings: String,
    time: String,
    kelvin: i32,
}

#[derive(Deserialize)]
struct Profile {
    #[serde(rename = "dayColorTempK")]
    day_color_temp_k: i32,
    #[serde(rename = "nightColorTempK")]
    night_color_temp_k: i32,
    #[serde(rename = "morningStart")]
    morning_start: [i32; 2],
    #[serde(rename = "morningEnd")]
    morning_end: [i32; 2],
    #[serde(rename = "eveningStart")]
    evening_start: [i32; 2],
    #[serde(rename = "eveningEnd")]
    evening_end: [i32; 2],
    #[serde(rename = "autoColorTemperature")]
    auto_color_temperature: bool,
    #[serde(rename = "manualBrightness")]
    manual_brightness: i32,
    #[serde(default, rename = "hardwareBrightnessEnabled")]
    hardware_brightness_enabled: Option<bool>,
    #[serde(default, rename = "softwareBrightnessEnabled")]
    software_brightness_enabled: Option<bool>,
    #[serde(default, rename = "splitBrightnessControls")]
    split_brightness_controls: Option<bool>,
    #[serde(default, rename = "manualHardwareBrightness")]
    manual_hardware_brightness: Option<i32>,
    #[serde(default, rename = "manualSoftwareBrightness")]
    manual_software_brightness: Option<i32>,
    #[serde(rename = "manualColorTempK")]
    manual_color_temp_k: i32,
    #[serde(rename = "launchAtLogin")]
    launch_at_login: bool,
}

impl Profile {
    #[allow(clippy::field_reassign_with_default)]
    fn to_settings(&self) -> AppSettings {
        let mut settings = AppSettings::default();
        settings.day_color_temp_k = self.day_color_temp_k;
        settings.night_color_temp_k = self.night_color_temp_k;
        settings.morning_start = pair(self.morning_start);
        settings.morning_end = pair(self.morning_end);
        settings.evening_start = pair(self.evening_start);
        settings.evening_end = pair(self.evening_end);
        settings.auto_color_temperature = self.auto_color_temperature;
        settings.manual_brightness = self.manual_brightness;
        if let Some(hw) = self.hardware_brightness_enabled {
            settings.hardware_brightness_enabled = hw;
        }
        if let Some(sw) = self.software_brightness_enabled {
            settings.software_brightness_enabled = sw;
        }
        if let Some(sp) = self.split_brightness_controls {
            settings.split_brightness_controls = sp;
        }
        if let Some(mhw) = self.manual_hardware_brightness {
            settings.manual_hardware_brightness = mhw;
        }
        if let Some(msw) = self.manual_software_brightness {
            settings.manual_software_brightness = msw;
        }
        settings.manual_color_temp_k = self.manual_color_temp_k;
        settings.launch_at_login = self.launch_at_login;
        settings
    }
}

fn pair([hour, minute]: [i32; 2]) -> TimeOfDay {
    TimeOfDay::new(hour, minute)
}

fn parse_time(text: &str) -> TimeOfDay {
    let (hour, minute) = text.split_once(':').expect("HH:mm");
    TimeOfDay::new(hour.parse().expect("hour"), minute.parse().expect("minute"))
}

#[test]
fn schedule_vectors_match_exactly() {
    let document: ScheduleDocument = load("schedule.json");
    assert!(!document.cases.is_empty(), "no schedule vectors");

    for entry in document.cases {
        let profile = document
            .settings
            .get(&entry.settings)
            .unwrap_or_else(|| panic!("unknown settings profile {}", entry.settings));
        let settings = profile.to_settings();
        let time = parse_time(&entry.time);

        let result = ScheduleCalculator::temperature_for(&settings, time);
        assert_eq!(
            result, entry.kelvin,
            "at {} with {}",
            entry.time, entry.settings
        );
    }
}

// MARK: - Temperature range

#[derive(Deserialize)]
struct RangeDocument {
    snap: Vec<SnapRow>,
    range: Vec<RangeRow>,
}

#[derive(Deserialize)]
struct SnapRow {
    input: i32,
    output: i32,
}

#[derive(Deserialize)]
struct RangeRow {
    night: i32,
    day: i32,
    #[serde(rename = "nightOut")]
    night_out: i32,
    #[serde(rename = "dayOut")]
    day_out: i32,
}

#[test]
fn snapping_vectors_match_exactly() {
    let document: RangeDocument = load("temperature-range.json");

    for row in document.snap {
        assert_eq!(
            TemperatureRange::snap(row.input),
            row.output,
            "snap({})",
            row.input
        );
    }
}

#[test]
fn range_ordering_vectors_match_exactly() {
    let document: RangeDocument = load("temperature-range.json");

    for row in document.range {
        let range = TemperatureRange::new(row.night, row.day);
        assert_eq!(
            range.night(),
            row.night_out,
            "night for {}/{}",
            row.night,
            row.day
        );
        assert_eq!(
            range.day(),
            row.day_out,
            "day for {}/{}",
            row.night,
            row.day
        );
    }
}

#[test]
fn settings_normalization_vectors_match() {
    let document: serde_json::Value = load("settings.json");
    for row in document["cases"].as_array().unwrap() {
        let actual: AppSettings = serde_json::from_value(row["input"].clone()).unwrap();
        let actual = serde_json::to_value(actual.normalized()).unwrap();
        for (key, expected) in row["expected"].as_object().unwrap() {
            assert_eq!(&actual[key], expected, "{}: {}", row["name"], key);
        }
    }
}

#[test]
fn brightness_routing_vectors_match() {
    let document: serde_json::Value = load("brightness-routing.json");
    let floor = document["softwareBrightnessFloor"].as_i64().unwrap() as i32;
    assert_eq!(floor, SOFTWARE_BRIGHTNESS_FLOOR);

    for row in document["vectors"].as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        let master = row["master"].as_i64().unwrap() as i32;
        let hw_input = row["hardwareInput"].as_i64().unwrap() as i32;
        let sw_input = row["softwareInput"].as_i64().unwrap() as i32;
        let split = row["split"].as_bool().unwrap();
        let hw_avail = row["hardwareAvailable"].as_bool().unwrap();
        let sw_avail = row["softwareAvailable"].as_bool().unwrap();
        let rec_reg = row["recoveryRegistered"].as_bool().unwrap();

        let output = brightness_plan::compute_routing(
            master, split, hw_input, sw_input, hw_avail, sw_avail, rec_reg,
        );

        let expected_hw = row["expectedHardware"].as_i64().map(|v| v as i32);
        let expected_sw = row["expectedSoftware"].as_i64().unwrap() as i32;

        assert_eq!(output.hardware, expected_hw, "{name} hardware");
        assert_eq!(output.software, expected_sw, "{name} software");
    }

    for row in document["transitions"].as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        let hw_avail = row["hardwareAvailable"].as_bool().unwrap();
        let sw_avail = row["softwareAvailable"].as_bool().unwrap();
        let rec_reg = row["recoveryRegistered"].as_bool().unwrap();

        if let Some(expected_master) = row.get("expectedMaster").and_then(|v| v.as_i64()) {
            let hw = row["hardware"].as_i64().map(|v| v as i32);
            let sw = row["software"].as_i64().unwrap() as i32;
            let master = if row["preserveOutput"].as_bool() == Some(true) {
                brightness_plan::master_preserving_output(
                    brightness_plan::RoutingOutput {
                        hardware: hw,
                        software: sw,
                    },
                    hw_avail,
                    sw_avail,
                    rec_reg,
                )
            } else {
                brightness_plan::split_to_unified(hw.unwrap(), sw, hw_avail, sw_avail, rec_reg)
            };
            assert_eq!(master, expected_master as i32, "{name} master");
        } else {
            let master = row["master"].as_i64().unwrap() as i32;
            let split = brightness_plan::unified_to_split(master, hw_avail, sw_avail, rec_reg);
            assert_eq!(
                split.hardware,
                row["expectedHardware"].as_i64().unwrap() as i32,
                "{name} hardware"
            );
            assert_eq!(
                split.software,
                row["expectedSoftware"].as_i64().unwrap() as i32,
                "{name} software"
            );
        }
    }
}
