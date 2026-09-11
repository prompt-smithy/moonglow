using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using Amber.MacOS.Services;
using Amber.MacOS.ViewModels;
using Amber.MacOS.Views;

namespace Amber.MacOS;

public partial class App : Application
{
    private MainWindow? _mainWindow;
    private ScheduleService? _schedule;
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

            IDisplayService displays = OperatingSystem.IsMacOS()
                ? new MacDisplayService()
                : new UnsupportedDisplayService();
            IBrightnessService brightness = new UnsupportedBrightnessService();
            _colorTemperature = OperatingSystem.IsMacOS()
                ? new MacColorTemperatureService(displays)
                : new UnsupportedColorTemperatureService();
            _schedule = new ScheduleService(_colorTemperature, settings);

            var viewModel = new MainViewModel(settings, displays, brightness, _colorTemperature, _schedule, theme);
            viewModel.ShowWindowRequested += (_, _) => ShowMainWindow();
            viewModel.QuitRequested += (_, _) => desktop.Shutdown();
            DataContext = viewModel;

            _mainWindow = new MainWindow
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

    private void OnDesktopExit(object? sender, ControlledApplicationLifetimeExitEventArgs e)
    {
        _mainWindow?.AllowClose();
        _schedule?.Dispose();
        _colorTemperature?.Reset();
        _colorTemperature?.Dispose();
    }
}