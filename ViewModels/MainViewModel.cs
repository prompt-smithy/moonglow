using System.Collections.ObjectModel;
using System.Globalization;
using DisplayController.Infrastructure;
using DisplayController.Models;
using DisplayController.Services;

namespace DisplayController.ViewModels;

public sealed class MainViewModel : ObservableObject
{
    private readonly BrightnessService _brightness;
    private readonly ColorTemperatureService _color;
    private readonly ScheduleService _schedule;
    private readonly SettingsService _settingsService;
    private readonly MonitorService _monitors;
    private readonly ThemeService _theme;

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
        MonitorService monitors,
        ThemeService theme)
    {
        _brightness = brightness;
        _color = color;
        _schedule = schedule;
        _settingsService = settingsService;
        _monitors = monitors;
        _theme = theme;

        _brightnessValue = Settings.ManualBrightness;
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
        if (!_auto)
            _color.Apply(_colorTemp);
    }

    public int Brightness
    {
        get => _brightnessValue;
        set
        {
            if (!SetProperty(ref _brightnessValue, Math.Clamp(value, 0, 100)))
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

    public bool Use24HourFormat
    {
        get => Settings.Use24HourFormat;
        set
        {
            if (Settings.Use24HourFormat == value)
                return;

            Settings.Use24HourFormat = value;
            _settingsService.Save();
            OnPropertyChanged();
            OnPropertyChanged(nameof(Use12HourFormat));
            OnPropertyChanged(nameof(MorningStart));
            OnPropertyChanged(nameof(MorningEnd));
            OnPropertyChanged(nameof(EveningStart));
            OnPropertyChanged(nameof(EveningEnd));
        }
    }

    public bool Use12HourFormat
    {
        get => !Settings.Use24HourFormat;
        set => Use24HourFormat = !value;
    }

    public string MorningStart
    {
        get => FormatTime(Settings.MorningStart);
        set
        {
            var normalized = NormalizeTime(value);
            if (normalized != null && Settings.MorningStart != normalized)
            {
                Settings.MorningStart = normalized;
                _settingsService.Save();
                RetickIfAuto();
            }

            OnPropertyChanged();
        }
    }

    public string MorningEnd
    {
        get => FormatTime(Settings.MorningEnd);
        set
        {
            var normalized = NormalizeTime(value);
            if (normalized != null && Settings.MorningEnd != normalized)
            {
                Settings.MorningEnd = normalized;
                _settingsService.Save();
                RetickIfAuto();
            }

            OnPropertyChanged();
        }
    }

    public string EveningStart
    {
        get => FormatTime(Settings.EveningStart);
        set
        {
            var normalized = NormalizeTime(value);
            if (normalized != null && Settings.EveningStart != normalized)
            {
                Settings.EveningStart = normalized;
                _settingsService.Save();
                RetickIfAuto();
            }

            OnPropertyChanged();
        }
    }

    public string EveningEnd
    {
        get => FormatTime(Settings.EveningEnd);
        set
        {
            var normalized = NormalizeTime(value);
            if (normalized != null && Settings.EveningEnd != normalized)
            {
                Settings.EveningEnd = normalized;
                _settingsService.Save();
                RetickIfAuto();
            }

            OnPropertyChanged();
        }
    }

    private string FormatTime(string stored)
    {
        if (!TimeSpan.TryParse(stored, CultureInfo.InvariantCulture, out var ts))
            return stored;

        DateTime dt = DateTime.Today.Add(ts);
        return dt.ToString(Settings.Use24HourFormat ? "HH:mm" : "h:mm tt", CultureInfo.InvariantCulture);
    }

    // Accepts either 24-hour (20:29) or 12-hour (8:29 PM) input and stores as 24-hour "HH:mm".
    private static string? NormalizeTime(string input)
    {
        input = (input ?? string.Empty).Trim();
        string[] formats = { "H:mm", "HH:mm", "h:mm tt", "hh:mm tt", "h:mmtt", "hh:mmtt" };
        if (DateTime.TryParseExact(input, formats, CultureInfo.InvariantCulture, DateTimeStyles.None, out DateTime dt))
            return dt.ToString("HH:mm", CultureInfo.InvariantCulture);
        if (DateTime.TryParse(input, CultureInfo.InvariantCulture, DateTimeStyles.NoCurrentDateDefault, out dt))
            return dt.ToString("HH:mm", CultureInfo.InvariantCulture);
        return null;
    }

    public IReadOnlyList<string> LightThemes => ThemeCatalog.LightNames;
    public IReadOnlyList<string> DarkThemes => ThemeCatalog.DarkNames;

    public string LightModeTheme
    {
        get => Settings.LightModeTheme;
        set
        {
            if (Settings.LightModeTheme == value)
                return;

            Settings.LightModeTheme = value;
            _settingsService.Save();
            OnPropertyChanged();
            _theme.ApplyForCurrentWindowsMode();
        }
    }

    public string DarkModeTheme
    {
        get => Settings.DarkModeTheme;
        set
        {
            if (Settings.DarkModeTheme == value)
                return;

            Settings.DarkModeTheme = value;
            _settingsService.Save();
            OnPropertyChanged();
            _theme.ApplyForCurrentWindowsMode();
        }
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
