using System.Windows.Threading;
using Moonglow.Core;

namespace Moonglow.Services;

/// <summary>Drives the gradual color-temperature transition based on the time of day.</summary>
public sealed class ScheduleService
{
    private readonly DispatcherTimer _timer;
    private readonly ColorTemperatureService _color;
    private readonly SettingsService _settings;

    /// <summary>Raised whenever the scheduler applies a temperature (Kelvin).</summary>
    public event Action<int>? TemperatureApplied;

    public ScheduleService(ColorTemperatureService color, SettingsService settings)
    {
        _color = color;
        _settings = settings;
        _timer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(10) };
        _timer.Tick += (_, _) => Tick();
    }

    public void Start()
    {
        _timer.Start();
        Tick();
    }

    public void Stop() => _timer.Stop();

    public int CurrentTargetTemperature() => ScheduleCalculator.TemperatureNow(_settings.Settings);

    /// <summary>Applies the temperature for the current time if automatic mode is enabled.</summary>
    public void Tick()
    {
        if (!_settings.Settings.AutoColorTemperature)
            return;

        int kelvin = ScheduleCalculator.TemperatureNow(_settings.Settings);
        _color.Apply(kelvin);
        TemperatureApplied?.Invoke(kelvin);
    }
}
