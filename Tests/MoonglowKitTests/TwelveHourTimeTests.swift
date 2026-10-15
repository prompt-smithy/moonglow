import Foundation
import Testing
@testable import MoonglowKit

/// Covers the twelve-hour presentation layer: the part a naive `hour % 12` gets wrong
/// is midnight and noon, so both are pinned explicitly.
@Suite("Twelve-hour clock")
struct TwelveHourTimeTests {

    @Test("Midnight and noon read as twelve, not zero", arguments: [
        (TimeOfDay(hour: 0, minute: 0), "12:00 AM"),
        (TimeOfDay(hour: 12, minute: 0), "12:00 PM"),
        (TimeOfDay(hour: 0, minute: 30), "12:30 AM"),
        (TimeOfDay(hour: 12, minute: 30), "12:30 PM"),
    ])
    func midnightAndNoon(time: TimeOfDay, expected: String) {
        #expect(time.twelveHour.displayText == expected)
    }

    @Test("The meridiem switches at noon and midnight, not at one")
    func meridiemBoundaries() {
        #expect(TimeOfDay(hour: 11, minute: 59).twelveHour == TwelveHourTime(hour: 11, minute: 59, meridiem: .am))
        #expect(TimeOfDay(hour: 12, minute: 0).twelveHour == TwelveHourTime(hour: 12, minute: 0, meridiem: .pm))
        #expect(TimeOfDay(hour: 13, minute: 0).twelveHour == TwelveHourTime(hour: 1, minute: 0, meridiem: .pm))
        #expect(TimeOfDay(hour: 23, minute: 59).twelveHour == TwelveHourTime(hour: 11, minute: 59, meridiem: .pm))
    }

    @Test("Every minute of the day round-trips through the twelve-hour clock")
    func everyMinuteRoundTrips() {
        for minutes in 0..<1440 {
            let original = TimeOfDay.from(minutesSinceMidnight: minutes)
            let rebuilt = TimeOfDay(original.twelveHour)

            #expect(rebuilt == original, "Failed to round-trip \(original.storageDescription)")
        }
    }

    @Test("A reading never reports hour zero")
    func twelveHourRangeExcludesZero() {
        for minutes in 0..<1440 {
            let hour = TimeOfDay.from(minutesSinceMidnight: minutes).twelveHour.hour
            #expect((1...12).contains(hour))
        }
    }

    @Test("Out-of-range readings are rejected rather than wrapped")
    func invalidReadingsAreRejected() {
        #expect(TimeOfDay(TwelveHourTime(hour: 0, minute: 0, meridiem: .am)) == nil)
        #expect(TimeOfDay(TwelveHourTime(hour: 13, minute: 0, meridiem: .pm)) == nil)
        #expect(TimeOfDay(TwelveHourTime(hour: 6, minute: 60, meridiem: .am)) == nil)
        #expect(TimeOfDay(TwelveHourTime(hour: -1, minute: 0, meridiem: .am)) == nil)

        #expect(TwelveHourTime(validatingHour: 12, minute: 0, meridiem: .pm) != nil)
        #expect(!TwelveHourTime(hour: 0, minute: 0, meridiem: .am).isValid)
        #expect(TwelveHourTime(hour: 12, minute: 0, meridiem: .am).isValid)
    }

    @Test("Switching the meridiem keeps the twelve-hour value")
    func togglingMeridiemKeepsHourReadout() {
        let elevenAm = TimeOfDay(hour: 11, minute: 0).twelveHour
        let elevenPm = elevenAm.with(meridiem: .pm)

        #expect(elevenPm.displayText == "11:00 PM")
        #expect(TimeOfDay(elevenPm) == TimeOfDay(hour: 23, minute: 0))
    }

    @Test("Display text is always twelve-hour")
    func displayTextIsAlwaysTwelveHour() {
        let evening = TimeOfDay(hour: 17, minute: 45)
        let morning = TimeOfDay(hour: 6, minute: 0)
        let midnight = TimeOfDay(hour: 0, minute: 30)

        #expect(evening.displayText == "5:45 PM")
        #expect(morning.displayText == "6:00 AM")
        #expect(midnight.displayText == "12:30 AM")

        // Persistence is never localised, whatever the pickers show.
        #expect(evening.storageDescription == "17:45")
        #expect(midnight.storageDescription == "00:30")
    }

    @Test("The clock format never changes the persisted value")
    func clockFormatDoesNotAffectPersistence() throws {
        let date = TimeOfDay(hour: 0, minute: 5)

        // Persistence is always 24-hour `HH:mm`, whatever the pickers show.
        #expect(date.storageDescription == "00:05")

        let encoded = try JSONEncoder().encode(date)
        #expect(String(decoding: encoded, as: UTF8.self) == "\"00:05\"")

        let decoded = try JSONDecoder().decode(TimeOfDay.self, from: encoded)
        #expect(decoded == date)
        #expect(decoded.twelveHour.displayText == "12:05 AM")
    }

    @Test("Drop-down value lists cover every selectable hour, minute, and meridiem")
    func pickerValueLists() {
        #expect(TwelveHourTime.hourValues == Array(1...12))
        #expect(TwelveHourTime.hourRange == 1...12)

        #expect(TimeOfDay.minuteValues == Array(0...59))
        #expect(TwelveHourTime.minuteRange == 0...59)

        #expect(Meridiem.allCases.map(\.displayName) == ["AM", "PM"])
    }
}
