using Moonglow.Infrastructure;
using Windows.ApplicationModel;
using Xunit;

namespace Moonglow.Platform.Tests;

public sealed class PackagedStartupRegistrationTests
{
    [Fact]
    public async Task ExplicitEnableUsesTheDeclaredTaskAndDisableUpdatesIt()
    {
        StartupTask.Current = new StartupTask { State = StartupTaskState.Disabled };
        var registration = new PackagedStartupRegistration();

        Assert.False(registration.RestorePreferenceOnLaunch);
        Assert.Equal(StartupState.Disabled, await registration.ReadAsync());
        Assert.Equal(StartupState.Enabled, await registration.SetEnabledAsync(true));
        Assert.Equal(StartupState.Disabled, await registration.SetEnabledAsync(false));
        Assert.Equal("MoonglowStartup", StartupTask.RequestedTaskId);
        Assert.Equal(1, StartupTask.Current.EnableRequests);
        Assert.Equal(1, StartupTask.Current.DisableRequests);
    }

    [Theory]
    [InlineData((int)StartupTaskState.DisabledByUser, (int)StartupState.DisabledByUser)]
    [InlineData((int)StartupTaskState.DisabledByPolicy, (int)StartupState.DisabledByPolicy)]
    [InlineData((int)StartupTaskState.EnabledByPolicy, (int)StartupState.EnabledByPolicy)]
    public async Task UserAndPolicyOverridesNeverInvokeEnableOrDisable(int nativeState, int state)
    {
        StartupTask.Current = new StartupTask { State = (StartupTaskState)nativeState };
        var registration = new PackagedStartupRegistration();

        Assert.Equal((StartupState)state, await registration.SetEnabledAsync(true));
        Assert.Equal((StartupState)state, await registration.SetEnabledAsync(false));
        Assert.Equal(0, StartupTask.Current.EnableRequests);
        Assert.Equal(0, StartupTask.Current.DisableRequests);
    }

    [Fact]
    public async Task RefusedEnableReturnsWindowsActualResult()
    {
        StartupTask.Current = new StartupTask
        {
            State = StartupTaskState.Disabled,
            EnableResult = StartupTaskState.DisabledByUser,
        };
        var registration = new PackagedStartupRegistration();

        Assert.Equal(StartupState.DisabledByUser, await registration.SetEnabledAsync(true));
        Assert.Equal(1, StartupTask.Current.EnableRequests);
    }
}
