using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using Moonglow.Core;
using Moonglow.Infrastructure;
using Moonglow.Models;
using Moonglow.Native;
using Moonglow.Services;
using Moonglow.ViewModels;
using Moonglow.Views;
using Hardcodet.Wpf.TaskbarNotification;
using Microsoft.Win32;

namespace Moonglow;

public partial class App : Application
{
    private static Mutex? _instanceMutex;
    private static Mutex? _legacyMutex;
    private TaskbarIcon? _tray;
    private EventWaitHandle? _shutdownEvent;
    private RegisteredWaitHandle? _shutdownWait;
    private MonitorService _monitors = null!;
    private BrightnessService _brightness = null!;
    private ColorTemperatureService _color = null!;
    private ScheduleService _schedule = null!;
    private SettingsService _settingsService = null!;
    private ThemeService _theme = null!;
    private MainViewModel _viewModel = null!;
    private MainWindow? _settingsWindow;
    private readonly DispatcherTimer _supportProbeTimer = new() { Interval = TimeSpan.FromSeconds(20) };
    private bool _sessionLocked;
    private DateTime _lastResumeUtc = DateTime.MinValue;
    private int _resumeSequence;
    private int _displayChangePending;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        if (Array.Exists(e.Args, arg => string.Equals(arg, "--shutdown", StringComparison.OrdinalIgnoreCase)))
        {
            RequestRunningInstanceShutdown();
            Shutdown();
            return;
        }

        // Only allow one instance per user session (RunAtStartup + a manual launch would otherwise duplicate).
        if (!TryAcquireMutex(InstanceNames.MutexName, out _instanceMutex))
        {
            Shutdown();
            return;
        }

        // The pre-merge Windows build used a different mutex name. Hold it too: if the old
        // generation is running, refuse to start beside it, and while this instance lives the
        // old one cannot start either. Otherwise both own the display gamma ramps on their
        // own schedule ticks and the tint stops tracking the slider.
        if (!TryAcquireMutex(InstanceNames.LegacyMutexName, out _legacyMutex))
        {
            MessageBox.Show(
                "Another copy (Amber / Night Mode Controller) is already running.\nClose it first, then start Moonglow again.",
                "Moonglow",
                MessageBoxButton.OK,
                MessageBoxImage.Information);
            _instanceMutex?.Dispose();
            _instanceMutex = null;
            Shutdown();
            return;
        }

        // A named mutex alone once let two same-generation builds tint the screen at
        // once, so a process scan backs it up before anything touches the hardware.
        if (IsAnotherCopyRunning())
        {
            MessageBox.Show(
                "Another copy of Moonglow (or the old Amber / Night Mode Controller) is already running.\nClose every copy first (check Task Manager for Moonglow and Amber), then start again.",
                "Moonglow",
                MessageBoxButton.OK,
                MessageBoxImage.Information);
            _legacyMutex?.Dispose();
            _legacyMutex = null;
            _instanceMutex?.Dispose();
            _instanceMutex = null;
            Shutdown();
            return;
        }

        _shutdownEvent = new EventWaitHandle(false, EventResetMode.AutoReset, GetShutdownEventName());
        _shutdownWait = ThreadPool.RegisterWaitForSingleObject(
            _shutdownEvent,
            (_, _) => Dispatcher.BeginInvoke(new Action(Shutdown)),
            null,
            Timeout.Infinite,
            executeOnlyOnce: true);

        ShutdownMode = ShutdownMode.OnExplicitShutdown;

        _settingsService = new SettingsService();
        _settingsService.Load();

        _theme = new ThemeService();
        _theme.ThemeChanged += OnThemeChanged;
        _theme.ApplyForCurrentWindowsMode();

        _monitors = new MonitorService();

        _brightness = new BrightnessService(_monitors);
        _color = new ColorTemperatureService(_monitors);
        _schedule = new ScheduleService(_color, _settingsService);
        _viewModel = new MainViewModel(_brightness, _color, _schedule, _settingsService, _monitors);
        _viewModel.OpenSettingsRequested += (_, _) => OpenSettings();
        _viewModel.ExitRequested += (_, _) => Shutdown();

        var popup = new TrayPopup { DataContext = _viewModel };
        // Opening the popup is the moment before a brightness change, so refresh the display
        // set then. The probe runs on a worker thread: DDC stalls while a panel is waking, and
        // a one-shot failure must not leave the brightness slider disabled.
        popup.IsVisibleChanged += (_, args) =>
        {
            if (args.NewValue is true)
                _viewModel.RefreshDisplaysAsync();
        };

        _tray = new TaskbarIcon
        {
            Icon = IconFactory.CreateTrayIcon(_theme.IsDark),
            ToolTipText = "Moonglow",
            DataContext = _viewModel,
            TrayPopup = popup,
            ContextMenu = BuildContextMenu(),
        };

        _viewModel.ApplyStartupState();
        _viewModel.ApplyInitialState();
        _schedule.Start();

        // A panel that was asleep when the app started can fail its first DDC probe. Retry
        // periodically until brightness becomes available, then stop probing.
        _supportProbeTimer.Tick += (_, _) =>
        {
            if (!_viewModel.HasBrightnessSupport)
                _viewModel.RefreshDisplaysAsync();
        };
        _supportProbeTimer.Start();

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
            // Resume re-initialises the display hardware: handles may be stale and the
            // panel set may have changed while the machine slept.
            _color.Invalidate();
            _viewModel.RefreshDisplays();
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

