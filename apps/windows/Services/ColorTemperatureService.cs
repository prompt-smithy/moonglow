using System.Threading;
using Moonglow.Core;
using static Moonglow.Native.NativeMethods;

namespace Moonglow.Services;

/// <summary>
/// Applies a color temperature to every display through its per-channel GPU gamma LUT.
/// </summary>
/// <remarks>
/// Gamma-LUT support varies by display driver. The original ramp is captured before the first
/// change and restored when Moonglow exits or when the session unlocks.
///
/// Writes happen on one dedicated background thread with latest-value-wins semantics:
/// dragging a slider would otherwise issue a driver transaction for every pixel of
/// movement on the UI thread. A single thread (rather than pool workers) also keeps
/// every Magnification call on the thread that initialized it: the fullscreen effect
/// only works there, and pool rotation starved the fallback with ERROR_NOT_READY.
/// Device contexts are cached because recreating them hits the display driver on every write.
/// </remarks>
public sealed class ColorTemperatureService : IDisposable
{
    private readonly MonitorService _monitors;
    private readonly Dictionary<string, RAMP> _savedGammaRamps = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, IntPtr> _deviceContexts = new(StringComparer.OrdinalIgnoreCase);
    private readonly object _applyGate = new();
    private readonly object _gammaGate = new();
    private readonly ManualResetEventSlim _workAvailable = new(false);
    private readonly Thread _workerThread;
    private RAMP? _savedDesktopGammaRamp;
    private int _targetKelvin = -1;
    private bool _disposed;
    private bool _magnificationInitialized;
    private bool _magnificationApplied;
    private readonly HashSet<int> _magInitThreads = new();
    private DateTime _lastMagnifierSetUtc = DateTime.MinValue;
    private int _lastMagError;

    public ColorTemperatureService(MonitorService monitors)
    {
        _monitors = monitors;
        _workerThread = new Thread(ApplyLoop) { IsBackground = true, Name = "ColorApply" };
        _workerThread.Start();
    }

    /// <summary>Queues a color temperature for all displays. Only the latest value is
    /// applied, so a rapid drag coalesces into a single write.</summary>
    public void Apply(int kelvin)
    {
        LogColorApply($"queue {kelvin}K");
        lock (_applyGate)
        {
            if (_disposed)
                return;

            _targetKelvin = kelvin;

            // Inside the same gate the worker resets under: a value arriving between
            // its drained-check and its reset re-wakes it instead of being lost.
            _workAvailable.Set();
        }
    }

    /// <summary>Restores the gamma ramps captured before Moonglow changed them.</summary>
    public void RestoreOriginalGammaRamps()
    {
        lock (_gammaGate)
        {
            RestoreOriginalGammaRampsLocked();
            ClearMagnification();
        }
    }

    private void RestoreOriginalGammaRampsLocked()
    {
        foreach (var entry in _savedGammaRamps.ToArray())
        {
            IntPtr hdc = GetDeviceContext(entry.Key);
            if (hdc == IntPtr.Zero)
                continue;

            RAMP originalRamp = entry.Value;
            if (SetDeviceGammaRamp(hdc, ref originalRamp))
                _savedGammaRamps.Remove(entry.Key);
        }

        if (_savedDesktopGammaRamp is { } desktopRamp)
        {
            IntPtr hdc = GetDC(IntPtr.Zero);
            if (hdc != IntPtr.Zero)
            {
                if (SetDeviceGammaRamp(hdc, ref desktopRamp))
                    _savedDesktopGammaRamp = null;
                ReleaseDC(IntPtr.Zero, hdc);
            }
        }
    }

    /// <summary>Restores the original display gamma ramps.</summary>
    public void Reset()
    {
        RestoreOriginalGammaRamps();
    }

    /// <summary>Drops state that a display change can invalidate: ramps captured for a panel
    /// that is no longer attached, and device contexts for a mode or driver reset.
    /// </summary>
    public void Invalidate()
    {
        lock (_gammaGate)
        {
            var present = new HashSet<string>(
                _monitors.Snapshot().Select(m => m.DeviceName),
                StringComparer.OrdinalIgnoreCase);

            foreach (string missing in _savedGammaRamps.Keys.Where(k => !present.Contains(k)).ToList())
                _savedGammaRamps.Remove(missing);

            foreach (IntPtr hdc in _deviceContexts.Values)
                DeleteDC(hdc);
            _deviceContexts.Clear();
        }
    }

