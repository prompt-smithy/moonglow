using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using Avalonia.Threading;
using Amber.MacOS.Services;
using Amber.MacOS.ViewModels;
using Amber.MacOS.Views;

namespace Amber.MacOS;

public partial class App : Application
{
    private MainWindow? _mainWindow;
    private QuickControlsWindow? _quickControlsWindow;
    private ScheduleService? _schedule;
    private IDisposable? _brightness;
    private IColorTemperatureService? _colorTemperature;

    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            desktop.ShutdownMode = ShutdownMode.OnExplicitShutdown;

            var settings = new SettingsService();
            settings.Load();
            var theme = new ThemeService(this, settings);
            theme.ApplyForCurrentMode();

            IDisplayService displays;
            IBrightnessService brightness;
            if (OperatingSystem.IsMacOS())
            {
                var ddcBrightness = new MacDdcBrightnessService();
                _brightness = ddcBrightness;
                brightness = ddcBrightness;
                displays = new MacDisplayService(ddcBrightness.IsSupported);
                _colorTemperature = new MacColorTemperatureService(displays);
            }
            else
            {
                displays = new UnsupportedDisplayService();
                brightness = new UnsupportedBrightnessService();
                _colorTemperature = new UnsupportedColorTemperatureService();
            }
            _schedule = new ScheduleService(_colorTemperature, settings);

            var viewModel = new MainViewModel(settings, displays, brightness, _colorTemperature, _schedule, theme);
            viewModel.ShowWindowRequested += (_, _) =>
            {
                _quickControlsWindow?.Hide();
                ShowMainWindow();
            };
            viewModel.ShowQuickControlsRequested += (_, _) => ShowQuickControls();
            viewModel.QuitRequested += (_, _) => desktop.Shutdown();
            DataContext = viewModel;

            _mainWindow = new MainWindow
            {
                DataContext = viewModel,
            };
            _quickControlsWindow = new QuickControlsWindow
            {
                DataContext = viewModel,
            };
            desktop.MainWindow = _mainWindow;
            desktop.Exit += OnDesktopExit;

            viewModel.ApplyInitialState();
            _schedule.Start();
        }

        base.OnFrameworkInitializationCompleted();
    }

    private void ShowMainWindow()
    {
        if (_mainWindow is null)
            return;

        _mainWindow.Show();
        _mainWindow.Activate();
    }

    private void ShowQuickControls()
    {
        if (_quickControlsWindow is null)
            return;

        if (_quickControlsWindow.IsVisible)
        {
            _quickControlsWindow.Hide();
            return;
        }

        _quickControlsWindow.ShowNearMenuBar();
    }

    private void OnTrayMenuOpening(object? sender, EventArgs e)
    {
        Dispatcher.UIThread.Post(ShowQuickControls, DispatcherPriority.Background);
    }

    private void OnDesktopExit(object? sender, ControlledApplicationLifetimeExitEventArgs e)
    {
        _mainWindow?.AllowClose();
        _quickControlsWindow?.AllowClose();
        _schedule?.Dispose();
        _colorTemperature?.Reset();
        _colorTemperature?.Dispose();
        _brightness?.Dispose();
    }
}