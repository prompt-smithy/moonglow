using System.Diagnostics;
using Moonglow.Native;
using Moonglow.Services;
using Xunit;

namespace Moonglow.Platform.Tests;

[CollectionDefinition("Display worker", DisableParallelization = true)]
public class DisplayWorkerCollection { }

[Collection("Display worker")]
public sealed class ColorTemperatureServiceTests
{
    [Fact]
    public void PartialGammaSuccessUsesGlobalFallbackAndRestoresTheSuccessfulPanel()
    {
        NativeMethods.Reset();
        NativeMethods.OnWrite = device => device != new IntPtr(2);
        using var service = new ColorTemperatureService(new MonitorService());
        service.Apply(3200);
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.MagnificationWrites > 0 && NativeMethods.Writes.Any(w => w.Device == new IntPtr(1) && w.Blue == ushort.MaxValue), 3000));
        int writes = NativeMethods.Writes.Count;
        service.Apply(3500);
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.MagnificationWrites >= 2, 3000));
        Assert.Equal(writes, NativeMethods.Writes.Count);
    }

    [Fact]
    public void DisposeHasABoundedWaitWhenTheDriverStalls()
    {
        NativeMethods.Reset();
        using var entered = new ManualResetEventSlim();
        using var release = new ManualResetEventSlim();
        NativeMethods.OnWrite = _ => { entered.Set(); release.Wait(); return true; };
        var service = new ColorTemperatureService(new MonitorService());
        service.Apply(3300);
        Assert.True(entered.Wait(3000));
        try
        {
            var stopwatch = Stopwatch.StartNew();
            service.Dispose();
            Assert.True(stopwatch.Elapsed < TimeSpan.FromSeconds(3));
        }
        finally
        {
            release.Set();
            Assert.True(SpinWait.SpinUntil(() => NativeMethods.DeletedContexts >= 2, 3000));
        }
    }

    [Fact]
    public void RestorationAndCleanupStayOnTheDisplayWorker()
    {
        NativeMethods.Reset();
        int caller = Environment.CurrentManagedThreadId;
        var service = new ColorTemperatureService(new MonitorService());
        service.Apply(3400);
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.Writes.Count >= 2, 3000));
        service.RestoreOriginalGammaRamps();
        service.Invalidate();
        service.Dispose();
        Assert.True(NativeMethods.DeletedContexts >= 2);
        Assert.DoesNotContain(caller, NativeMethods.NativeThreads);
    }

    [Fact]
    public void ATransientDriverExceptionRetriesTheSameTarget()
    {
        NativeMethods.Reset();
        int attempts = 0;
        NativeMethods.OnWrite = _ => Interlocked.Increment(ref attempts) == 1 ? throw new InvalidOperationException("driver reset") : true;
        using var service = new ColorTemperatureService(new MonitorService());
        service.Apply(3500);
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.Writes.Count >= 2, 3000));
        Assert.True(attempts >= 3);
    }
}
