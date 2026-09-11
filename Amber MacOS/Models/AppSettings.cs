using System.Text.Json.Serialization;

namespace Amber.MacOS.Models;

public sealed class AppSettings
{
    public int DayColorTempK { get; set; } = 6500;
    public int NightColorTempK { get; set; } = 1200;
    public string MorningStart { get; set; } = "06:00";
    public string MorningEnd { get; set; } = "07:00";
    public string EveningStart { get; set; } = "17:30";
    public string EveningEnd { get; set; } = "18:30";
    public bool AutoColorTemperature { get; set; } = true;
    public int ManualBrightness { get; set; } = 80;
    public int ManualColorTempK { get; set; } = 6500;
    public string LightModeTheme { get; set; } = "Light Amber";
    public string DarkModeTheme { get; set; } = "Dark Amber";

    [JsonIgnore]
    public TimeSpan MorningStartTime => ParseTime(MorningStart, 6, 0);

    [JsonIgnore]
    public TimeSpan MorningEndTime => ParseTime(MorningEnd, 7, 0);

    [JsonIgnore]
    public TimeSpan EveningStartTime => ParseTime(EveningStart, 17, 30);

    [JsonIgnore]
    public TimeSpan EveningEndTime => ParseTime(EveningEnd, 18, 30);

    private static TimeSpan ParseTime(string value, int hours, int minutes)
        => TimeSpan.TryParse(value, out var time) ? time : new TimeSpan(hours, minutes, 0);
}