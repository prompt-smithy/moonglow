using Moonglow.Core;
using Moonglow.Infrastructure;
using Xunit;

namespace Moonglow.Platform.Tests;

public sealed class StartupControllerTests
{
    [Fact]
    public async Task PackagedLaunchAdoptsWindowsStateInsteadOfRestoringAStalePreference()
    {
        var registration = new FakeRegistration { State = StartupState.Disabled };
        var settings = new AppSettings { LaunchAtLogin = true };
        int saves = 0;
        var controller = new StartupController(registration, settings, () => saves++);

        await controller.InitializeAsync();

        Assert.False(controller.Enabled);
        Assert.False(settings.LaunchAtLogin);
        Assert.True(controller.CanChange);
        Assert.Empty(registration.Requests);
        Assert.Equal(1, saves);
    }

    [Theory]
    [InlineData((int)StartupState.DisabledByUser, false, "Settings > Apps > Startup")]
    [InlineData((int)StartupState.DisabledByPolicy, false, "policy")]
    [InlineData((int)StartupState.EnabledByPolicy, true, "policy")]
    public async Task WindowsOverridesAreDisplayedAndCannotBeChangedByTheApp(int state, bool enabled, string message)
    {
        var registration = new FakeRegistration { State = (StartupState)state };
        var settings = new AppSettings { LaunchAtLogin = !enabled };
        var controller = new StartupController(registration, settings, () => { });

        await controller.InitializeAsync();
        await controller.SetEnabledAsync(!enabled);

        Assert.Equal(enabled, controller.Enabled);
        Assert.Equal(enabled, settings.LaunchAtLogin);
        Assert.False(controller.CanChange);
        Assert.Contains(message, controller.Status);
        Assert.Empty(registration.Requests);
    }

    [Fact]
    public async Task ExplicitEnableWaitsForWindowsBeforeSavingAndPreventsOverlappingChanges()
    {
        var completion = new TaskCompletionSource<StartupState>();
        var registration = new FakeRegistration
        {
            State = StartupState.Disabled,
            OnSet = _ => completion.Task,
        };
        var settings = new AppSettings();
        int saves = 0;
        var controller = new StartupController(registration, settings, () => saves++);
        await controller.InitializeAsync();

        Task change = controller.SetEnabledAsync(true);
        Assert.False(controller.CanChange);
        Assert.False(controller.Enabled);
        Assert.False(settings.LaunchAtLogin);
        Assert.Equal(0, saves);
        await controller.RefreshAsync();
        await controller.SetEnabledAsync(true);
        Assert.Single(registration.Requests);
        Assert.Equal(1, registration.Reads);

        completion.SetResult(StartupState.Enabled);
        await change;
        Assert.True(controller.Enabled);
        Assert.True(settings.LaunchAtLogin);
        Assert.True(controller.CanChange);
        Assert.Equal(1, saves);
    }

    [Fact]
    public async Task RefusedEnableLeavesTheToggleAndPreferenceOff()
    {
        var registration = new FakeRegistration
        {
            State = StartupState.Disabled,
            OnSet = _ => Task.FromResult(StartupState.Disabled),
        };
        var settings = new AppSettings();
        var controller = new StartupController(registration, settings, () => { });
        await controller.InitializeAsync();

        await controller.SetEnabledAsync(true);

        Assert.False(controller.Enabled);
        Assert.False(settings.LaunchAtLogin);
        Assert.True(controller.CanChange);
        Assert.Contains("did not accept", controller.Status);
    }

    [Fact]
    public async Task WindowsDisablingStartupDuringAnEnableRequestIsRespected()
    {
        var registration = new FakeRegistration
        {
            State = StartupState.Disabled,
            OnSet = _ => Task.FromResult(StartupState.DisabledByUser),
        };
        var settings = new AppSettings();
        var controller = new StartupController(registration, settings, () => { });
        await controller.InitializeAsync();

        await controller.SetEnabledAsync(true);

        Assert.False(controller.Enabled);
        Assert.False(settings.LaunchAtLogin);
        Assert.False(controller.CanChange);
        Assert.Contains("Task Manager", controller.Status);
    }

