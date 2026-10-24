namespace Moonglow.Core;

/// <summary>The night-to-day color temperature window, edited as one value.</summary>
/// <remarks>
/// Ported from <c>Models/TemperatureRange.swift</c> and <c>temperature_range.rs</c>. Every
/// constructor and mutation keeps <c>night &lt;= day</c> and snaps to the kelvin step, so
/// callers never have to defend the invariant themselves. The ordering and snapping rules
/// are pinned by <c>spec/vectors/temperature-range.json</c>.
/// </remarks>
public readonly struct TemperatureRange
{
    /// <summary>Builds a range from two raw values, snapping both and ordering them.</summary>
    public TemperatureRange(int night, int day)
    {
        int snappedNight = Snap(night);
        int snappedDay = Snap(day);
        Night = Math.Min(snappedNight, snappedDay);
        Day = Math.Max(snappedNight, snappedDay);
    }

    /// <summary>The window currently stored in settings.</summary>
    public static TemperatureRange FromSettings(AppSettings settings)
        => new(settings.NightColorTempK, settings.DayColorTempK);

    /// <summary>Warmer end of the window — the night target.</summary>
    public int Night { get; }

    /// <summary>Cooler end of the window — the day target.</summary>
    public int Day { get; }

    /// <summary>Snaps a raw value to the nearest kelvin step and clamps it to range.</summary>
    public static int Snap(int value)
    {
        int snapped = (int)Math.Round(
            value / (double)AppSettings.KelvinStep,
            MidpointRounding.AwayFromZero) * AppSettings.KelvinStep;
        return AppSettings.ClampKelvin(snapped);
    }

    /// <summary>Moves the night handle. It stops at the day handle rather than crossing
    /// it, so dragging past the other thumb pins the handles together.</summary>
    public TemperatureRange MovingNight(int value)
        => new(Math.Min(Snap(value), Day), Day);

    /// <summary>Moves the day handle. It stops at the night handle rather than crossing it.</summary>
    public TemperatureRange MovingDay(int value)
        => new(Night, Math.Max(Snap(value), Night));

    /// <summary>Distance between the handles, in kelvin.</summary>
    public int Span => Day - Night;

    /// <summary><c>1200 K – 6500 K</c>, for the caption under the control.</summary>
    public string DisplayText => $"{Night} K \u2013 {Day} K";
}
