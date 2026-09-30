import Testing
@testable import AmberKit

/// Ported from `ColorTemperatureTests` in the .NET test project.
@Suite("Kelvin conversion")
struct KelvinConversionTests {

    @Test("Warm temperatures reduce the blue channel")
    func warmTemperatureReducesBlueChannel() {
        let warm = KelvinConversion.rgbScale(kelvin: 1200)
        let neutral = KelvinConversion.rgbScale(kelvin: 6500)

        #expect(warm.blue < neutral.blue)
    }

    @Test("900 K is warmer than 1200 K")
    func nineHundredKelvinIsWarmerThanTwelveHundred() {
        // Below ~1900 K the blue channel is clamped to zero by the curve, so warmth has to
        // be compared on green: less green means warmer.
        let warmest = KelvinConversion.rgbScale(kelvin: 900)
        let warmer = KelvinConversion.rgbScale(kelvin: 1200)

        #expect(warmest.green < warmer.green)
    }

    @Test("Blue is fully suppressed at and below the ~1900 K floor")
    func blueIsSuppressedAtVeryWarmTemperatures() {
        #expect(KelvinConversion.rgbScale(kelvin: 900).blue == 0)
        #expect(KelvinConversion.rgbScale(kelvin: 1900).blue == 0)
        // Non-zero again just above the floor, which is why the warmth comparisons above
        // use green rather than blue.
        #expect(KelvinConversion.rgbScale(kelvin: 2000).blue > 0)
    }

    @Test("Blue increases with temperature above the floor")
    func blueIncreasesWithTemperature() {
        let cooler = KelvinConversion.rgbScale(kelvin: 2000)
        let warmer = KelvinConversion.rgbScale(kelvin: 3000)
        let neutral = KelvinConversion.rgbScale(kelvin: 6500)

        #expect(cooler.blue < warmer.blue)
        #expect(warmer.blue < neutral.blue)
    }

    @Test("Channel scales stay normalized", arguments: [900, 1200, 2700, 3500, 5000, 6500, 6600])
    func channelScalesStayNormalized(kelvin: Int) {
        let scale = KelvinConversion.rgbScale(kelvin: kelvin)

        for channel in [scale.red, scale.green, scale.blue] {
            #expect(channel >= 0.0)
            #expect(channel <= 1.0)
        }
    }

    @Test("Values outside the supported range are clamped, not rejected")
    func outOfRangeValuesAreClamped() {
        let belowFloor = KelvinConversion.rgbScale(kelvin: 100)
        let atFloor = KelvinConversion.rgbScale(kelvin: 900)
        let aboveCeiling = KelvinConversion.rgbScale(kelvin: 99_999)
        let atCeiling = KelvinConversion.rgbScale(kelvin: 40_000)

        #expect(belowFloor == atFloor)
        #expect(aboveCeiling == atCeiling)
    }

    @Test("Ramp spans the full range and is monotonic")
    func rampSpansFullRangeAndIsMonotonic() {
        let ramp = KelvinConversion.buildRamp(capacity: 256, redScale: 1.0, greenScale: 0.5, blueScale: 0.25)

        #expect(ramp.red.count == 256)
        #expect(ramp.red.first == 0.0)
        #expect(ramp.red.last == 1.0)
        // Green and blue are scaled, so their maxima are the scale factors.
        #expect(ramp.green.last == 0.5)
        #expect(ramp.blue.last == 0.25)

        for index in 1..<ramp.red.count {
            #expect(ramp.red[index] >= ramp.red[index - 1])
        }
    }

    @Test("Brightness scale dims all channels without changing ramp shape")
    func brightnessScaleDimsRamp() {
        let ramp = KelvinConversion.buildRamp(
            capacity: 3,
            redScale: 1,
            greenScale: 0.8,
            blueScale: 0.6,
            brightnessScale: 0.5
        )

        #expect(ramp.red == [0, 0.25, 0.5])
        #expect(ramp.green == [0, 0.2, 0.4])
        #expect(ramp.blue == [0, 0.15, 0.3])
    }

    @Test("A single-entry ramp is fully saturated rather than dividing by zero")
    func singleEntryRamp() {
        let ramp = KelvinConversion.buildRamp(capacity: 1, redScale: 1.0, greenScale: 1.0, blueScale: 1.0)

        #expect(ramp.red == [1.0])
        #expect(ramp.green == [1.0])
        #expect(ramp.blue == [1.0])
    }

    @Test("A zero-capacity ramp produces empty tables")
    func zeroCapacityRamp() {
        let ramp = KelvinConversion.buildRamp(capacity: 0, redScale: 1.0, greenScale: 1.0, blueScale: 1.0)

        #expect(ramp.red.isEmpty)
        #expect(ramp.green.isEmpty)
        #expect(ramp.blue.isEmpty)
    }
}
