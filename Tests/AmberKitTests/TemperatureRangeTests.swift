import Foundation
import Testing
@testable import AmberKit

/// Covers the two-handle colour temperature window: ordering, snapping, and the
/// fraction-to-value maths the control relies on for hit testing.
@Suite("Temperature range")
struct TemperatureRangeTests {

    @Test("Values snap to the kelvin step and stay inside the supported range")
    func snapping() {
        #expect(TemperatureRange.snap(900) == 900)
        #expect(TemperatureRange.snap(949) == 900)
        #expect(TemperatureRange.snap(950) == 1000)
        #expect(TemperatureRange.snap(6549) == 6500)
        #expect(TemperatureRange.snap(0) == 900)
        #expect(TemperatureRange.snap(99_999) == 6600)
    }

    @Test("Ordering is enforced however the endpoints are supplied")
    func orderingIsEnforced() {
        let inverted = TemperatureRange(night: 6500, day: 1200)
        #expect(inverted.night == 1200)
        #expect(inverted.day == 6500)

        let equal = TemperatureRange(night: 3000, day: 3000)
        #expect(equal.night == 3000)
        #expect(equal.day == 3000)
        #expect(equal.span == 0)
    }

    @Test("The night handle stops at the day handle instead of crossing it")
    func nightHandlePinsToDay() {
        let range = TemperatureRange(night: 1200, day: 6500)

        let dragged = range.movingNight(to: 7000)
        #expect(dragged.night == 6500)
        #expect(dragged.day == 6500)
    }

    @Test("The day handle stops at the night handle instead of crossing it")
    func dayHandlePinsToNight() {
        let range = TemperatureRange(night: 1200, day: 6500)

        let dragged = range.movingDay(to: 0)
        #expect(dragged.night == 1200)
        #expect(dragged.day == 1200)
    }

    @Test("Dragging a handle keeps the other endpoint untouched")
    func draggingKeepsOtherEndpoint() {
        let range = TemperatureRange(night: 1200, day: 6500)

        #expect(range.movingNight(to: 2000) == TemperatureRange(night: 2000, day: 6500))
        #expect(range.movingDay(to: 5000) == TemperatureRange(night: 1200, day: 5000))
    }

    @Test("Fractions run from zero at the warm end to one at the cool end")
    func fractions() {
        #expect(TemperatureScale.fraction(of: 900) == 0)
        #expect(TemperatureScale.fraction(of: 6600) == 1)
        #expect(TemperatureScale.fraction(of: 3750) == 0.5)
        #expect(TemperatureScale.fraction(of: 100) == 0)
        #expect(TemperatureScale.fraction(of: 99_999) == 1)
    }

    @Test("Fractions and values round-trip at every step")
    func fractionValueRoundTrip() {
        for value in stride(from: 900, through: 6600, by: 100) {
            let fraction = TemperatureScale.fraction(of: value)
            #expect(TemperatureScale.value(atFraction: fraction) == value)
        }
    }

    @Test("Positions beyond the track clamp to an end")
    func outOfTrackFractionsClamp() {
        #expect(TemperatureScale.value(atFraction: -1) == 900)
        #expect(TemperatureScale.value(atFraction: 2) == 6600)
    }

    @Test("The stored window survives a round trip through settings")
    func settingsRoundTrip() {
        var settings = AppSettings.default
        settings.nightColorTempK = 1000
        settings.dayColorTempK = 5000

        let range = TemperatureRange(settings: settings)
        #expect(range.night == 1000)
        #expect(range.day == 5000)
        #expect(range.displayText == "1000 K – 5000 K")
    }
}
