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
    private readonly StartupController _startup;
    private readonly Dispatcher _dispatcher = Dispatcher.CurrentDispatcher;

    private bool _applyingFromSchedule;
    private int _refreshInProgress;

    private int _brightnessValue;
    private int _colorTemp;
    private bool _auto;
    private string? _hotkeyMessage;
    private bool _restoreBrightnessHotkeyRegistered;

    public event EventHandler? OpenSettingsRequested;
    public event EventHandler? ExitRequested;
    public event EventHandler? HotkeysChanged;

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
        _startup = new StartupController(StartupManager.Create(), Settings, _settingsService.Save);
        _startup.PropertyChanged += (_, _) =>
        {
            OnPropertyChanged(nameof(RunAtStartup));
            OnPropertyChanged(nameof(CanChangeRunAtStartup));
            OnPropertyChanged(nameof(RunAtStartupStatus));
        };

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
        ApplyBrightnessSettings();

        ReapplyColorTemperature();
    }

    public void ReapplyColorTemperature()
    {
        if (_auto)
            _schedule.Tick();
        else if (HasColorSupport)
            _color.Apply(_colorTemp);
    }

    public bool AllActiveDisplaysSupportHardwareBrightness
        => _brightness.IsSupported && _brightness.AllActiveDisplaysSupportHardware;

    public bool HasHardwareBrightnessSupport
        => Settings.HardwareBrightnessEnabled && AllActiveDisplaysSupportHardwareBrightness;

    public bool HasSoftwareBrightnessSupport => _color.SupportsSoftwareBrightness;

    public bool HasBrightnessSupport => HasHardwareBrightnessSupport || HasSoftwareBrightnessSupport;

    public bool HasBrightnessRecoveryShortcut
        => Settings.RestoreBrightnessShortcut is not null && _restoreBrightnessHotkeyRegistered;

    public bool HardwareBrightnessControlEnabled
        => Settings.HardwareBrightnessEnabled && AllActiveDisplaysSupportHardwareBrightness;

    public bool SoftwareBrightnessControlEnabled
        => Settings.SoftwareBrightnessEnabled && HasSoftwareBrightnessSupport;

    public bool BrightnessControlEnabled
        => HardwareBrightnessControlEnabled || SoftwareBrightnessControlEnabled;

    public bool SplitBrightnessControls
    {
        get => Settings.SplitBrightnessControls;
        set
        {
            if (Settings.SplitBrightnessControls == value)
                return;
            if (value)
            {
                var split = BrightnessPlan.UnifiedToSplit(
                    Settings.ManualBrightness,
                    HardwareBrightnessControlEnabled,
                    SoftwareBrightnessControlEnabled,
                    HasBrightnessRecoveryShortcut
                );
                Settings.SplitBrightnessControls = true;
                Settings.ManualHardwareBrightness = split.Hardware;
                Settings.ManualSoftwareBrightness = split.Software;
            }
            else
            {
                int master = BrightnessPlan.SplitToUnified(
                    Settings.ManualHardwareBrightness,
                    Settings.ManualSoftwareBrightness,
                    HardwareBrightnessControlEnabled,
                    SoftwareBrightnessControlEnabled,
                    HasBrightnessRecoveryShortcut
                );
                Settings.SplitBrightnessControls = false;
                Settings.ManualBrightness = master;
                _brightnessValue = master;
            }
            _settingsService.Save();
            ApplyBrightnessSettings();
            NotifyBrightnessProperties();
        }
    }

    public bool HardwareBrightnessEnabled
    {
        get => Settings.HardwareBrightnessEnabled;
        set
        {
            if (Settings.HardwareBrightnessEnabled == value)
                return;
            if (!value)
                ReleaseHardwareIfOwned();
            Settings.HardwareBrightnessEnabled = value;
            _settingsService.Save();
            ApplyBrightnessSettings();
            NotifyBrightnessProperties();
        }
    }

    public bool SoftwareBrightnessEnabled
    {
        get => Settings.SoftwareBrightnessEnabled;
        set
        {
            if (Settings.SoftwareBrightnessEnabled == value)
                return;
            if (!value)
                _color.SetSoftwareBrightness(AppSettings.MaxBrightnessPercent);
            Settings.SoftwareBrightnessEnabled = value;
            _settingsService.Save();
            ApplyBrightnessSettings();
            NotifyBrightnessProperties();
        }
    }

    public int HardwareBrightness
    {
        get => AppSettings.ClampBrightness(Settings.ManualHardwareBrightness);
        set
        {
            if (!HardwareBrightnessControlEnabled)
                return;
            int normalized = AppSettings.ClampBrightness(value);
            if (Settings.ManualHardwareBrightness == normalized)
                return;
            Settings.ManualHardwareBrightness = normalized;
            _settingsService.ScheduleSave();
            OnPropertyChanged(nameof(HardwareBrightness));
            ApplyBrightnessSettings();
        }
    }

    public int SoftwareBrightness
    {
        get
        {
            if (!HasBrightnessRecoveryShortcut)
                return Math.Max(AppSettings.ClampBrightness(Settings.ManualSoftwareBrightness), AppSettings.SoftwareBrightnessFloor);
            return AppSettings.ClampBrightness(Settings.ManualSoftwareBrightness);
        }
        set
        {
            if (!SoftwareBrightnessControlEnabled)
                return;
            int normalized = AppSettings.ClampBrightness(value);
            if (Settings.ManualSoftwareBrightness == normalized)
                return;
            Settings.ManualSoftwareBrightness = normalized;
            _settingsService.ScheduleSave();
            OnPropertyChanged(nameof(SoftwareBrightness));
            ApplyBrightnessSettings();
        }
    }

    public string BrightnessDisplayText
    {
        get
        {
            if (Settings.SplitBrightnessControls)
            {
                if (HardwareBrightnessControlEnabled && SoftwareBrightnessControlEnabled)
                    return $"HW {HardwareBrightness}% / SW {SoftwareBrightness}%";
                if (HardwareBrightnessControlEnabled)
                    return $"{HardwareBrightness}%";
                if (SoftwareBrightnessControlEnabled)
                    return $"{SoftwareBrightness}%";
            }
            return $"{Brightness}%";
        }
    }

    public bool ShowBrightnessInfoButton
        => !HardwareBrightnessControlEnabled
            || (Settings.SoftwareBrightnessEnabled && HasSoftwareBrightnessSupport && !HasBrightnessRecoveryShortcut);

    public string BrightnessStatus
    {
        get
        {
            if (!Settings.HardwareBrightnessEnabled && !Settings.SoftwareBrightnessEnabled)
                return "Hardware and software brightness are disabled in Settings.";
            if (HardwareBrightnessControlEnabled)
            {
                if (SoftwareBrightnessControlEnabled)
                    return HasBrightnessRecoveryShortcut
                        ? "Hardware brightness is available. Software dimming provides extra-low levels."
                        : "Hardware brightness is available. Software dimming is limited to 50% until a restore shortcut is registered.";
                return "Hardware brightness is available. Software Digital Shade is disabled in Settings.";
            }
            if (Settings.HardwareBrightnessEnabled && !AllActiveDisplaysSupportHardwareBrightness)
                return SoftwareBrightnessControlEnabled
                    ? (HasBrightnessRecoveryShortcut
                        ? "At least one display lacks hardware brightness. Software dimming is applied across all displays."
                        : "At least one display lacks hardware brightness. Software dimming is limited to 50% until a restore shortcut is registered.")
                    : "All connected displays must support hardware brightness, or software brightness must be enabled.";
            if (!HasSoftwareBrightnessSupport)
                return "Hardware brightness is unavailable, and software brightness is not supported by this display backend.";
            if (!Settings.SoftwareBrightnessEnabled)
                return "Software Digital Shade is disabled in Settings.";
            return HasBrightnessRecoveryShortcut
                ? "Software brightness is available. The recovery shortcut restores it to 100%."
                : "Software dimming is limited to 50% until a restore shortcut is registered.";
        }
    }

    public bool HasColorSupport => _monitors.Snapshot().Count > 0;

    public bool HasNoDisplays => !_brightness.InternalPanelAvailable && _monitors.Snapshot().Count == 0;

    public int Brightness
    {
        get
        {
            if (!HardwareBrightnessControlEnabled && SoftwareBrightnessControlEnabled && !HasBrightnessRecoveryShortcut)
                return Math.Max(_brightnessValue, AppSettings.SoftwareBrightnessFloor);
            return _brightnessValue;
        }
        set
        {
            if (Settings.SplitBrightnessControls || !BrightnessControlEnabled)
                return;
            if (!SetProperty(ref _brightnessValue, AppSettings.ClampBrightness(value)))
                return;

            Settings.ManualBrightness = _brightnessValue;
            _settingsService.ScheduleSave();

            ApplyBrightnessSettings();
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
            _settingsService.ScheduleSave();

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
            if (!value)
                Settings.ManualColorTempK = _colorTemp;
            _settingsService.Save();
            OnPropertyChanged(nameof(ManualColorEnabled));

            if (value)
            {
                _schedule.Tick();
            }
            else if (HasColorSupport)
            {
                // Hold the displayed temperature, including after the app restarts.
                _color.Apply(_colorTemp);
            }
        }
    }

    public bool ManualColorEnabled => !_auto && HasColorSupport;

    public HotkeySlot? CapturingSlot { get; private set; }

    public string? HotkeyMessage
    {
        get => _hotkeyMessage;
        private set
        {
            SetProperty(ref _hotkeyMessage, value);
        }
    }

    public void SetHotkeyMessage(string message)
    {
        HotkeyMessage = message;
    }

    /// <summary>One shortcut step brighter or dimmer. Like the other platforms,
    /// the keys do nothing without brightness-capable hardware.</summary>
    public void NudgeBrightness(int direction)
    {
        if (!BrightnessControlEnabled)
            return;
        if (Settings.SplitBrightnessControls)
        {
            if (direction > 0)
            {
                if (SoftwareBrightnessControlEnabled && SoftwareBrightness < 100)
                    SoftwareBrightness += direction * AppSettings.HotkeyBrightnessStep;
                else if (HardwareBrightnessControlEnabled)
                    HardwareBrightness += direction * AppSettings.HotkeyBrightnessStep;
            }
            else
            {
                if (HardwareBrightnessControlEnabled && HardwareBrightness > 1)
                    HardwareBrightness += direction * AppSettings.HotkeyBrightnessStep;
                else if (SoftwareBrightnessControlEnabled)
                    SoftwareBrightness += direction * AppSettings.HotkeyBrightnessStep;
            }
        }
        else
        {
            Brightness += direction * AppSettings.HotkeyBrightnessStep;
        }
    }

    public void RestoreBrightnessToFull()
    {
        _brightnessValue = AppSettings.MaxBrightnessPercent;
        Settings.ManualBrightness = _brightnessValue;
        Settings.ManualHardwareBrightness = AppSettings.MaxBrightnessPercent;
        Settings.ManualSoftwareBrightness = AppSettings.MaxBrightnessPercent;
        _settingsService.ScheduleSave();
        NotifyBrightnessProperties();
        ApplyBrightnessSettings(allowSoftwareWithoutShortcut: true);
    }

    /// <summary>Updates the safety gate from the OS registration result.</summary>
    public void SetRestoreBrightnessHotkeyRegistered(bool registered)
    {
        _restoreBrightnessHotkeyRegistered = registered;
        OnPropertyChanged(nameof(HasBrightnessRecoveryShortcut));
        ApplyBrightnessSettings();
        NotifyBrightnessProperties();
    }

    private bool _hardwareOwned;

    private void ReleaseHardwareIfOwned()
    {
        if (!_hardwareOwned)
            return;
        if (_brightness.IsSupported)
            _brightness.SetBrightness(AppSettings.MaxBrightnessPercent);
        _hardwareOwned = false;
    }

    private void ApplyBrightnessSettings(bool allowSoftwareWithoutShortcut = false)
    {
        bool hardwareUsable = HardwareBrightnessControlEnabled;
        bool softwareUsable = SoftwareBrightnessControlEnabled;

        var route = BrightnessPlan.ComputeRouting(
            _brightnessValue,
            Settings.SplitBrightnessControls,
            Settings.ManualHardwareBrightness,
            Settings.ManualSoftwareBrightness,
            hardwareUsable,
            softwareUsable,
            HasBrightnessRecoveryShortcut || allowSoftwareWithoutShortcut
        );

        if (route.Hardware.HasValue)
        {
            _brightness.SetBrightness(route.Hardware.Value);
            _hardwareOwned = true;
        }
        else
        {
            ReleaseHardwareIfOwned();
        }

        _color.SetSoftwareBrightness(route.Software);
    }

    private void NotifyBrightnessProperties()
    {
        OnPropertyChanged(nameof(BrightnessDisplayText));
        OnPropertyChanged(nameof(HasHardwareBrightnessSupport));
        OnPropertyChanged(nameof(AllActiveDisplaysSupportHardwareBrightness));
        OnPropertyChanged(nameof(HasSoftwareBrightnessSupport));
        OnPropertyChanged(nameof(HasBrightnessSupport));
        OnPropertyChanged(nameof(HasBrightnessRecoveryShortcut));
        OnPropertyChanged(nameof(HardwareBrightnessEnabled));
        OnPropertyChanged(nameof(SoftwareBrightnessEnabled));
        OnPropertyChanged(nameof(SplitBrightnessControls));
        OnPropertyChanged(nameof(HardwareBrightnessControlEnabled));
        OnPropertyChanged(nameof(SoftwareBrightnessControlEnabled));
        OnPropertyChanged(nameof(HardwareBrightness));
        OnPropertyChanged(nameof(SoftwareBrightness));
        OnPropertyChanged(nameof(BrightnessControlEnabled));
        OnPropertyChanged(nameof(ShowBrightnessInfoButton));
        OnPropertyChanged(nameof(BrightnessStatus));
        OnPropertyChanged(nameof(Brightness));
    }

    /// <summary>One shortcut step cooler or warmer. Manual keys must not fight
    /// the schedule: in automatic mode they do nothing.</summary>
    public void NudgeTemperature(int direction)
    {
        if (_auto || !HasColorSupport)
            return;
        int target = AppSettings.ClampKelvin(_colorTemp + direction * AppSettings.HotkeyTemperatureStep);
        if (_colorTemp == target)
            return;
        // Assign directly: SetProperty would report this method's name, but bindings
        // listen for ColorTemperature.
        _colorTemp = target;
        OnPropertyChanged(nameof(ColorTemperature));
        Settings.ManualColorTempK = _colorTemp;
        _settingsService.Save();
        _color.Apply(_colorTemp);
    }

    public string IncreaseBrightnessHotkeyText => HotkeyText(HotkeySlot.IncreaseBrightness, Settings.IncreaseBrightnessShortcut);
    public string DecreaseBrightnessHotkeyText => HotkeyText(HotkeySlot.DecreaseBrightness, Settings.DecreaseBrightnessShortcut);
    public string IncreaseTemperatureHotkeyText => HotkeyText(HotkeySlot.IncreaseTemperature, Settings.IncreaseTemperatureShortcut);
    public string DecreaseTemperatureHotkeyText => HotkeyText(HotkeySlot.DecreaseTemperature, Settings.DecreaseTemperatureShortcut);
    public string RestoreBrightnessHotkeyText => HotkeyText(HotkeySlot.RestoreBrightness, Settings.RestoreBrightnessShortcut);

    public bool HasIncreaseBrightnessHotkey => Settings.IncreaseBrightnessShortcut is not null;
    public bool HasDecreaseBrightnessHotkey => Settings.DecreaseBrightnessShortcut is not null;
    public bool HasIncreaseTemperatureHotkey => Settings.IncreaseTemperatureShortcut is not null;
    public bool HasDecreaseTemperatureHotkey => Settings.DecreaseTemperatureShortcut is not null;
    public bool HasRestoreBrightnessHotkey => Settings.RestoreBrightnessShortcut is not null;

    public void BeginCapture(HotkeySlot slot)
    {
        CapturingSlot = slot;
        HotkeyMessage = null;
        NotifyHotkeys();
    }

    public void CancelCapture()
    {
        CapturingSlot = null;
        HotkeyMessage = null;
        NotifyHotkeys();
    }

    /// <summary>Assigns a captured gesture (canonical text) or disables with null.
    /// A combo already used elsewhere moves here, like the Linux settings.</summary>
    public bool SetHotkey(HotkeySlot slot, string? gesture)
    {
        string? canonical = null;
        if (gesture is not null)
        {
            if (!Hotkey.TryParse(gesture, out Hotkey? parsed) || parsed is null)
                return false;
            canonical = parsed.ToString();
        }

        if (canonical is not null)
            ClearHotkeyDuplicate(slot, canonical);
        SetSlotGesture(slot, canonical);
        _settingsService.Save();
        CapturingSlot = null;
        HotkeyMessage = null;
        NotifyHotkeys();
        HotkeysChanged?.Invoke(this, EventArgs.Empty);
        return true;
    }

    public void ResetHotkeys()
    {
        Settings.IncreaseBrightnessShortcut = AppSettings.DefaultIncreaseBrightnessShortcut;
        Settings.DecreaseBrightnessShortcut = AppSettings.DefaultDecreaseBrightnessShortcut;
        Settings.IncreaseTemperatureShortcut = AppSettings.DefaultIncreaseTemperatureShortcut;
        Settings.DecreaseTemperatureShortcut = AppSettings.DefaultDecreaseTemperatureShortcut;
        Settings.RestoreBrightnessShortcut = null;
        _settingsService.Save();
        CapturingSlot = null;
        HotkeyMessage = null;
        NotifyHotkeys();
        HotkeysChanged?.Invoke(this, EventArgs.Empty);
    }

    private string HotkeyText(HotkeySlot slot, string? gesture)
        => CapturingSlot == slot ? "Press keys..." : gesture ?? "Off";

    private void ClearHotkeyDuplicate(HotkeySlot keep, string canonical)
    {
        if (keep != HotkeySlot.IncreaseBrightness && Settings.IncreaseBrightnessShortcut == canonical)
            Settings.IncreaseBrightnessShortcut = null;
        if (keep != HotkeySlot.DecreaseBrightness && Settings.DecreaseBrightnessShortcut == canonical)
            Settings.DecreaseBrightnessShortcut = null;
        if (keep != HotkeySlot.IncreaseTemperature && Settings.IncreaseTemperatureShortcut == canonical)
            Settings.IncreaseTemperatureShortcut = null;
        if (keep != HotkeySlot.DecreaseTemperature && Settings.DecreaseTemperatureShortcut == canonical)
            Settings.DecreaseTemperatureShortcut = null;
        if (keep != HotkeySlot.RestoreBrightness && Settings.RestoreBrightnessShortcut == canonical)
            Settings.RestoreBrightnessShortcut = null;
    }

    private void SetSlotGesture(HotkeySlot slot, string? gesture)
    {
        switch (slot)
        {
            case HotkeySlot.IncreaseBrightness:
                Settings.IncreaseBrightnessShortcut = gesture;
                break;
            case HotkeySlot.DecreaseBrightness:
                Settings.DecreaseBrightnessShortcut = gesture;
                break;
            case HotkeySlot.IncreaseTemperature:
                Settings.IncreaseTemperatureShortcut = gesture;
                break;
            case HotkeySlot.DecreaseTemperature:
                Settings.DecreaseTemperatureShortcut = gesture;
                break;
            case HotkeySlot.RestoreBrightness:
                Settings.RestoreBrightnessShortcut = gesture;
                break;
        }
    }

    private void NotifyHotkeys()
    {
        OnPropertyChanged(nameof(IncreaseBrightnessHotkeyText));
        OnPropertyChanged(nameof(DecreaseBrightnessHotkeyText));
        OnPropertyChanged(nameof(IncreaseTemperatureHotkeyText));
        OnPropertyChanged(nameof(DecreaseTemperatureHotkeyText));
        OnPropertyChanged(nameof(HasIncreaseBrightnessHotkey));
        OnPropertyChanged(nameof(HasDecreaseBrightnessHotkey));
        OnPropertyChanged(nameof(HasIncreaseTemperatureHotkey));
        OnPropertyChanged(nameof(HasDecreaseTemperatureHotkey));
        OnPropertyChanged(nameof(RestoreBrightnessHotkeyText));
        OnPropertyChanged(nameof(HasRestoreBrightnessHotkey));
        NotifyBrightnessProperties();
        OnPropertyChanged(nameof(CapturingSlot));
        OnPropertyChanged(nameof(HotkeyMessage));
    }

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
        _settingsService.ScheduleSave();
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
        get => _startup.Enabled;
        set => _ = _startup.SetEnabledAsync(value);
    }

    public bool CanChangeRunAtStartup => _startup.CanChange;

    public string RunAtStartupStatus => _startup.Status;

    public Task ApplyStartupStateAsync() => _startup.InitializeAsync();

    public Task RefreshStartupStateAsync() => _startup.RefreshAsync();

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
        bool hadHardwareBrightnessSupport = HasHardwareBrightnessSupport;
        _brightness.RefreshSupport();
        _monitors.Refresh();
        PublishDisplays(hadHardwareBrightnessSupport);
    }

    /// <summary>Re-enumerates displays on a worker thread. DDC probing can block while a
    /// panel is waking, so it must not run on the UI thread.</summary>
    public void RefreshDisplaysAsync()
    {
        if (Interlocked.Exchange(ref _refreshInProgress, 1) == 1)
            return;

        bool hadHardwareBrightnessSupport = HasHardwareBrightnessSupport;
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
                _dispatcher.BeginInvoke(new Action(() => PublishDisplays(hadHardwareBrightnessSupport)));
            }
            catch (InvalidOperationException)
            {
                // The dispatcher shut down between the check and the post.
            }
        });
    }

    private void PublishDisplays(bool hadHardwareBrightnessSupport)
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

        ApplyBrightnessSettings();

        NotifyBrightnessProperties();
        OnPropertyChanged(nameof(HasColorSupport));
        OnPropertyChanged(nameof(HasNoDisplays));
        OnPropertyChanged(nameof(ManualColorEnabled));

        // A panel that just woke may have lost the ramp (and the composition fallback), so
        // reassert the current target whenever the display set is republished.
        ReapplyColorTemperature();
    }
}
