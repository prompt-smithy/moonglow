using System.Windows.Threading;
using Amber.Models;

namespace Amber.Services;

/// <summary>Drives the gradual color-temperature transition based on the time of day.</summary>
public sealed class ScheduleService
{
    // Color temperature changes in discrete 100 K steps during a transition.
    private const int StepKelvin = 100;

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

    public int CurrentTargetTemperature() => ComputeTemperature(DateTime.Now);

    /// <summary>Applies the temperature for the current time if automatic mode is enabled.</summary>
    public void Tick()
    {
        if (!_settings.Settings.AutoColorTemperature)
            return;

        int kelvin = ComputeTemperature(DateTime.Now);
        _color.Apply(kelvin);
        TemperatureApplied?.Invoke(kelvin);
    }

    private int ComputeTemperature(DateTime now)
    {
        var settings = _settings.Settings;
        TimeSpan t = now.TimeOfDay;
        int dayK = settings.DayColorTempK;
        int nightK = settings.NightColorTempK;

        TimeSpan morningStart = settings.MorningStartTime;
        TimeSpan morningEnd = settings.MorningEndTime;
        TimeSpan eveningStart = settings.EveningStartTime;
        TimeSpan eveningEnd = settings.EveningEndTime;

        // Daily cycle: night(warm) -> [morning cools up to day] -> day(cool) -> [evening warms down to night] -> night(warm)
        if (t < morningStart)
            return nightK;
        if (t < morningEnd)
            return Interpolate(t, morningStart, morningEnd, nightK, dayK);
        if (t < eveningStart)
            return dayK;
        if (t < eveningEnd)
            return Interpolate(t, eveningStart, eveningEnd, dayK, nightK);
        return nightK;
    }

    private static int Interpolate(TimeSpan t, TimeSpan start, TimeSpan end, int fromK, int toK)
    {
        double span = (end - start).TotalMinutes;
        if (span <= 0)
            return SnapToStep(toK);

        double progress = Math.Clamp((t - start).TotalMinutes / span, 0.0, 1.0);
        return SnapToStep(fromK + (toK - fromK) * progress);
    }

    // Round to the nearest 100 K so the transition advances in fixed increments.
    private static int SnapToStep(double kelvin)
        => (int)Math.Round(kelvin / StepKelvin) * StepKelvin;
}
