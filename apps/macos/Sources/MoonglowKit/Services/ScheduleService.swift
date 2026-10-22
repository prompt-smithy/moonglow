import Foundation

/// Pure schedule evaluation.
///
/// Ported from `ScheduleService.ComputeTemperature` in `Services/ScheduleService.cs`.
/// The five states are, in order:
///
/// 1. Night target before the morning transition.
/// 2. Linear warming-to-cooling interpolation during the morning transition.
/// 3. Day target between the transitions.
/// 4. Linear cooling-to-warming interpolation during the evening transition.
/// 5. Night target after the evening transition.
///
/// Results are rounded to ``AppSettings/kelvinStep``.
public enum ScheduleCalculator {

    /// Computes the target temperature for a wall-clock time of day.
    public static func temperature(for settings: AppSettings, at time: TimeOfDay) -> Int {
        if time < settings.morningStart {
            return settings.nightColorTempK
        }
        if time < settings.morningEnd {
            return interpolate(
                time,
                from: settings.morningStart,
                to: settings.morningEnd,
                fromKelvin: settings.nightColorTempK,
                toKelvin: settings.dayColorTempK
            )
        }
        if time < settings.eveningStart {
            return settings.dayColorTempK
        }
        if time < settings.eveningEnd {
            return interpolate(
                time,
                from: settings.eveningStart,
                to: settings.eveningEnd,
                fromKelvin: settings.dayColorTempK,
                toKelvin: settings.nightColorTempK
            )
        }
        return settings.nightColorTempK
    }

    /// Convenience overload for the live path.
    ///
    /// Gregorian, because the schedule follows wall-clock hours and minutes and must not
    /// move with the user's calendar choice. `Calendar(identifier:)` takes the current time
    /// zone, so this also follows a time-zone change. ``ScheduleService`` holds its own copy
    /// rather than calling this every tick.
    public static func temperature(for settings: AppSettings, at date: Date) -> Int {
        temperature(for: settings, at: date, calendar: Calendar(identifier: .gregorian))
    }

    /// Overload for callers that must evaluate in a specific calendar.
    public static func temperature(
        for settings: AppSettings,
        at date: Date,
        calendar: Calendar
    ) -> Int {
        let parts = calendar.dateComponents([.hour, .minute], from: date)
        return temperature(
            for: settings,
            at: TimeOfDay(hour: parts.hour ?? 0, minute: parts.minute ?? 0)
        )
    }

    static func interpolate(
        _ time: TimeOfDay,
        from start: TimeOfDay,
        to end: TimeOfDay,
        fromKelvin: Int,
        toKelvin: Int
    ) -> Int {
        let span = Double(end.minutesSinceMidnight - start.minutesSinceMidnight)
        guard span > 0 else { return snap(Double(toKelvin)) }

        let elapsed = Double(time.minutesSinceMidnight - start.minutesSinceMidnight)
        let progress = min(max(elapsed / span, 0), 1)
        return snap(Double(fromKelvin) + (Double(toKelvin - fromKelvin) * progress))
    }

    static func snap(_ kelvin: Double) -> Int {
        Int((kelvin / Double(AppSettings.kelvinStep)).rounded()) * AppSettings.kelvinStep
    }
}

/// Drives the schedule on a fixed interval and applies the result.
///
/// Ported from `ScheduleService`. The timer runs on the main run loop, so
/// ``onTemperatureApplied`` is always delivered on the main thread and can update UI
/// state directly.
///
/// Main-thread confined by contract: every entry point is `@MainActor`, the timer fires
/// on the main run loop, the time-zone observer delivers on the main queue, and the
/// initializer only assigns immutable state. That is what makes the unchecked conformance
/// below sound.
public final class ScheduleService: @unchecked Sendable {

    /// The .NET original evaluated every 10 seconds; kept identical because the cadence
    /// is user-visible during transitions.
    public static let tickInterval: TimeInterval = 10

    private let colorService: ColorTemperatureService
    private var timer: Timer?

    /// Held rather than rebuilt per tick, because the tick runs every 10 s and resolving a
    /// calendar consults the user's locale settings. Rebuilt when the time zone changes so
    /// the schedule keeps following the clock the user sees.
    private var schedulingCalendar = Calendar(identifier: .gregorian)

    /// The time-zone observer, held so ``stop()`` can remove it.
    private var timeZoneObserver: NSObjectProtocol?

    /// The last applied target, so transitions — not every 10 s tick — reach the log.
    private var lastAppliedKelvin: Int?

    /// Supplies the current settings at tick time. Set by the view model.
    public var settingsProvider: @MainActor () -> AppSettings = { .default }

    /// Invoked on the main thread after a temperature is successfully applied.
    public var onTemperatureApplied: (@MainActor (Int) -> Void)?

    public init(colorService: ColorTemperatureService) {
        self.colorService = colorService
    }

    /// Starts the schedule. Ticks immediately, then every ``tickInterval`` seconds.
    @MainActor
    public func start() {
        guard timer == nil else { return }

        tick()

        let timer = Timer(timeInterval: Self.tickInterval, repeats: true) { [weak self] _ in
            // Scheduled on RunLoop.main below, so this always runs on the main thread.
            MainActor.assumeIsolated {
                self?.tick()
            }
        }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer

        observeTimeZoneChanges()
    }

    @MainActor
    public func stop() {
        timer?.invalidate()
        timer = nil
        if let timeZoneObserver {
            NotificationCenter.default.removeObserver(timeZoneObserver)
            self.timeZoneObserver = nil
        }
    }

    /// Rebuilds the calendar and re-evaluates when the user's time zone moves, so a
    /// transition lands on the local clock rather than the one captured at launch.
    ///
    /// `queue: .main` delivers on the actor the rest of the class is confined to.
    private func observeTimeZoneChanges() {
        guard timeZoneObserver == nil else { return }

        timeZoneObserver = NotificationCenter.default.addObserver(
            forName: .NSSystemTimeZoneDidChange,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.schedulingCalendar = Calendar(identifier: .gregorian)
                self.tick()
            }
        }
    }

    /// Evaluates and applies the schedule once. No-op when automatic mode is off or the
    /// colour service is unavailable.
    @MainActor
    public func tick() {
        let settings = settingsProvider()
        guard settings.autoColorTemperature, colorService.isSupported else { return }

        let kelvin = ScheduleCalculator.temperature(
            for: settings,
            at: Date(),
            calendar: schedulingCalendar
        )
        guard colorService.apply(kelvin: kelvin) else {
            AppLog.shared.log(.schedule, "schedule target \(kelvin)K applied to no display")
            return
        }
        if lastAppliedKelvin != kelvin {
            AppLog.shared.log(.schedule, "schedule target now \(kelvin)K")
            lastAppliedKelvin = kelvin
        }
        onTemperatureApplied?(kelvin)
    }
}
