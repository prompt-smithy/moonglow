using System.Text.Json.Serialization;

namespace DisplayController.Models;

/// <summary>Persisted user configuration.</summary>
public sealed class AppSettings
{
    /// <summary>Daytime / coolest color temperature in Kelvin (the "max").</summary>
    public int DayColorTempK { get; set; } = 6500;

    /// <summary>Nighttime / warmest color temperature in Kelvin (the "min"). Lower = more yellow.</summary>
    public int NightColorTempK { get; set; } = 1200;

    /// <summary>Morning transition start, "HH:mm". Screen cools (K increases) from night to day.</summary>
    public string MorningStart { get; set; } = "06:00";

    /// <summary>Morning transition end, "HH:mm".</summary>
    public string MorningEnd { get; set; } = "07:00";

    /// <summary>Evening transition start, "HH:mm". Screen warms (K decreases) from day to night.</summary>
    public string EveningStart { get; set; } = "17:30";

    /// <summary>Evening transition end, "HH:mm".</summary>
    public string EveningEnd { get; set; } = "18:30";

    public bool AutoColorTemperature { get; set; } = true;
    public int ManualBrightness { get; set; } = 80;
    public int ManualColorTempK { get; set; } = 6500;
    public bool RunAtStartup { get; set; }
    public bool Use24HourFormat { get; set; } = true;
    public string LightModeTheme { get; set; } = "White";
    public string DarkModeTheme { get; set; } = "Dark";

    [JsonIgnore]
    public TimeSpan MorningStartTime => ParseTime(MorningStart, 6, 0);

    [JsonIgnore]
    public TimeSpan MorningEndTime => ParseTime(MorningEnd, 7, 0);

    [JsonIgnore]
    public TimeSpan EveningStartTime => ParseTime(EveningStart, 17, 30);

    [JsonIgnore]
    public TimeSpan EveningEndTime => ParseTime(EveningEnd, 18, 30);

    private static TimeSpan ParseTime(string value, int hours, int minutes)
        => TimeSpan.TryParse(value, out var t) ? t : new TimeSpan(hours, minutes, 0);
}