    public void Dispose()
    {
        lock (_applyGate)
        {
            _disposed = true;
        }

        // Wake the worker so it observes disposal and ends. It is a background thread
        // and is never joined: a hung driver call must not block application exit.
        _workAvailable.Set();

        RestoreOriginalGammaRamps();

        lock (_gammaGate)
        {
            if (_magnificationInitialized)
            {
                MagUninitialize();
                _magnificationInitialized = false;
            }

            foreach (IntPtr hdc in _deviceContexts.Values)
                DeleteDC(hdc);
            _deviceContexts.Clear();
        }
    }

    /// <summary>Whether one pass over the current target reached the screen.</summary>
    private enum ApplyResult
    {
        Applied,
        /// <summary>The compositor refused a fallback update; retry the latest value.</summary>
        RetryLater,
    }

    private void ApplyLoop()
    {
        // Stall episodes are keyed by target value: each new value gets its own episode.
        // (Keying by time alone let a fresh value inherit an expired episode and give up
        // almost immediately after a long warm drag.)
        int? episodeTarget = null;
        DateTime pendingSince = DateTime.MinValue;

        while (true)
        {
            _workAvailable.Wait();

            int target;
            lock (_applyGate)
            {
                target = _targetKelvin;
            }

            bool freshEpisode = episodeTarget != target;
            episodeTarget = target;
            if (freshEpisode)
                pendingSince = DateTime.UtcNow;

            ApplyResult result;
            var watch = System.Diagnostics.Stopwatch.StartNew();
            try
            {
                result = ApplySynchronously(target);
            }
            catch
            {
                // Display drivers can transiently reject a ramp; keep serving the latest value.
                result = ApplyResult.Applied;
            }
            watch.Stop();

            // A hung driver call below looks exactly like a lost update from the outside:
            // the UI keeps queueing while the screen freezes. Name it when it happens.
            if (watch.Elapsed > TimeSpan.FromSeconds(5))
                LogColorApply($"apply {target}K took {watch.Elapsed.TotalSeconds:F1}s (driver stall?)");

            lock (_applyGate)
            {
                if (_disposed)
                    return;

                if (_targetKelvin != target)
                {
                    // A newer value arrived while applying: serve it immediately.
                    continue;
                }

                if (result == ApplyResult.Applied)
                {
                    // Drained: sleep until the next Apply. The reset is under the same
                    // gate as the queue write, so a value arriving between the check
                    // and the reset re-wakes the thread instead of being lost.
                    _workAvailable.Reset();
                    continue;
                }

                // Still failing for the same target: never abandon it (an exited worker
                // would strand a clicked value with no further input coming). Snappy
                // while the user is moving, gentle once stalled.
                if (freshEpisode)
                    LogColorApply($"apply {target}K pending (transient refusal err={_lastMagError}), retrying latest");
            }

            TimeSpan age = DateTime.UtcNow - pendingSince;
            System.Threading.Thread.Sleep(age < TimeSpan.FromSeconds(2) ? 200 : 2000);
        }
    }

    private ApplyResult ApplySynchronously(int kelvin)
    {
        var (redScale, greenScale, blueScale) = KelvinConversion.RgbScale(kelvin);
        RAMP ramp = BuildRamp(redScale, greenScale, blueScale);

        lock (_gammaGate)
        {
            if (_disposed)
                return ApplyResult.Applied;

            bool gammaApplied = ApplyToMonitors(ramp) || ApplyToDesktop(ramp);
            if (gammaApplied)
            {
                // The ramp works again (e.g. Night Light was switched off); drop the
                // composition fallback so the two transforms cannot stack. A refused
                // clear would leave them stacked, so it rejoins the retry loop instead.
                if (!ClearMagnification())
                    return ApplyResult.RetryLater;
                LogColorApply($"apply {kelvin}K rgb=({redScale:F3},{greenScale:F3},{blueScale:F3}) via=gamma");
                return ApplyResult.Applied;
            }

            // The driver refused the ramp: tint during composition instead. The hardware
            // ramp is restored only after the fallback takes over (a no-op while already
            // on it), so a refused fallback leaves the last tint in place instead of
            // flashing neutral.
            if (ApplyMagnification(redScale, greenScale, blueScale))
            {
                RestoreOriginalGammaRampsLocked();
                LogColorApply($"apply {kelvin}K rgb=({redScale:F3},{greenScale:F3},{blueScale:F3}) via=magnification");
                return ApplyResult.Applied;
            }

            return ApplyResult.RetryLater;
        }
    }

