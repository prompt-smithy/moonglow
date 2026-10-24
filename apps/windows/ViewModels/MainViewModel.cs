using System.Collections.ObjectModel;
using System.Threading.Tasks;
using System.Windows.Threading;
using Moonglow.Core;
using Moonglow.Infrastructure;
using Moonglow.Services;

namespace Moonglow.ViewModels;

public sealed class MainViewModel : ObservableObject
{
    private readonly BrightnessService _brightness;
    private readonly ColorTemperatureService _color;
    private readonly ScheduleService _schedule;
    private readonly SettingsService _settingsService;
    private readonly MonitorService _monitors;
    private readonly Dispatcher _dispatcher = Dispatcher.CurrentDispatcher;

    private bool _applyingFromSchedule;
    private bool _startupRegistrationFailed;
    private int _refreshInProgress;

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

        _brightnessValue = AppSettings.ClampBrightness(Settings.ManualBrightness);
        _auto = Settings.AutoColorTemperature;
        _colorTemp = _auto ? _schedule.CurrentTargetTemperature() : AppSettings.ClampKelvin(Settings.ManualColorTempK);

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
        if (HasBrightnessSupport)
            _brightness.SetBrightness(_brightnessValue);

        ReapplyColorTemperature();
    }

    public void ReapplyColorTemperature()
    {
        if (_auto)
            _schedule.Tick();
        else if (HasColorSupport)
            _color.Apply(_colorTemp);
    }

    public bool HasBrightnessSupport => _brightness.IsSupported;

    public bool HasColorSupport => _monitors.Snapshot().Count > 0;

    public bool HasNoDisplays => !_brightness.InternalPanelAvailable && _monitors.Snapshot().Count == 0;

    public int Brightness
    {
        get => _brightnessValue;
        set
        {
            if (!SetProperty(ref _brightnessValue, AppSettings.ClampBrightness(value)))
                return;

            Settings.ManualBrightness = _brightnessValue;
            _settingsService.Save();

            // The setting persists regardless; hardware only moves with a capable display.
            if (HasBrightnessSupport)
                _brightness.SetBrightness(_brightnessValue);
        }
    }

    public int ColorTemperature
    {
        get => _colorTemp;
        set
        {
            // The shared contract stores and applies colour temperature in 100 K steps.
            if (!SetProperty(ref _colorTemp, TemperatureRange.Snap(value)))
                return;

            // In automatic mode the slider shows the schedule's value, which is never
            // persisted as a manual override or applied over the schedule.
            if (_applyingFromSchedule || _auto)
                return;

            Settings.ManualColorTempK = _colorTemp;
            _settingsService.Save();

            if (HasColorSupport)
                _color.Apply(_colorTemp);
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
            else if (HasColorSupport)
            {
                // Keep showing the temperature that was on screen. The manual preference is
                // left untouched, so turning automatic mode back on does not forget it.
                _color.Apply(_colorTemp);
            }
        }
    }

    public bool ManualColorEnabled => !_auto && HasColorSupport;

    public int DayColorTemp
    {
        get => Settings.DayColorTempK;
        set => SetTemperatureRange(TemperatureRange.FromSettings(Settings).MovingDay(value));
    }

    public int NightColorTemp
    {
        get => Settings.NightColorTempK;
        set => SetTemperatureRange(TemperatureRange.FromSettings(Settings).MovingNight(value));
    }

    /// <summary>Stores an ordered, snapped range. Both handles are refreshed because moving
    /// one can pin it against the other.</summary>
    private void SetTemperatureRange(TemperatureRange range)
    {
        if (Settings.NightColorTempK == range.Night && Settings.DayColorTempK == range.Day)
            return;

        Settings.NightColorTempK = range.Night;
        Settings.DayColorTempK = range.Day;
        _settingsService.Save();
        OnPropertyChanged(nameof(NightColorTemp));
        OnPropertyChanged(nameof(DayColorTemp));
        RetickIfAuto();
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
        string clamped = ClampScheduleTime(value, propertyName, current);
        if (current == clamped)
            return;

        assign(clamped);
        _settingsService.Save();
        RetickIfAuto();
        OnPropertyChanged(propertyName);
    }

    /// <summary>
    /// Keeps <c>morningStart &lt; morningEnd &lt;= eveningStart &lt; eveningEnd</c> by clamping
    /// an edit against its neighbours, leaving the other three untouched. Without this an
    /// inverted window collapses the day to night plus one abrupt flash at the boundary.
    /// </summary>
    private string ClampScheduleTime(string value, string propertyName, string current)
    {
        if (!AppSettings.TryParseTime(value, out TimeSpan time))
            return current;

        TimeSpan oneMinute = TimeSpan.FromMinutes(1);
        var settings = Settings;
        TimeSpan lower;
        TimeSpan upper;

        if (propertyName == nameof(MorningStart))
        {
            lower = TimeSpan.Zero;
            upper = settings.MorningEndTime - oneMinute;
        }
        else if (propertyName == nameof(MorningEnd))
        {
            lower = settings.MorningStartTime + oneMinute;
            upper = settings.EveningStartTime;
        }
        else if (propertyName == nameof(EveningStart))
        {
            lower = settings.MorningEndTime;
            upper = settings.EveningEndTime - oneMinute;
        }
        else if (propertyName == nameof(EveningEnd))
        {
            lower = settings.EveningStartTime + oneMinute;
            upper = TimeSpan.FromDays(1) - oneMinute;
        }
        else
        {
            return value;
        }

        // No valid value exists (the neighbours already touch): refuse the edit rather
        // than invent a time.
        if (lower > upper)
            return current;

        if (time < lower)
            time = lower;
        else if (time > upper)
            time = upper;

        return AppSettings.FormatTime(time);
    }

    public bool RunAtStartup
    {
        get => Settings.LaunchAtLogin;
        set
        {
            if (Settings.LaunchAtLogin == value)
                return;

            // A change Windows refuses is deliberately not recorded, so the toggle falls back
            // to the truth instead of claiming a registration that was never made.
            if (!StartupManager.Set(value))
            {
                _startupRegistrationFailed = true;
                OnPropertyChanged();
                OnPropertyChanged(nameof(RunAtStartupStatus));
                return;
            }

            _startupRegistrationFailed = false;
            Settings.LaunchAtLogin = value;
            _settingsService.Save();
            OnPropertyChanged();
            OnPropertyChanged(nameof(RunAtStartupStatus));
        }
    }

    /// <summary>Describes the startup state for the settings window.</summary>
    public string RunAtStartupStatus
    {
        get
        {
            if (_startupRegistrationFailed)
                return "Windows did not accept this startup entry. Check the registry permissions, then try again.";
            if (StartupManager.IsEnabled())
                return "Moonglow opens automatically when you sign in.";
            if (Settings.LaunchAtLogin)
                return "Could not register Moonglow as a startup app on this system.";
            return "Moonglow opens only when you launch it.";
        }
    }

    /// <summary>
    /// Re-asserts the stored preference on Windows at launch, and adopts an entry the app did
    /// not create instead of switching it off behind the user's back.
    /// </summary>
    public void ApplyStartupState()
    {
        if (Settings.LaunchAtLogin)
        {
            _startupRegistrationFailed = !StartupManager.Set(true);
        }
        else if (StartupManager.IsEnabled())
        {
            Settings.LaunchAtLogin = true;
            _settingsService.Save();
        }

        OnPropertyChanged(nameof(RunAtStartup));
        OnPropertyChanged(nameof(RunAtStartupStatus));
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

    /// <summary>Re-enumerates displays and re-probes hardware support.</summary>
    public void RefreshDisplays()
    {
        bool hadBrightnessSupport = HasBrightnessSupport;
        _brightness.RefreshSupport();
        _monitors.Refresh();
        PublishDisplays(hadBrightnessSupport);
    }

    /// <summary>Re-enumerates displays on a worker thread. DDC probing can block while a
    /// panel is waking, so it must not run on the UI thread.</summary>
    public void RefreshDisplaysAsync()
    {
        if (Interlocked.Exchange(ref _refreshInProgress, 1) == 1)
            return;

        bool hadBrightnessSupport = HasBrightnessSupport;
        Task.Run(() =>
        {
            try
            {
                _brightness.RefreshSupport();
                _monitors.Refresh();
            }
            catch
            {
                // Probing is best effort; the next attempt can retry.
            }
            finally
            {
                Interlocked.Exchange(ref _refreshInProgress, 0);
            }

            if (_dispatcher.HasShutdownStarted || _dispatcher.HasShutdownFinished)
                return;

            try
            {
                _dispatcher.BeginInvoke(new Action(() => PublishDisplays(hadBrightnessSupport)));
            }
            catch (InvalidOperationException)
            {
                // The dispatcher shut down between the check and the post.
            }
        });
    }

    private void PublishDisplays(bool hadBrightnessSupport)
    {
        Displays.Clear();

        foreach (var monitor in _monitors.Snapshot())
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

        // Hardware that answered late can take the persisted values now.
        if (!hadBrightnessSupport && HasBrightnessSupport)
            _brightness.SetBrightness(_brightnessValue);

        OnPropertyChanged(nameof(HasBrightnessSupport));
        OnPropertyChanged(nameof(HasColorSupport));
        OnPropertyChanged(nameof(HasNoDisplays));
        OnPropertyChanged(nameof(ManualColorEnabled));

        // A panel that just woke may have lost the ramp (and the composition fallback), so
        // reassert the current target whenever the display set is republished.
        ReapplyColorTemperature();
    }
}
