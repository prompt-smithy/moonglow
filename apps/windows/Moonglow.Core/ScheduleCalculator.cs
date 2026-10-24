namespace Moonglow.Core;

/// <summary>Pure schedule evaluation: wall-clock time to color temperature.</summary>
/// <remarks>
/// Ported from <c>ScheduleService.swift</c> and <c>schedule.rs</c>. The five states, in order:
///
/// 1. Night target before the morning transition.
/// 2. Cooling interpolation during the morning transition.
/// 3. Day target between the transitions.
/// 4. Warming interpolation during the evening transition.
/// 5. Night target after the evening transition.
///
/// Results are rounded to <see cref="AppSettings.KelvinStep"/> and pinned by
/// <c>spec/vectors/schedule.json</c>.
/// </remarks>
public static class ScheduleCalculator
{
    public static int TemperatureFor(AppSettings settings, TimeSpan timeOfDay)
    {
        int dayK = settings.DayColorTempK;
        int nightK = settings.NightColorTempK;

        TimeSpan morningStart = settings.MorningStartTime;
        TimeSpan morningEnd = settings.MorningEndTime;
        TimeSpan eveningStart = settings.EveningStartTime;
        TimeSpan eveningEnd = settings.EveningEndTime;

        if (timeOfDay < morningStart)
            return nightK;
        if (timeOfDay < morningEnd)
            return Interpolate(timeOfDay, morningStart, morningEnd, nightK, dayK);
        if (timeOfDay < eveningStart)
            return dayK;
        if (timeOfDay < eveningEnd)
            return Interpolate(timeOfDay, eveningStart, eveningEnd, dayK, nightK);
        return nightK;
    }

    /// <summary>Convenience overload for the live path, using the local clock.</summary>
    public static int TemperatureNow(AppSettings settings)
        => TemperatureFor(settings, DateTime.Now.TimeOfDay);

    private static int Interpolate(TimeSpan time, TimeSpan start, TimeSpan end, int fromK, int toK)
    {
        double span = (end - start).TotalMinutes;
        if (span <= 0)
            return Snap(toK);

        double progress = MathUtil.Clamp((time - start).TotalMinutes / span, 0.0, 1.0);
        return Snap(fromK + (toK - fromK) * progress);
    }

    // Round half away from zero so the platform matches the Swift and Rust implementations
    // exactly; .NET's default banker's rounding would disagree on the midpoint.
    private static int Snap(double kelvin)
        => (int)Math.Round(kelvin / AppSettings.KelvinStep, MidpointRounding.AwayFromZero)
            * AppSettings.KelvinStep;
}