    /// <summary>Diagnostic trail for tint-vs-slider issues: what the app queued vs what
    /// actually reached the screen (gamma ramp or composition fallback).</summary>
    /// <remarks>Best effort; written to <c>%Temp%\Moonglow-color.log</c>. It tells a
    /// dropped/stale apply inside the app apart from a driver that ignores the ramp.
    /// </remarks>
    private static readonly int ProcessId = System.Diagnostics.Process.GetCurrentProcess().Id;

    private static void LogColorApply(string message)
    {
        try
        {
            System.IO.File.AppendAllText(
                System.IO.Path.Combine(System.IO.Path.GetTempPath(), "Moonglow-color.log"),
                $"{System.DateTime.Now:HH:mm:ss.fff} [p{ProcessId}/t{System.Environment.CurrentManagedThreadId}] {message}{System.Environment.NewLine}");
        }
        catch
        {
            // Diagnostics must never break the apply path.
        }
    }

    private bool ApplyToMonitors(RAMP ramp)
    {
        bool appliedToMonitor = false;

        foreach (var monitor in _monitors.Snapshot())
        {
            IntPtr hdc = GetDeviceContext(monitor.DeviceName);
            if (hdc == IntPtr.Zero)
                continue;

            bool captured = _savedGammaRamps.TryGetValue(monitor.DeviceName, out RAMP originalRamp);
            if (!captured)
            {
                originalRamp = CreateEmptyRamp();
                captured = GetDeviceGammaRamp(hdc, ref originalRamp);
            }

            if (captured && SetDeviceGammaRamp(hdc, ref ramp))
            {
                if (!_savedGammaRamps.ContainsKey(monitor.DeviceName))
                    _savedGammaRamps[monitor.DeviceName] = originalRamp;
                appliedToMonitor = true;
            }
            else
            {
                // A DC can go stale after a driver reset; recreate it on the next write.
                ReleaseDeviceContext(monitor.DeviceName);
            }
        }

        return appliedToMonitor;
    }

    private bool ApplyToDesktop(RAMP ramp)
    {
        IntPtr desktopHdc = GetDC(IntPtr.Zero);
        if (desktopHdc == IntPtr.Zero)
            return false;

        try
        {
            bool desktopCaptured = _savedDesktopGammaRamp.HasValue;
            RAMP originalDesktopRamp = _savedDesktopGammaRamp ?? CreateEmptyRamp();
            if (desktopCaptured || GetDeviceGammaRamp(desktopHdc, ref originalDesktopRamp))
            {
                if (SetDeviceGammaRamp(desktopHdc, ref ramp))
                {
                    _savedDesktopGammaRamp = originalDesktopRamp;
                    return true;
                }
            }

            return false;
        }
        finally
        {
            ReleaseDC(IntPtr.Zero, desktopHdc);
        }
    }

    /// <summary>
    /// Applies the temperature as a fullscreen colour matrix during composition. Used when
    /// the display driver exposes no GDI gamma ramp (Windows Night Light and HDR both take
    /// the ramp away), so the schedule keeps working instead of silently doing nothing.
    /// Returns whether the effect is active afterwards.
    /// </summary>
    /// <summary>Initializes the magnification runtime on the calling thread unless it
    /// already has. The fullscreen effect only works on initialized threads: every
    /// observed success ran on the first calling thread while all other pool threads
    /// got ERROR_NOT_READY. Must be called under <c>_gammaGate</c>.</summary>
    private bool EnsureMagnifierInitialized()
    {
        int here = System.Environment.CurrentManagedThreadId;
        if (_magInitThreads.Contains(here))
            return true;

        if (!MagInitialize())
        {
            _lastMagError = System.Runtime.InteropServices.Marshal.GetLastWin32Error();
            return false;
        }

        _magInitThreads.Add(here);
        _magnificationInitialized = true;
        return true;
    }