    private void OnThemeChanged()
    {
        // The mark is monochrome, so it has to be redrawn in the colour that contrasts with
        // the new theme.
        if (_tray is not null)
            _tray.Icon = IconFactory.CreateTrayIcon(_theme.IsDark);

        if (_settingsWindow is not null)
        {
            _theme.ApplyWindowTitleBar(_settingsWindow);
            _settingsWindow.Icon = IconFactory.CreateImageSource(_theme.IsDark);
        }
    }

    private void OnDisplaySettingsChanged(object? sender, EventArgs e)
    {
        // SystemEvents raises this on its own thread; bound collections and services must be
        // touched on the UI thread. Coalesce the bursts drivers send during a mode change.
        if (Interlocked.Exchange(ref _displayChangePending, 1) == 1)
            return;

        Dispatcher.BeginInvoke(new Action(() =>
        {
            Interlocked.Exchange(ref _displayChangePending, 0);

            // The display set changed under us, so ramps captured for a panel that is no
            // longer attached must not be restored onto one that reuses its name.
            _color.Invalidate();
            _viewModel.RefreshDisplaysAsync();
            _viewModel.ReapplyColorTemperature();
        }));
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _supportProbeTimer.Stop();
        SystemEvents.DisplaySettingsChanged -= OnDisplaySettingsChanged;
        SystemEvents.UserPreferenceChanged -= OnUserPreferenceChanged;
        SystemEvents.SessionSwitch -= OnSessionSwitch;
        SystemEvents.PowerModeChanged -= OnPowerModeChanged;
        _schedule?.Stop();
        _color?.Reset();
        _color?.Dispose();
        _monitors?.Dispose();
        _tray?.Dispose();
        _shutdownWait?.Unregister(null);
        _shutdownEvent?.Dispose();
        _legacyMutex?.Dispose();
        _instanceMutex?.Dispose();
        base.OnExit(e);
    }

    private static void RequestRunningInstanceShutdown()
    {
        try
        {
            using EventWaitHandle shutdownEvent = EventWaitHandle.OpenExisting(GetShutdownEventName());
            shutdownEvent.Set();
        }
        catch (WaitHandleCannotBeOpenedException)
        {
        }
        catch (UnauthorizedAccessException)
        {
        }
    }

    /// <summary>Looks for another copy of this app family among the running processes,
    /// excluding this process and the portable launcher that started it.</summary>
    private static bool IsAnotherCopyRunning()
    {
        int selfPid = System.Diagnostics.Process.GetCurrentProcess().Id;
        var snapshot = new System.Collections.Generic.List<(string Name, int Pid)>();

        foreach (System.Diagnostics.Process process in System.Diagnostics.Process.GetProcesses())
        {
            try
            {
                snapshot.Add((process.ProcessName, process.Id));
            }
            catch
            {
                // A process can exit or deny inspection mid-scan; it is not ours to judge.
            }
            finally
            {
                process.Dispose();
            }
        }

        return InstanceGuard.IsAnotherCopyRunning(snapshot, selfPid, InstanceGuard.OwnPrefixes, ParentProcessId(selfPid));
    }

    /// <summary>Direct parent of a process, or -1 when it cannot be determined.</summary>
    private static int ParentProcessId(int pid)
    {
        IntPtr snapshot = NativeMethods.CreateToolhelp32Snapshot(NativeMethods.TH32CS_SNAPPROCESS, 0);
        if (snapshot == IntPtr.Zero || snapshot == new IntPtr(-1))
            return -1;

        try
        {
            var entry = new NativeMethods.PROCESSENTRY32
            {
                dwSize = (uint)System.Runtime.InteropServices.Marshal.SizeOf<NativeMethods.PROCESSENTRY32>(),
            };

            if (!NativeMethods.Process32First(snapshot, ref entry))
                return -1;

            do
            {
                if (entry.th32ProcessID == (uint)pid)
                    return (int)entry.th32ParentProcessID;
            }
            while (NativeMethods.Process32Next(snapshot, ref entry));

            return -1;
        }
        finally
        {
            NativeMethods.CloseHandle(snapshot);
        }
    }

    /// <summary>Takes a session-local single-instance mutex. A mutex abandoned by a
    /// crashed predecessor counts as acquired rather than crashing this launch.</summary>
    private static bool TryAcquireMutex(string name, out Mutex? mutex)
    {
        mutex = new Mutex(false, name, out _);
        try
        {
            if (!mutex.WaitOne(0, false))
            {
                mutex.Dispose();
                mutex = null;
                return false;
            }

            return true;
        }
        catch (AbandonedMutexException)
        {
            // A crashed predecessor left the mutex behind; this thread owns it now.
            return true;
        }
    }

    private static string GetShutdownEventName()
    {
        string processPath = Process.GetCurrentProcess().MainModule?.FileName
            ?? AppDomain.CurrentDomain.BaseDirectory;
        using SHA256 sha256 = SHA256.Create();
        byte[] hash = sha256.ComputeHash(Encoding.UTF8.GetBytes(processPath.ToUpperInvariant()));
        return @"Local\Moonglow.Shutdown." + BitConverter.ToString(hash).Replace("-", string.Empty);
    }
}

