using System.Globalization;

namespace Moonglow.Core;

/// <summary>Persisted application settings.</summary>
/// <remarks>
/// Ranges, defaults, and normalization mirror the macOS and Linux builds; see
/// <c>spec/settings.schema.json</c> for the canonical shape every platform shares.
/// </remarks>
public sealed class AppSettings
{
    public const int MinBrightnessPercent = 1;
    public const int MaxBrightnessPercent = 100;
    public const int MinKelvin = 900;
    public const int MaxKelvin = 6600;
    public const int KelvinStep = 100;

    public const string DefaultMorningStart = "06:00";
    public const string DefaultMorningEnd = "07:00";
    public const string DefaultEveningStart = "17:30";
    public const string DefaultEveningEnd = "18:30";

    private static readonly TimeSpan OneDay = TimeSpan.FromDays(1);

    public int ManualBrightness { get; set; } = 80;
    public int ManualColorTempK { get; set; } = 6500;
    public bool AutoColorTemperature { get; set; } = true;
    public int DayColorTempK { get; set; } = 6500;
    public int NightColorTempK { get; set; } = 1200;

    public string MorningStart { get; set; } = DefaultMorningStart;
    public string MorningEnd { get; set; } = DefaultMorningEnd;
    public string EveningStart { get; set; } = DefaultEveningStart;
    public string EveningEnd { get; set; } = DefaultEveningEnd;

    public bool LaunchAtLogin { get; set; }

    // Parsed forms never throw: a hand-edited or truncated file falls back to the default
    // wall-clock time instead of crashing the scheduler on its next tick.
    public TimeSpan MorningStartTime => ParseOrDefault(MorningStart, DefaultMorningStart);
    public TimeSpan MorningEndTime => ParseOrDefault(MorningEnd, DefaultMorningEnd);
    public TimeSpan EveningStartTime => ParseOrDefault(EveningStart, DefaultEveningStart);
    public TimeSpan EveningEndTime => ParseOrDefault(EveningEnd, DefaultEveningEnd);

    public static int ClampBrightness(int value)
        => MathUtil.Clamp(value, MinBrightnessPercent, MaxBrightnessPercent);

    public static int ClampKelvin(int value)
        => MathUtil.Clamp(value, MinKelvin, MaxKelvin);

    /// <summary>Parses a zero-padded 24-hour <c>HH:mm</c> wall-clock time.</summary>
    public static bool TryParseTime(string? value, out TimeSpan time)
    {
        time = default;
        if (string.IsNullOrWhiteSpace(value))
            return false;

        string trimmed = value!.Trim();
        if (!TimeSpan.TryParseExact(trimmed, @"hh\:mm", CultureInfo.InvariantCulture, out time)
            && !TimeSpan.TryParse(trimmed, CultureInfo.InvariantCulture, out time))
        {
            return false;
        }

        return time >= TimeSpan.Zero && time < OneDay;
    }

    /// <summary>Canonical <c>HH:mm</c> form used on disk and by the pickers.</summary>
    public static string FormatTime(TimeSpan time)
        => $"{(int)time.TotalHours:00}:{time.Minutes:00}";

    /// <summary>The time in canonical form, or <paramref name="fallback"/> when unparseable.</summary>
    public static string NormalizeTime(string? value, string fallback)
        => TryParseTime(value, out TimeSpan time) ? FormatTime(time) : fallback;

    /// <summary>Returns a copy with every value inside its supported range and every
    /// schedule time well formed.</summary>
    public AppSettings Normalized()
    {
        var copy = (AppSettings)MemberwiseClone();

        copy.ManualBrightness = ClampBrightness(copy.ManualBrightness);
        copy.ManualColorTempK = ClampKelvin(copy.ManualColorTempK);
        copy.DayColorTempK = ClampKelvin(copy.DayColorTempK);
        copy.NightColorTempK = ClampKelvin(copy.NightColorTempK);

        copy.MorningStart = NormalizeTime(copy.MorningStart, DefaultMorningStart);
        copy.MorningEnd = NormalizeTime(copy.MorningEnd, DefaultMorningEnd);
        copy.EveningStart = NormalizeTime(copy.EveningStart, DefaultEveningStart);
        copy.EveningEnd = NormalizeTime(copy.EveningEnd, DefaultEveningEnd);

        return copy;
    }

    private static TimeSpan ParseOrDefault(string? value, string fallback)
        => TryParseTime(value, out TimeSpan time)
            ? time
            : TimeSpan.Parse(fallback, CultureInfo.InvariantCulture);
}
