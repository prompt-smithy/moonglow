//! The night-to-day colour temperature window, edited as one value.
//!
//! Ported from `Models/TemperatureRange.swift`. Every constructor and mutation
//! keeps `night <= day` and snaps to the kelvin step, so UI code never has to
//! defend the invariant itself.

use crate::settings::{AppSettings, KELVIN_MAX, KELVIN_MIN, KELVIN_STEP};

/// The night-to-day colour temperature window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemperatureRange {
    /// Warmer end of the window — the night target.
    night: i32,
    /// Cooler end of the window — the day target.
    day: i32,
}

impl TemperatureRange {
    /// Builds a range from two raw values, snapping both and ordering them.
    pub fn new(night: i32, day: i32) -> Self {
        let snapped_night = Self::snap(night);
        let snapped_day = Self::snap(day);
        Self {
            night: snapped_night.min(snapped_day),
            day: snapped_night.max(snapped_day),
        }
    }

    /// The window currently stored in settings.
    pub fn from_settings(settings: &AppSettings) -> Self {
        Self::new(settings.night_color_temp_k, settings.day_color_temp_k)
    }

    pub const fn night(&self) -> i32 {
        self.night
    }

    pub const fn day(&self) -> i32 {
        self.day
    }

    /// Snaps a raw value to the nearest kelvin step and clamps it to range.
    pub fn snap(value: i32) -> i32 {
        let value = AppSettings::clamp_kelvin(value);
        let step = KELVIN_STEP as f64;
        let rounded = ((value as f64 / step).round() as i32) * KELVIN_STEP;
        AppSettings::clamp_kelvin(rounded)
    }

    /// Moves the night handle. It stops at the day handle rather than crossing
    /// it, so dragging past the other thumb pins the handles together instead
    /// of swapping them.
    pub fn moving_night(&self, value: i32) -> Self {
        Self::new(Self::snap(value).min(self.day), self.day)
    }

    /// Moves the day handle. It stops at the night handle rather than crossing it.
    pub fn moving_day(&self, value: i32) -> Self {
        Self::new(self.night, Self::snap(value).max(self.night))
    }

    /// Distance between the handles, in kelvin.
    pub const fn span(&self) -> i32 {
        self.day - self.night
    }

    /// `1200 K – 6500 K`, for the caption under the control.
    pub fn display_text(&self) -> String {
        format!("{} K – {} K", self.night, self.day)
    }
}

/// Converts between kelvin values and positions along a linear track.
///
/// Kept separate from the UI control so the geometry maths is testable without
/// a window; the control only supplies pixels.
pub struct TemperatureScale;

impl TemperatureScale {
    /// Position of a value along the range, as `0...1`.
    pub fn fraction(value: i32, range: (i32, i32)) -> f64 {
        let span = (range.1 - range.0) as f64;
        if span <= 0.0 {
            return 0.0;
        }
        let offset = (AppSettings::clamp_kelvin(value) - range.0) as f64;
        (offset / span).clamp(0.0, 1.0)
    }

    /// The snapped value at a `0...1` position. Positions beyond either end clamp.
    pub fn value_at_fraction(fraction: f64, range: (i32, i32)) -> i32 {
        let span = (range.1 - range.0) as f64;
        let raw = range.0 as f64 + fraction.clamp(0.0, 1.0) * span;
        let snapped = TemperatureRange::snap(raw.round() as i32);
        snapped.clamp(range.0, range.1)
    }

    /// Full kelvin bounds shared with settings so the two cannot drift apart.
    pub const fn bounds() -> (i32, i32) {
        (KELVIN_MIN, KELVIN_MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_and_snaps_endpoints() {
        let range = TemperatureRange::new(6530, 1150);
        assert_eq!(range.night(), 1200);
        assert_eq!(range.day(), 6500);
        assert_eq!(range.span(), 5300);
        assert_eq!(range.display_text(), "1200 K – 6500 K");
    }

    #[test]
    fn handles_pin_instead_of_crossing() {
        let range = TemperatureRange::new(1200, 6500);
        assert_eq!(range.moving_night(6600).night(), 6500);
        assert_eq!(range.moving_day(900).day(), 1200);
    }

    #[test]
    fn scale_converts_both_directions() {
        let bounds = TemperatureScale::bounds();
        assert_eq!(TemperatureScale::fraction(900, bounds), 0.0);
        assert_eq!(TemperatureScale::fraction(6600, bounds), 1.0);
        assert_eq!(TemperatureScale::value_at_fraction(0.0, bounds), 900);
        assert_eq!(TemperatureScale::value_at_fraction(1.0, bounds), 6600);
        // Midpoint of 900...6600 is 3750, which snaps to 3800.
        assert_eq!(TemperatureScale::value_at_fraction(0.5, bounds), 3800);
        assert_eq!(TemperatureScale::value_at_fraction(-1.0, bounds), 900);
        assert_eq!(TemperatureScale::value_at_fraction(2.0, bounds), 6600);
    }
}
