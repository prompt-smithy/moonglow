using System.Collections.ObjectModel;
using Avalonia.Threading;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Amber.MacOS.Models;
using Amber.MacOS.Services;

namespace Amber.MacOS.ViewModels;

public sealed class MainViewModel : ObservableObject
{
    private readonly SettingsService _settingsService;
    private readonly IDisplayService _displayService;
    private readonly IBrightnessService _brightnessService;
    private readonly IColorTemperatureService _colorService;
    private readonly ScheduleService _schedule;
    private readonly ThemeService _theme;
    private int _brightness;
    private int _colorTemperature;
    private bool _autoColorTemperature;

    public MainViewModel(
        SettingsService settingsService,
        IDisplayService displayService,
        IBrightnessService brightnessService,
        IColorTemperatureService colorService,
        ScheduleService schedule,
        ThemeService theme)
    {
        _settingsService = settingsService;
        _displayService = displayService;
        _brightnessService = brightnessService;
        _colorService = colorService;
        _schedule = schedule;
        _theme = theme;

        _brightness = Math.Clamp(Settings.ManualBrightness, 1, 100);
        _autoColorTemperature = Settings.AutoColorTemperature;
        _colorTemperature = _autoColorTemperature
            ? ScheduleService.ComputeTemperature(Settings, DateTime.Now)
            : Settings.ManualColorTempK;

        ShowWindowCommand = new RelayCommand(() => ShowWindowRequested?.Invoke(this, EventArgs.Empty));
        ShowQuickControlsCommand = new RelayCommand(() => ShowQuickControlsRequested?.Invoke(this, EventArgs.Empty));
        QuitCommand = new RelayCommand(() => QuitRequested?.Invoke(this, EventArgs.Empty));
        RefreshCommand = new RelayCommand(RefreshDisplays);
        _schedule.TemperatureApplied += OnTemperatureApplied;
        RefreshDisplays();
    }

    public event EventHandler? ShowWindowRequested;
    public event EventHandler? ShowQuickControlsRequested;
    public event EventHandler? QuitRequested;

    public ObservableCollection<DisplayInfo> Displays { get; } = new();
    public IRelayCommand ShowWindowCommand { get; }
    public IRelayCommand ShowQuickControlsCommand { get; }
    public IRelayCommand QuitCommand { get; }
    public IRelayCommand RefreshCommand { get; }
    public IReadOnlyList<string> LightThemes => ThemeCatalog.LightNames;
    public IReadOnlyList<string> DarkThemes => ThemeCatalog.DarkNames;

    private AppSettings Settings => _settingsService.Settings;

    public bool HasNoDisplays => Displays.Count == 0;
    public bool HasBrightnessSupport => _brightnessService.IsSupported && Displays.Any(display => display.SupportsBrightness);
    public bool HasColorSupport => _colorService.IsSupported && Displays.Any(display => display.SupportsColorTemperature);
    public string BrightnessStatus => HasBrightnessSupport
        ? "Hardware brightness is available through DDC/CI."
        : "Brightness control is unavailable for the detected display connection.";
    public string ColorStatus => HasColorSupport
        ? "CoreGraphics gamma control is available."
        : "Color-temperature control is unavailable on this system.";

    public int Brightness
    {
        get => _brightness;
        set
        {
            value = Math.Clamp(value, 1, 100);
            if (!SetProperty(ref _brightness, value))
                return;

            Settings.ManualBrightness = value;
            _settingsService.Save();
            _brightnessService.SetBrightness(value);
        }
    }

    public int ColorTemperature
    {
        get => _colorTemperature;
        set
        {
            value = Math.Clamp(value, 900, 6600);
            if (!SetProperty(ref _colorTemperature, value) || _autoColorTemperature)
                return;

            Settings.ManualColorTempK = value;
            _settingsService.Save();
            _colorService.Apply(value);
        }
    }

