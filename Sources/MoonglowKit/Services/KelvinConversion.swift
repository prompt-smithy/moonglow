import Foundation

/// Pure colour-temperature math, ported from `Services/MacColorTemperatureService.cs`.
///
/// Kept separate from the CoreGraphics calls so it is fully unit testable.
public enum KelvinConversion {

    /// Converts a kelvin temperature to normalized per-channel scale factors.
    ///
    /// Values below 900 K and above 40000 K clamp before conversion, matching the original.
    public static func rgbScale(kelvin: Int) -> (red: Double, green: Double, blue: Double) {
        let temperature = Double(min(max(kelvin, 900), 40000)) / 100.0

        let red: Double
        let green: Double
        let blue: Double

        if temperature <= 66 {
            red = 255
            green = clamp((99.4708025861 * log(temperature)) - 161.1195681661, 0, 255)
        } else {
            red = clamp(329.698727446 * pow(temperature - 60, -0.1332047592), 0, 255)
            green = clamp(288.1221695283 * pow(temperature - 60, -0.0755148492), 0, 255)
        }

        if temperature >= 66 {
            blue = 255
        } else if temperature <= 19 {
            blue = 0
        } else {
            blue = clamp((138.5177312231 * log(temperature - 10)) - 305.0447927307, 0, 255)
        }

        return (red / 255.0, green / 255.0, blue / 255.0)
    }

    /// Builds one transfer table per channel for `CGSetDisplayTransferByTable`.
    ///
    /// `capacity` comes from `CGDisplayGammaTableCapacity`. Each table is a linear ramp
    /// scaled by the channel and brightness factors, clamped to `0...1`.
    public static func buildRamp(
        capacity: Int,
        redScale: Double,
        greenScale: Double,
        blueScale: Double,
        brightnessScale: Double = 1
    ) -> (red: [Float], green: [Float], blue: [Float]) {
        guard capacity > 0 else { return ([], [], []) }

        var red = [Float](repeating: 0, count: capacity)
        var green = [Float](repeating: 0, count: capacity)
        var blue = [Float](repeating: 0, count: capacity)

        for index in 0..<capacity {
            let level = capacity == 1 ? 1.0 : Double(index) / Double(capacity - 1)
            red[index] = Float(clamp(level * redScale * brightnessScale, 0, 1))
            green[index] = Float(clamp(level * greenScale * brightnessScale, 0, 1))
            blue[index] = Float(clamp(level * blueScale * brightnessScale, 0, 1))
        }

        return (red, green, blue)
    }

    private static func clamp(_ value: Double, _ lower: Double, _ upper: Double) -> Double {
        min(max(value, lower), upper)
    }
}