    private bool ApplyMagnification(double redScale, double greenScale, double blueScale)
    {
        if (!EnsureMagnifierInitialized())
            return false;

        // Pace fullscreen effect updates: back-to-back sets are refused while a previous
        // one is still going through. The worker loop retries the latest value and
        // converges instead of hammering a busy compositor.
        if ((DateTime.UtcNow - _lastMagnifierSetUtc).TotalMilliseconds < 400)
        {
            _lastMagError = 21;
            return false;
        }

        var effect = new MAGCOLOREFFECT
        {
            Transform = new float[25],
        };
        effect.Transform[0] = (float)redScale;
        effect.Transform[6] = (float)greenScale;
        effect.Transform[12] = (float)blueScale;
        effect.Transform[18] = 1.0f;
        effect.Transform[24] = 1.0f;

        if (MagSetFullscreenColorEffect(ref effect))
        {
            _lastMagnifierSetUtc = DateTime.UtcNow;
            _magnificationApplied = true;
            return true;
        }

        _lastMagError = System.Runtime.InteropServices.Marshal.GetLastWin32Error();
        return false;
    }

    /// <summary>Removes the composition fallback. Returns whether no effect is active
    /// afterwards; a refusal must rejoin the retry loop or the two transforms stack.
    /// </summary>
    private bool ClearMagnification()
    {
        if (!_magnificationApplied)
            return true;

        // A clear refused for thread reasons would leave the two transforms stacked,
        // so it initializes and rejoins the retry loop like any other update.
        if (!EnsureMagnifierInitialized())
            return false;

        var identity = new MAGCOLOREFFECT
        {
            Transform = new float[25],
        };
        identity.Transform[0] = 1.0f;
        identity.Transform[6] = 1.0f;
        identity.Transform[12] = 1.0f;
        identity.Transform[18] = 1.0f;
        identity.Transform[24] = 1.0f;

        if (!MagSetFullscreenColorEffect(ref identity))
        {
            _lastMagError = System.Runtime.InteropServices.Marshal.GetLastWin32Error();
            return false;
        }

        _lastMagnifierSetUtc = DateTime.UtcNow;
        _magnificationApplied = false;
        return true;
    }

    private IntPtr GetDeviceContext(string deviceName)
    {
        if (_deviceContexts.TryGetValue(deviceName, out IntPtr cached))
            return cached;

        IntPtr hdc = CreateDC("DISPLAY", deviceName, null, IntPtr.Zero);
        if (hdc != IntPtr.Zero)
            _deviceContexts[deviceName] = hdc;
        return hdc;
    }

    private void ReleaseDeviceContext(string deviceName)
    {
        if (!_deviceContexts.TryGetValue(deviceName, out IntPtr hdc))
            return;

        _deviceContexts.Remove(deviceName);
        if (hdc != IntPtr.Zero)
            DeleteDC(hdc);
    }

    private static RAMP BuildRamp(double redScale, double greenScale, double blueScale)
    {
        var ramp = CreateEmptyRamp();

        for (int i = 0; i < 256; i++)
        {
            int level = i * 257; // scale 0..255 -> 0..65535
            ramp.Red[i] = (ushort)MathUtil.Clamp(level * redScale, 0.0, 65535.0);
            ramp.Green[i] = (ushort)MathUtil.Clamp(level * greenScale, 0.0, 65535.0);
            ramp.Blue[i] = (ushort)MathUtil.Clamp(level * blueScale, 0.0, 65535.0);
        }

        return ramp;
    }

    private static RAMP CreateEmptyRamp() => new()
    {
        Red = new ushort[256],
        Green = new ushort[256],
        Blue = new ushort[256],
    };
}
