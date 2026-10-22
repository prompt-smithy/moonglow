//! Pure colour-temperature math, ported from `Services/KelvinConversion.swift`.
//!
//! Kept separate from the gamma-table calls so it is fully unit testable.

/// Normalized per-channel scale factors for a kelvin temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbScale {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
}

/// One transfer table per channel.
#[derive(Debug, Clone, PartialEq)]
pub struct GammaRamp {
    pub red: Vec<f32>,
    pub green: Vec<f32>,
    pub blue: Vec<f32>,
}

/// Converts a kelvin temperature to normalized per-channel scale factors.
///
/// Values below 900 K and above 40000 K clamp before conversion, matching the original.
pub fn rgb_scale(kelvin: i32) -> RgbScale {
    let temperature = kelvin.clamp(900, 40000) as f64 / 100.0;

    let (red, green) = if temperature <= 66.0 {
        (
            255.0,
            clamp(
                99.4708025861 * temperature.ln() - 161.1195681661,
                0.0,
                255.0,
            ),
        )
    } else {
        (
            clamp(
                329.698727446 * (temperature - 60.0).powf(-0.1332047592),
                0.0,
                255.0,
            ),
            clamp(
                288.1221695283 * (temperature - 60.0).powf(-0.0755148492),
                0.0,
                255.0,
            ),
        )
    };

    let blue = if temperature >= 66.0 {
        255.0
    } else if temperature <= 19.0 {
        0.0
    } else {
        clamp(
            138.5177312231 * (temperature - 10.0).ln() - 305.0447927307,
            0.0,
            255.0,
        )
    };

    RgbScale {
        red: red / 255.0,
        green: green / 255.0,
        blue: blue / 255.0,
    }
}

/// Builds one transfer table per channel.
///
/// `capacity` is the backend's gamma table size. Each table is a linear ramp
/// scaled by the channel and brightness factors, clamped to `0...1`.
pub fn build_ramp(
    capacity: usize,
    red_scale: f64,
    green_scale: f64,
    blue_scale: f64,
    brightness_scale: f64,
) -> GammaRamp {
    if capacity == 0 {
        return GammaRamp {
            red: vec![],
            green: vec![],
            blue: vec![],
        };
    }
    let mut red = Vec::with_capacity(capacity);
    let mut green = Vec::with_capacity(capacity);
    let mut blue = Vec::with_capacity(capacity);
    for index in 0..capacity {
        let level = if capacity == 1 {
            1.0
        } else {
            index as f64 / (capacity - 1) as f64
        };
        red.push(clamp(level * red_scale * brightness_scale, 0.0, 1.0) as f32);
        green.push(clamp(level * green_scale * brightness_scale, 0.0, 1.0) as f32);
        blue.push(clamp(level * blue_scale * brightness_scale, 0.0, 1.0) as f32);
    }
    GammaRamp { red, green, blue }
}

fn clamp(value: f64, lower: f64, upper: f64) -> f64 {
    value.clamp(lower, upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warm_temperature_reduces_blue_channel() {
        let warm = rgb_scale(1200);
        let neutral = rgb_scale(6500);
        assert!(warm.blue < neutral.blue);
    }

    #[test]
    fn nine_hundred_kelvin_is_warmer_than_twelve_hundred() {
        // Below ~1900 K blue is clamped to zero, so warmth is compared on green.
        let warmest = rgb_scale(900);
        let warmer = rgb_scale(1200);
        assert!(warmest.green < warmer.green);
    }

    #[test]
    fn blue_suppressed_at_and_below_floor() {
        assert_eq!(rgb_scale(900).blue, 0.0);
        assert_eq!(rgb_scale(1900).blue, 0.0);
        assert!(rgb_scale(2000).blue > 0.0);
    }

    #[test]
    fn blue_increases_with_temperature_above_floor() {
        let cooler = rgb_scale(2000);
        let warmer = rgb_scale(3000);
        let neutral = rgb_scale(6500);
        assert!(cooler.blue < warmer.blue);
        assert!(warmer.blue < neutral.blue);
    }

    #[test]
    fn channel_scales_stay_normalized() {
        for kelvin in [900, 1200, 2700, 3500, 5000, 6500, 6600] {
            let scale = rgb_scale(kelvin);
            for channel in [scale.red, scale.green, scale.blue] {
                assert!((0.0..=1.0).contains(&channel), "{kelvin}: {channel}");
            }
        }
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        assert_eq!(rgb_scale(100), rgb_scale(900));
        assert_eq!(rgb_scale(99_999), rgb_scale(40_000));
    }

    #[test]
    fn ramp_spans_full_range_and_is_monotonic() {
        let ramp = build_ramp(256, 1.0, 0.5, 0.25, 1.0);
        assert_eq!(ramp.red.len(), 256);
        assert_eq!(ramp.red.first(), Some(&0.0));
        assert_eq!(ramp.red.last(), Some(&1.0));
        assert_eq!(ramp.green.last(), Some(&0.5));
        assert_eq!(ramp.blue.last(), Some(&0.25));
        for i in 1..ramp.red.len() {
            assert!(ramp.red[i] >= ramp.red[i - 1]);
        }
    }

    #[test]
    fn brightness_scale_dims_ramp() {
        let ramp = build_ramp(3, 1.0, 0.8, 0.6, 0.5);
        assert_eq!(ramp.red, [0.0, 0.25, 0.5]);
        assert_eq!(ramp.green, [0.0, 0.2, 0.4]);
        // 0.15 is not exact in binary; compare with tolerance.
        assert_eq!(ramp.blue.len(), 3);
        assert!((ramp.blue[1] - 0.15).abs() < 1e-6);
        assert!((ramp.blue[2] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn single_entry_ramp_is_saturated() {
        let ramp = build_ramp(1, 1.0, 1.0, 1.0, 1.0);
        assert_eq!(ramp.red, [1.0]);
        assert_eq!(ramp.green, [1.0]);
        assert_eq!(ramp.blue, [1.0]);
    }

    #[test]
    fn zero_capacity_ramp_is_empty() {
        let ramp = build_ramp(0, 1.0, 1.0, 1.0, 1.0);
        assert!(ramp.red.is_empty() && ramp.green.is_empty() && ramp.blue.is_empty());
    }
}
