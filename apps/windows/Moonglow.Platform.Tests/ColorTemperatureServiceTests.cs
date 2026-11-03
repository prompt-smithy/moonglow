using System.Diagnostics;
using Moonglow.Core;
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
    public void CompositionFadesMonotonicallyThroughTheWholeWarmRange()
    {
        NativeMethods.Reset();
        NativeMethods.OnWrite = _ => false;
        using var service = new ColorTemperatureService(new MonitorService(), useMagnification: true);
        service.Apply(6500);
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.ColorEffects.Any(e => e.Applied), 3000));

        service.Apply(1200);
        float targetGreen = (float)KelvinConversion.RgbScale(1200).G;
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.ColorEffects.Any(e => e.Applied && Math.Abs(e.Green - targetGreen) < 0.000001f), 10000));
        var frames = NativeMethods.ColorEffects.Where(e => e.Applied).ToArray();
        Assert.True(frames.Length > 50, $"Only {frames.Length} frames reached the compositor.");
        for (int index = 1; index < frames.Length; index++)
        {
            Assert.InRange(frames[index].Green, targetGreen - 0.000001f, frames[index - 1].Green + 0.000001f);
            Assert.InRange(frames[index - 1].Green - frames[index].Green, -0.000001f, 0.02f);
            Assert.InRange(frames[index].Blue, -0.000001f, frames[index - 1].Blue + 0.000001f);
        }
    }

    [Fact]
    public void GammaFadeFramesArePacedRatherThanWrittenInOneBurst()
    {
        NativeMethods.Reset();
        using var service = new ColorTemperatureService(new MonitorService(), useMagnification: false);
        service.Apply(6500);
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.Writes.Count >= 2, 3000));
        service.Apply(6000);
        ushort targetBlue = (ushort)(65535 * KelvinConversion.RgbScale(6000).B);
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.Writes.Any(w => w.Device == new IntPtr(1) && w.Blue == targetBlue), 3000));
        var frames = NativeMethods.Writes.Where(w => w.Device == new IntPtr(1)).ToArray();
        Assert.True(frames.Length >= 10);
        double durationMs = (frames[^1].Timestamp - frames[1].Timestamp) * 1000.0 / Stopwatch.Frequency;
        Assert.True(durationMs >= 100, $"The entire fade was written in {durationMs:F1} ms.");
    }

    [Fact]
    public void CompositionNeverStacksWithGammaOrSwitchesBackAfterInvalidation()
    {
        NativeMethods.Reset();
        NativeMethods.OnWrite = device => device != new IntPtr(2);
        using var service = new ColorTemperatureService(new MonitorService(), useMagnification: true);
        service.Apply(3200);
        float warmGreen = (float)KelvinConversion.RgbScale(3200).G;
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.ColorEffects.Any(e => Math.Abs(e.Green - warmGreen) < 0.000001f), 5000));
        Assert.Empty(NativeMethods.Writes);
        service.Invalidate();
        service.Apply(3500);
        float coolGreen = (float)KelvinConversion.RgbScale(3500).G;
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.ColorEffects.Any(e => Math.Abs(e.Green - coolGreen) < 0.000001f), 3000));
        Assert.Empty(NativeMethods.Writes);
        Assert.DoesNotContain(NativeMethods.ColorEffects, e => e.Green == 1.0f);
        Assert.Single(NativeMethods.NativeThreads.Distinct());
    }

    [Fact]
    public void RefusedCompositionFramesAreRetriedWithoutAdvancingTheFade()
    {
        NativeMethods.Reset();
        int attempts = 0;
        NativeMethods.OnMagnificationWrite = _ => Interlocked.Increment(ref attempts) > 3;
        using var service = new ColorTemperatureService(new MonitorService(), useMagnification: true);
        service.Apply(6000);
        float targetGreen = (float)KelvinConversion.RgbScale(6000).G;
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.ColorEffects.Any(e => e.Applied && Math.Abs(e.Green - targetGreen) < 0.000001f), 5000));
        var firstFrames = NativeMethods.ColorEffects.Take(4).ToArray();
        Assert.Equal(4, firstFrames.Length);
        Assert.All(firstFrames, frame => Assert.Equal(firstFrames[0].Green, frame.Green));
        Assert.All(firstFrames.Take(3), frame => Assert.False(frame.Applied));
        Assert.True(firstFrames[3].Applied);
        Assert.Empty(NativeMethods.Writes);
    }

    [Fact]
    public void ReversingTheSliderRetargetsFromTheLastAcceptedFrame()
    {
        NativeMethods.Reset();
        using var entered = new ManualResetEventSlim();
        using var release = new ManualResetEventSlim();
        int blocked = 0;
        NativeMethods.OnMagnificationWrite = _ =>
        {
            if (Interlocked.Exchange(ref blocked, 1) == 0)
            {
                entered.Set();
                release.Wait();
            }
            return true;
        };
        using var service = new ColorTemperatureService(new MonitorService(), useMagnification: true);
        try
        {
            service.Apply(1200);
            Assert.True(entered.Wait(3000));
            service.Apply(6500);
        }
        finally { release.Set(); }
        float neutralGreen = (float)KelvinConversion.RgbScale(6500).G;
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.ColorEffects.Any(e => e.Applied && Math.Abs(e.Green - neutralGreen) < 0.000001f), 3000));
        var frames = NativeMethods.ColorEffects.Where(e => e.Applied).ToArray();
        Assert.Equal(2, frames.Length);
        Assert.True(frames[1].Green > frames[0].Green);
        Assert.Empty(NativeMethods.Writes);
    }

    [Fact]
    public void DisposeRetriesATransientlyRefusedCompositorClearOnTheSameThread()
    {
        NativeMethods.Reset();
        int clears = 0;
        NativeMethods.OnMagnificationWrite = effect => effect.Transform[6] != 1.0f || Interlocked.Increment(ref clears) >= 3;
        var service = new ColorTemperatureService(new MonitorService(), useMagnification: true);
        service.Apply(6400);
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.ColorEffects.Any(e => e.Applied), 3000));
        service.Dispose();
        Assert.Equal(3, clears);
        Assert.Equal(1.0f, NativeMethods.ColorEffects.Last(e => e.Applied).Green);
        Assert.Single(NativeMethods.NativeThreads.Distinct());
    }

    [Fact]
    public void DisposeHasABoundedWaitWhenTheDriverStalls()
    {
        NativeMethods.Reset();
        using var entered = new ManualResetEventSlim();
        using var release = new ManualResetEventSlim();
        NativeMethods.OnWrite = _ => { entered.Set(); release.Wait(); return true; };
        var service = new ColorTemperatureService(new MonitorService(), useMagnification: false);
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
        var service = new ColorTemperatureService(new MonitorService(), useMagnification: false);
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
        using var service = new ColorTemperatureService(new MonitorService(), useMagnification: false);
        service.Apply(3500);
        Assert.True(SpinWait.SpinUntil(() => NativeMethods.Writes.Count >= 2, 3000));
        Assert.True(attempts >= 3);
    }
}
