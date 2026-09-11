using System.Threading;
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

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // Only allow one instance per user session (RunAtStartup + a manual launch would otherwise duplicate).
        _instanceMutex = new Mutex(true, @"Local\DisplayController.SingleInstance", out bool createdNew);
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
            ToolTipText = "Display Controller",
            DataContext = _viewModel,
            TrayPopup = new TrayPopup { DataContext = _viewModel },
            ContextMenu = BuildContextMenu(),
        };

        _viewModel.ApplyInitialState();
        _schedule.Start();

        SystemEvents.DisplaySettingsChanged += OnDisplaySettingsChanged;
        SystemEvents.UserPreferenceChanged += OnUserPreferenceChanged;
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
        _schedule?.Stop();
        _color?.Reset();
        _color?.Dispose();
        _monitors?.Dispose();
        _tray?.Dispose();
        _instanceMutex?.Dispose();
        base.OnExit(e);
    }
}
