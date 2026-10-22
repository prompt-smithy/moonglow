namespace Moonglow.Models;

public sealed class AppSettings
{
    public int ManualBrightness { get; set; } = 60;
    public int ManualColorTempK { get; set; } = 5000;
    public bool AutoColorTemperature { get; set; } = true;
    public int DayColorTempK { get; set; } = 6500;
    public int NightColorTempK { get; set; } = 3500;

    public string MorningStart { get; set; } = "07:00";
    public string MorningEnd { get; set; } = "09:00";
    public string EveningStart { get; set; } = "18:00";
    public string EveningEnd { get; set; } = "22:00";

    public bool RunAtStartup { get; set; } = false;

    public TimeSpan MorningStartTime => TimeSpan.Parse(MorningStart);
    public TimeSpan MorningEndTime => TimeSpan.Parse(MorningEnd);
    public TimeSpan EveningStartTime => TimeSpan.Parse(EveningStart);
    public TimeSpan EveningEndTime => TimeSpan.Parse(EveningEnd);
}

