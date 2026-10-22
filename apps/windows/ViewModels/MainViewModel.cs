using System.Collections.ObjectModel;
using Moonglow.Infrastructure;
using Moonglow.Models;
using Moonglow.Services;

namespace Moonglow.ViewModels;

public sealed class MainViewModel : ObservableObject
{
    private readonly BrightnessService _brightness;
    private readonly ColorTemperatureService _color;
    private readonly ScheduleService _schedule;
    private readonly SettingsService _settingsService;
    private readonly MonitorService _monitors;

    private bool _applyingFromSchedule;

    private int _brightnessValue;
    private int _colorTemp;
    private bool _auto;

    public event EventHandler? OpenSettingsRequested;
    public event EventHandler? ExitRequested;

    public ObservableCollection<string> Displays { get; } = new();

    public RelayCommand OpenSettingsCommand { get; }
    public RelayCommand ExitCommand { get; }
    public RelayCommand RefreshCommand { get; }

    public MainViewModel(
        BrightnessService brightness,
        ColorTemperatureService color,
        ScheduleService schedule,
        SettingsService settingsService,
        MonitorService monitors)
    {
        _brightness = brightness;
        _color = color;
        _schedule = schedule;
        _settingsService = settingsService;
        _monitors = monitors;

        _brightnessValue = MathUtil.Clamp(Settings.ManualBrightness, BrightnessService.MinBrightnessPercent, 100);
        _auto = Settings.AutoColorTemperature;
        _colorTemp = _auto ? _schedule.CurrentTargetTemperature() : Settings.ManualColorTempK;

        OpenSettingsCommand = new RelayCommand(_ => OpenSettingsRequested?.Invoke(this, EventArgs.Empty));
        ExitCommand = new RelayCommand(_ => ExitRequested?.Invoke(this, EventArgs.Empty));
        RefreshCommand = new RelayCommand(_ => RefreshDisplays());

        _schedule.TemperatureApplied += OnScheduleTemperatureApplied;

        RefreshDisplays();
    }

    private AppSettings Settings => _settingsService.Settings;

    /// <summary>Applies the persisted brightness and (manual) color temperature at launch.</summary>
    public void ApplyInitialState()
    {
        _brightness.SetBrightness(_brightnessValue);
        ReapplyColorTemperature();
    }

    public void ReapplyColorTemperature()
    {
        if (_auto)
            _schedule.Tick();
        else
            _color.Apply(_colorTemp);
    }

    public int Brightness
    {
        get => _brightnessValue;
        set
        {
            if (!SetProperty(ref _brightnessValue, MathUtil.Clamp(value, BrightnessService.MinBrightnessPercent, 100)))
                return;

            _brightness.SetBrightness(_brightnessValue);
            Settings.ManualBrightness = _brightnessValue;
            _settingsService.Save();
        }
    }

    public int ColorTemperature
    {
        get => _colorTemp;
        set
        {
            if (!SetProperty(ref _colorTemp, value))
                return;

            if (_applyingFromSchedule || _auto)
                return;

            _color.Apply(_colorTemp);
            Settings.ManualColorTempK = _colorTemp;
            _settingsService.Save();
        }
    }

    public bool AutoColorTemperature
    {
        get => _auto;
        set
        {
            if (!SetProperty(ref _auto, value))
                return;

            Settings.AutoColorTemperature = value;
            _settingsService.Save();
            OnPropertyChanged(nameof(ManualColorEnabled));

            if (value)
            {
                _schedule.Tick();
            }
            else
            {
                Settings.ManualColorTempK = _colorTemp;
                _color.Apply(_colorTemp);
            }
        }
    }

    public bool ManualColorEnabled => !_auto;

    public int DayColorTemp
    {
        get => Settings.DayColorTempK;
        set
        {
            if (Settings.DayColorTempK == value)
                return;

            Settings.DayColorTempK = value;
            _settingsService.Save();
            OnPropertyChanged();
            RetickIfAuto();
        }
    }

    public int NightColorTemp
    {
        get => Settings.NightColorTempK;
        set
        {
            if (Settings.NightColorTempK == value)
                return;

            Settings.NightColorTempK = value;
            _settingsService.Save();
            OnPropertyChanged();
            RetickIfAuto();
        }
    }

    public string MorningStart
    {
        get => Settings.MorningStart;
        set => SetTime(value, Settings.MorningStart, v => Settings.MorningStart = v, nameof(MorningStart));
    }

    public string MorningEnd
    {
        get => Settings.MorningEnd;
        set => SetTime(value, Settings.MorningEnd, v => Settings.MorningEnd = v, nameof(MorningEnd));
    }

    public string EveningStart
    {
        get => Settings.EveningStart;
        set => SetTime(value, Settings.EveningStart, v => Settings.EveningStart = v, nameof(EveningStart));
    }

    public string EveningEnd
    {
        get => Settings.EveningEnd;
        set => SetTime(value, Settings.EveningEnd, v => Settings.EveningEnd = v, nameof(EveningEnd));
    }

    // Times are stored and edited as canonical 24-hour "HH:mm"; the TimePicker handles display.
    private void SetTime(string value, string current, Action<string> assign, string propertyName)
    {
        if (string.IsNullOrWhiteSpace(value) || current == value)
            return;

        assign(value);
        _settingsService.Save();
        RetickIfAuto();
        OnPropertyChanged(propertyName);
    }

    public bool RunAtStartup
    {
        get => Settings.RunAtStartup;
        set
        {
            if (Settings.RunAtStartup == value)
                return;

            Settings.RunAtStartup = value;
            _settingsService.Save();
            OnPropertyChanged();
            StartupManager.Set(value);
        }
    }

    private void OnScheduleTemperatureApplied(int kelvin)
    {
        _applyingFromSchedule = true;
        ColorTemperature = kelvin;
        _applyingFromSchedule = false;
    }

    private void RetickIfAuto()
    {
        if (_auto)
            _schedule.Tick();
    }

    private void RefreshDisplays()
    {
        _monitors.Refresh();
        Displays.Clear();

        foreach (var monitor in _monitors.Monitors)
        {
            string caps = monitor.SupportsDdcBrightness
                ? "brightness via DDC/CI"
                : "no DDC/CI brightness";
            string star = monitor.IsPrimary ? "\u2605 " : string.Empty;
            Displays.Add($"{star}{monitor.Description} ({monitor.DeviceName}) \u2014 {caps}");
        }

        if (_brightness.InternalPanelAvailable)
            Displays.Add("Internal panel \u2014 brightness via WMI");

        if (Displays.Count == 0)
            Displays.Add("No displays detected.");
    }
}