    [Fact]
    public async Task RefreshAdoptsChangesMadeOutsideTheApp()
    {
        var registration = new FakeRegistration { State = StartupState.Enabled };
        var settings = new AppSettings();
        var controller = new StartupController(registration, settings, () => { });
        await controller.InitializeAsync();
        Assert.True(settings.LaunchAtLogin);

        registration.State = StartupState.DisabledByUser;
        await controller.RefreshAsync();
        Assert.False(controller.Enabled);
        Assert.False(settings.LaunchAtLogin);

        registration.State = StartupState.Enabled;
        await controller.RefreshAsync();
        Assert.True(controller.Enabled);
        Assert.True(settings.LaunchAtLogin);
        Assert.Empty(registration.Requests);
    }

    [Fact]
    public async Task UnpackagedLaunchStillRestoresAnEnabledPreference()
    {
        var registration = new FakeRegistration
        {
            State = StartupState.Disabled,
            RestorePreferenceOnLaunch = true,
        };
        var settings = new AppSettings { LaunchAtLogin = true };
        var controller = new StartupController(registration, settings, () => { });

        await controller.InitializeAsync();

        Assert.True(controller.Enabled);
        Assert.True(settings.LaunchAtLogin);
        Assert.Equal(new[] { true }, registration.Requests);
    }

    [Fact]
    public async Task UnpackagedLaunchAdoptsAnExistingEntryWithoutRemovingIt()
    {
        var registration = new FakeRegistration
        {
            State = StartupState.Enabled,
            RestorePreferenceOnLaunch = true,
        };
        var settings = new AppSettings();
        var controller = new StartupController(registration, settings, () => { });

        await controller.InitializeAsync();

        Assert.True(settings.LaunchAtLogin);
        Assert.Empty(registration.Requests);
    }

    [Fact]
    public async Task AFailedDisableKeepsTheLastPreferenceAndRecoversOnRefresh()
    {
        var registration = new FakeRegistration
        {
            State = StartupState.Enabled,
            OnSet = _ => throw new InvalidOperationException("Windows API failure"),
        };
        var settings = new AppSettings { LaunchAtLogin = true };
        var controller = new StartupController(registration, settings, () => { });
        await controller.InitializeAsync();

        await controller.SetEnabledAsync(false);

        Assert.True(controller.Enabled);
        Assert.True(settings.LaunchAtLogin);
        Assert.False(controller.CanChange);
        Assert.Contains("Could not read", controller.Status);

        await controller.RefreshAsync();
        Assert.True(controller.CanChange);
        Assert.True(controller.Enabled);
    }

    [Fact]
    public async Task AnUnavailableApiDoesNotOverwritePreferencesOrTryAnotherRegistration()
    {
        var registration = new FakeRegistration
        {
            OnRead = () => throw new InvalidOperationException("Missing startup task"),
        };
        var settings = new AppSettings { LaunchAtLogin = true };
        int saves = 0;
        var controller = new StartupController(registration, settings, () => saves++);

        await controller.InitializeAsync();
        await controller.SetEnabledAsync(false);

        Assert.True(settings.LaunchAtLogin);
        Assert.False(controller.CanChange);
        Assert.Equal(0, saves);
        Assert.Empty(registration.Requests);
    }

    private sealed class FakeRegistration : IStartupRegistration
    {
        public bool RestorePreferenceOnLaunch { get; init; }
        public StartupState State { get; set; }
        public int Reads { get; private set; }
        public List<bool> Requests { get; } = new();
        public Func<Task<StartupState>>? OnRead { get; init; }
        public Func<bool, Task<StartupState>>? OnSet { get; init; }

        public Task<StartupState> ReadAsync()
        {
            Reads++;
            return OnRead?.Invoke() ?? Task.FromResult(State);
        }

        public Task<StartupState> SetEnabledAsync(bool enabled)
        {
            Requests.Add(enabled);
            if (OnSet is not null)
                return OnSet(enabled);
            State = enabled ? StartupState.Enabled : StartupState.Disabled;
            return Task.FromResult(State);
        }
    }
}
