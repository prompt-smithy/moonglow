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
    public static func temperature(
        for settings: AppSettings,
        at date: Date,
        calendar: Calendar = .current
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
public final class ScheduleService {

    /// The .NET original evaluated every 10 seconds; kept identical because the cadence
    /// is user-visible during transitions.
    public static let tickInterval: TimeInterval = 10

    private let colorService: ColorTemperatureService
    private var timer: Timer?

    /// Supplies the current settings at tick time. Set by the view model.
    public var settingsProvider: () -> AppSettings = { .default }

    /// Invoked on the main thread after a temperature is successfully applied.
    public var onTemperatureApplied: ((Int) -> Void)?

    public init(colorService: ColorTemperatureService) {
        self.colorService = colorService
    }

    public var isRunning: Bool { timer != nil }

    /// Starts the schedule. Ticks immediately, then every ``tickInterval`` seconds.
    public func start() {
        guard timer == nil else { return }

        tick()

        let timer = Timer(timeInterval: Self.tickInterval, repeats: true) { [weak self] _ in
            self?.tick()
        }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    public func stop() {
        timer?.invalidate()
        timer = nil
    }

    /// Evaluates and applies the schedule once. No-op when automatic mode is off or the
    /// colour service is unavailable.
    public func tick() {
        let settings = settingsProvider()
        guard settings.autoColorTemperature, colorService.isSupported else { return }

        let kelvin = ScheduleCalculator.temperature(for: settings, at: Date())
        if colorService.apply(kelvin: kelvin) {
            onTemperatureApplied?(kelvin)
        }
    }
}
