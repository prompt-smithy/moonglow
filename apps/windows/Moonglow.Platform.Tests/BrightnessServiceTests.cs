using Moonglow.Services;
using System.Management;
using Xunit;

namespace Moonglow.Platform.Tests;

[Collection("Display worker")]
public sealed class BrightnessServiceTests
{
    [Fact]
    public async Task SlowPanelRefreshKeepsThePreviousCapabilityUntilPublication()
    {
        ManagementObjectSearcher.Query = () => new[] { new ManagementObject() };
        var monitors = new MonitorService { Monitors = new[] { new FakeMonitor("A") } };
        var service = new BrightnessService(monitors);
        using var entered = new ManualResetEventSlim();
        using var release = new ManualResetEventSlim();
        ManagementObjectSearcher.Query = () => { entered.Set(); release.Wait(); return new[] { new ManagementObject() }; };
        var refresh = Task.Run(service.RefreshSupport);
        try
        {
            Assert.True(entered.Wait(3000));
            Assert.True(service.InternalPanelAvailable);
            Assert.True(service.IsSupported);
            Assert.True(service.AllActiveDisplaysSupportHardware);
        }
        finally { release.Set(); await refresh; }
        Assert.True(service.InternalPanelAvailable);
    }
}