    public bool AutoColorTemperature
    {
        get => _autoColorTemperature;
        set
        {
            if (!SetProperty(ref _autoColorTemperature, value))
                return;

            Settings.AutoColorTemperature = value;
            _settingsService.Save();
            OnPropertyChanged(nameof(ManualColorEnabled));
            if (value)
                _schedule.Tick();
            else
                _colorService.Apply(_colorTemperature);
        }
    }

    public bool ManualColorEnabled => !_autoColorTemperature && HasColorSupport;

    public string LightModeTheme
    {
        get => Settings.LightModeTheme;
        set
        {
            if (!ThemeCatalog.LightNames.Contains(value) || Settings.LightModeTheme == value)
                return;
            Settings.LightModeTheme = value;
            _settingsService.Save();
            OnPropertyChanged();
            _theme.ApplyForCurrentMode();
        }
    }

    public string DarkModeTheme
    {
        get => Settings.DarkModeTheme;
        set
        {
            if (!ThemeCatalog.DarkNames.Contains(value) || Settings.DarkModeTheme == value)
                return;
            Settings.DarkModeTheme = value;
            _settingsService.Save();
            OnPropertyChanged();
            _theme.ApplyForCurrentMode();
        }
    }

    public int DayColorTemp
    {
        get => Settings.DayColorTempK;
        set => SetTemperatureTarget(value, true);
    }

    public int NightColorTemp
    {
        get => Settings.NightColorTempK;
        set => SetTemperatureTarget(value, false);
    }

    public string MorningStart
    {
        get => Settings.MorningStart;
        set => SetTime(value, Settings.MorningStart, newValue => Settings.MorningStart = newValue);
    }

    public string MorningEnd
    {
        get => Settings.MorningEnd;
        set => SetTime(value, Settings.MorningEnd, newValue => Settings.MorningEnd = newValue);
    }

    public string EveningStart
    {
        get => Settings.EveningStart;
        set => SetTime(value, Settings.EveningStart, newValue => Settings.EveningStart = newValue);
    }

    public string EveningEnd
    {
        get => Settings.EveningEnd;
        set => SetTime(value, Settings.EveningEnd, newValue => Settings.EveningEnd = newValue);
    }

    public void ApplyInitialState()
    {
        if (HasBrightnessSupport)
            _brightnessService.SetBrightness(_brightness);
        if (!_autoColorTemperature && HasColorSupport)
            _colorService.Apply(_colorTemperature);
    }

    private void RefreshDisplays()
    {
        var refreshed = _displayService.Refresh();
        Displays.Clear();
        foreach (var display in refreshed)
            Displays.Add(display);

        OnPropertyChanged(nameof(HasNoDisplays));
        OnPropertyChanged(nameof(HasBrightnessSupport));
        OnPropertyChanged(nameof(HasColorSupport));
        OnPropertyChanged(nameof(ManualColorEnabled));
        OnPropertyChanged(nameof(BrightnessStatus));
        OnPropertyChanged(nameof(ColorStatus));
    }

    private void SetTemperatureTarget(int value, bool isDay)
    {
        value = Math.Clamp(value, 900, 6600);
        if (isDay)
        {
            if (Settings.DayColorTempK == value)
                return;
            Settings.DayColorTempK = value;
            OnPropertyChanged(nameof(DayColorTemp));
        }
        else
        {
            if (Settings.NightColorTempK == value)
                return;
            Settings.NightColorTempK = value;
            OnPropertyChanged(nameof(NightColorTemp));
        }

        _settingsService.Save();
        if (_autoColorTemperature)
            _schedule.Tick();
    }

    private void SetTime(string value, string current, Action<string> assign)
    {
        if (value == current || !TimeSpan.TryParse(value, out _))
            return;

        assign(value);
        _settingsService.Save();
        if (_autoColorTemperature)
            _schedule.Tick();
    }

    private void OnTemperatureApplied(int kelvin)
    {
        Dispatcher.UIThread.Post(() => SetProperty(ref _colorTemperature, kelvin, nameof(ColorTemperature)));
    }
}
