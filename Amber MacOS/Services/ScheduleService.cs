using Amber.MacOS.Models;

namespace Amber.MacOS.Services;

public sealed class ScheduleService : IDisposable
{
    private const int StepKelvin = 100;
    private readonly IColorTemperatureService _color;
    private readonly SettingsService _settings;
    private Timer? _timer;

    public ScheduleService(IColorTemperatureService color, SettingsService settings)
    {
        _color = color;
        _settings = settings;
    }

    public event Action<int>? TemperatureApplied;

    public void Start()
    {
        _timer ??= new Timer(_ => Tick(), null, TimeSpan.Zero, TimeSpan.FromSeconds(10));
    }

    public void Tick()
    {
        if (!_settings.Settings.AutoColorTemperature || !_color.IsSupported)
            return;

        int kelvin = ComputeTemperature(_settings.Settings, DateTime.Now);
        if (_color.Apply(kelvin))
            TemperatureApplied?.Invoke(kelvin);
    }

    public static int ComputeTemperature(AppSettings settings, DateTime now)
    {
        TimeSpan time = now.TimeOfDay;
        if (time < settings.MorningStartTime)
            return settings.NightColorTempK;
        if (time < settings.MorningEndTime)
            return Interpolate(time, settings.MorningStartTime, settings.MorningEndTime,
                settings.NightColorTempK, settings.DayColorTempK);
        if (time < settings.EveningStartTime)
            return settings.DayColorTempK;
        if (time < settings.EveningEndTime)
            return Interpolate(time, settings.EveningStartTime, settings.EveningEndTime,
                settings.DayColorTempK, settings.NightColorTempK);
        return settings.NightColorTempK;
    }

    private static int Interpolate(TimeSpan time, TimeSpan start, TimeSpan end, int fromKelvin, int toKelvin)
    {
        double span = (end - start).TotalMinutes;
        if (span <= 0)
            return SnapToStep(toKelvin);

        double progress = Math.Clamp((time - start).TotalMinutes / span, 0.0, 1.0);
        return SnapToStep(fromKelvin + ((toKelvin - fromKelvin) * progress));
    }

    private static int SnapToStep(double kelvin)
        => (int)Math.Round(kelvin / StepKelvin) * StepKelvin;

    public void Dispose()
    {
        _timer?.Dispose();
        _timer = null;
    }
}