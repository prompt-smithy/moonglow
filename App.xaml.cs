using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using Amber.Infrastructure;
using Amber.Models;
using Amber.Services;
using Amber.ViewModels;
using Amber.Views;
using Hardcodet.Wpf.TaskbarNotification;
using Microsoft.Win32;

namespace Amber;

public partial class App : Application
{
    private static Mutex? _instanceMutex;
    private TaskbarIcon? _tray;
    private MonitorService _monitors = null!;
    private BrightnessService _brightness = null!;
    private ColorTemperatureService _color = null!;
    private ScheduleService _schedule = null!;
    private SettingsService _settingsService = null!;
    private ThemeService _theme = null!;
    private MainViewModel _viewModel = null!;
    private MainWindow? _settingsWindow;
    private bool _sessionLocked;
    private DateTime _lastResumeUtc = DateTime.MinValue;
    private int _resumeSequence;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // Only allow one instance per user session (RunAtStartup + a manual launch would otherwise duplicate).
        _instanceMutex = new Mutex(true, @"Local\Amber.SingleInstance", out bool createdNew);
        if (!createdNew)
        {
            Shutdown();
            return;
        }

        ShutdownMode = ShutdownMode.OnExplicitShutdown;

        _settingsService = new SettingsService();
        _settingsService.Load();

        _theme = new ThemeService(_settingsService);
        _theme.ThemeChanged += OnThemeChanged;
        _theme.ApplyForCurrentWindowsMode();

        _monitors = new MonitorService();

        _brightness = new BrightnessService(_monitors);
        _color = new ColorTemperatureService(_monitors);
        _schedule = new ScheduleService(_color, _settingsService);
        _viewModel = new MainViewModel(_brightness, _color, _schedule, _settingsService, _monitors, _theme);
        _viewModel.OpenSettingsRequested += (_, _) => OpenSettings();
        _viewModel.ExitRequested += (_, _) => Shutdown();

        _tray = new TaskbarIcon
        {
            Icon = IconFactory.CreateTrayIcon(),
            ToolTipText = "Amber",
            DataContext = _viewModel,
            TrayPopup = new TrayPopup { DataContext = _viewModel },
            ContextMenu = BuildContextMenu(),
        };

        _viewModel.ApplyInitialState();
        _schedule.Start();

        SystemEvents.DisplaySettingsChanged += OnDisplaySettingsChanged;
        SystemEvents.UserPreferenceChanged += OnUserPreferenceChanged;
        SystemEvents.SessionSwitch += OnSessionSwitch;
        SystemEvents.PowerModeChanged += OnPowerModeChanged;
    }

    private async void OnSessionSwitch(object sender, SessionSwitchEventArgs e)
    {
        if (e.Reason == SessionSwitchReason.SessionLock)
        {
            Dispatcher.Invoke(() =>
            {
                _sessionLocked = true;
                if (DateTime.UtcNow - _lastResumeUtc <= TimeSpan.FromSeconds(60))
                    _viewModel.ReapplyColorTemperature();
            });
            return;
        }

        if (e.Reason != SessionSwitchReason.SessionUnlock)
            return;

        await Dispatcher.InvokeAsync(() =>
        {
            _sessionLocked = false;
            _lastResumeUtc = DateTime.MinValue;
            Interlocked.Increment(ref _resumeSequence);
            _color.RestoreOriginalGammaRamps();
            _viewModel.ReapplyColorTemperature();
        });

        // Windows can reset display color settings after sign-in, so retry while the desktop settles.
        for (int attempt = 0; attempt < 5; attempt++)
        {
            await Task.Delay(1000);
            if (Dispatcher.HasShutdownStarted || Dispatcher.HasShutdownFinished)
                return;

            await Dispatcher.InvokeAsync(() =>
            {
                _viewModel.ReapplyColorTemperature();
            });
        }
    }

    private async void OnPowerModeChanged(object sender, PowerModeChangedEventArgs e)
    {
        // Resuming re-initializes the display hardware, which drops any applied ramp.
        if (e.Mode != PowerModes.Resume)
            return;

        int resumeSequence = Interlocked.Increment(ref _resumeSequence);
        Dispatcher.Invoke(() =>
        {
            _lastResumeUtc = DateTime.UtcNow;
            _viewModel.ReapplyColorTemperature();
        });

        // Windows may restore its display state after the resume notification; retry only while locked.
        for (int attempt = 0; attempt < 5; attempt++)
        {
            await Task.Delay(1000);
            if (Dispatcher.HasShutdownStarted || Dispatcher.HasShutdownFinished)
                return;

            await Dispatcher.InvokeAsync(() =>
            {
                if (resumeSequence == Volatile.Read(ref _resumeSequence) &&
                    _sessionLocked &&
                    DateTime.UtcNow - _lastResumeUtc <= TimeSpan.FromSeconds(10))
                    _viewModel.ReapplyColorTemperature();
            });
        }
    }

    private void OnUserPreferenceChanged(object sender, UserPreferenceChangedEventArgs e)
    {
        if (e.Category is UserPreferenceCategory.General or UserPreferenceCategory.Color)
            Dispatcher.Invoke(() => _theme.ApplyForCurrentWindowsMode());
    }

    private ContextMenu BuildContextMenu()
    {
        var menu = new ContextMenu();

        var settingsItem = new MenuItem { Header = "Settings\u2026" };
        settingsItem.Click += (_, _) => OpenSettings();

        var exitItem = new MenuItem { Header = "Exit" };
        exitItem.Click += (_, _) => Shutdown();

        menu.Items.Add(settingsItem);
        menu.Items.Add(new Separator());
        menu.Items.Add(exitItem);
        return menu;
    }

    private void OpenSettings()
    {
        // Collapse the tray popup when moving focus to the full settings window.
        if (_tray?.TrayPopupResolved is { } popup)
            popup.IsOpen = false;

        if (_settingsWindow is not null)
        {
            _settingsWindow.Activate();
            return;
        }

        var window = new MainWindow { DataContext = _viewModel };
        window.SourceInitialized += (_, _) => _theme.ApplyWindowTitleBar(window);
        window.Closed += (_, _) => _settingsWindow = null;
        _settingsWindow = window;
        window.Show();
        window.Activate();
    }

    private void OnThemeChanged(AppTheme theme)
    {
        if (_settingsWindow is not null)
            _theme.ApplyWindowTitleBar(_settingsWindow);
    }

    private void OnDisplaySettingsChanged(object? sender, EventArgs e)
    {
        _monitors.Refresh();
        _schedule.Tick();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        SystemEvents.DisplaySettingsChanged -= OnDisplaySettingsChanged;
        SystemEvents.UserPreferenceChanged -= OnUserPreferenceChanged;
        SystemEvents.SessionSwitch -= OnSessionSwitch;
        SystemEvents.PowerModeChanged -= OnPowerModeChanged;
        _schedule?.Stop();
        _color?.Reset();
        _color?.Dispose();
        _monitors?.Dispose();
        _tray?.Dispose();
        _instanceMutex?.Dispose();
        base.OnExit(e);
    }
}
