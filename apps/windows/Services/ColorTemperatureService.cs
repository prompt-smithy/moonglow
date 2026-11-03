using System.Threading;
using Moonglow.Core;
using static Moonglow.Native.NativeMethods;

namespace Moonglow.Services;

/// <summary>
/// Applies a color temperature to the desktop through the compositor on Windows 8+, or
/// through per-display gamma ramps on Windows 7.
/// </summary>
/// <remarks>
/// Writes happen on one dedicated background thread with latest-value-wins semantics:
/// dragging a slider would otherwise issue a driver transaction for every pixel of
/// movement on the UI thread. A single thread (rather than pool workers) also keeps
/// every Magnification call on the thread that initialized it: the fullscreen effect
/// only works there, and pool rotation starved the fallback with ERROR_NOT_READY.
/// All updates are paced and faded, including compositor updates. One backend owns the tint
/// for the session: switching from gamma to composition mid-fade briefly stacks their effects.
/// </remarks>
public sealed class ColorTemperatureService : IDisposable
{
    private readonly MonitorService _monitors;
    private readonly Dictionary<string, RAMP> _savedGammaRamps = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, IntPtr> _deviceContexts = new(StringComparer.OrdinalIgnoreCase);
    private readonly object _applyGate = new();
    private readonly object _gammaGate = new();
    private readonly ManualResetEventSlim _workAvailable = new(false);
    private readonly AutoResetEvent _stateChanged = new(false);
    private readonly Thread _workerThread;
    private RAMP? _savedDesktopGammaRamp;
    private int _targetKelvin = -1;
    // About 60 frames/s and at most 50 K per accepted frame. A full-range move takes
    // roughly 1.7 s; a 100 K schedule tick fades without changing the schedule itself.
    private const int FrameIntervalMilliseconds = 16;
    private const int FadeStepKelvin = 50;
    private int _displayedKelvin = 6500;
    private bool _useMagnification;
    private bool _disposed;
    private bool _restoreRequested;
    private bool _invalidateRequested;
    private bool _magnificationInitialized;
    private bool _magnificationApplied;
    private int _lastMagError;

    public ColorTemperatureService(MonitorService monitors)
        : this(monitors, Environment.OSVersion.Version >= new Version(6, 2))
    {
    }

    // Explicit backend selection lets the production worker's Windows 7 path be exercised
    // by tests without changing the host OS or touching its real display.
    internal ColorTemperatureService(MonitorService monitors, bool useMagnification)
    {
        _monitors = monitors;
        _useMagnification = useMagnification;
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

            _targetKelvin = MathUtil.Clamp(kelvin, KelvinConversion.MinKelvin, KelvinConversion.MaxKelvin);

            // Inside the same gate the worker resets under: a value arriving between
            // its drained-check and its reset re-wakes it instead of being lost.
            _workAvailable.Set();
            _stateChanged.Set();
        }
    }

    /// <summary>Queues removal of the compositor tint or restoration of legacy gamma ramps.</summary>
    public void RestoreOriginalGammaRamps()
    {
        lock (_applyGate)
        {
            if (_disposed) return;
            _restoreRequested = true;
            _targetKelvin = -1;
            _workAvailable.Set();
            _stateChanged.Set();
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

    /// <summary>Removes the app's display transform.</summary>
    public void Reset()
    {
        RestoreOriginalGammaRamps();
    }

    /// <summary>Drops state that a display change can invalidate: ramps captured for a panel
    /// that is no longer attached, and device contexts for a mode or driver reset.
    /// </summary>
    public void Invalidate()
    {
        lock (_applyGate)
        {
            if (_disposed) return;
            _invalidateRequested = true;
            _workAvailable.Set();
            _stateChanged.Set();
        }
    }

    private void InvalidateOnWorker()
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
            if (_disposed) return;
            _disposed = true;
            _workAvailable.Set();
            _stateChanged.Set();
        }
        // Native calls cannot be cancelled. Cleanup belongs to the same worker
        // that initialized Magnification; a stuck driver gets a bounded grace.
        if (!_workerThread.Join(TimeSpan.FromSeconds(2)))
            LogColorApply("display worker did not stop within the shutdown grace period");
    }

    private void CleanupOnWorker()
    {
        lock (_gammaGate)
        {
            RestoreOriginalGammaRampsLocked();
            // A busy compositor can refuse the first clear as well as an ordinary frame.
            // Keep a short shutdown grace so a transient refusal does not leave the tint on.
            for (int attempt = 0; !ClearMagnification() && attempt < 3; attempt++)
                Thread.Sleep(50);
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
        /// <summary>The display backend refused an update; retry the latest value.</summary>
        RetryLater,
    }

    private void ApplyLoop()
    {
        try { RunApplyLoop(); }
        finally
        {
            try { CleanupOnWorker(); }
            catch (Exception error) { LogColorApply("display cleanup failed: " + error.Message); }
        }
    }

    private void RunApplyLoop()
    {
        // Stall episodes are keyed by target value: each new value gets its own episode.
        // (Keying by time alone let a fresh value inherit an expired episode and give up
        // almost immediately after a long warm drag.)
        int? episodeTarget = null;
        DateTime pendingSince = DateTime.MinValue;

        while (true)
        {
            _workAvailable.Wait();
            // Coalesce input before taking the target snapshot, and pace *every* successful
            // frame. The old tight gamma loop could send an entire fade in under 1 ms.
            Thread.Sleep(FrameIntervalMilliseconds);

            int target;
            bool restore;
            bool invalidate;
            lock (_applyGate)
            {
                if (_disposed) return;
                target = _targetKelvin;
                restore = _restoreRequested;
                invalidate = _invalidateRequested;
                _restoreRequested = false;
                _invalidateRequested = false;
            }
            if (invalidate) InvalidateOnWorker();
            if (restore)
            {
                lock (_gammaGate)
                {
                    RestoreOriginalGammaRampsLocked();
                    if (ClearMagnification())
                        _displayedKelvin = 6500;
                    else
                    {
                        lock (_applyGate) _restoreRequested = true;
                    }
                }
                // Retry a refused clear before applying a new target. Advancing from an
                // assumed neutral state while the old tint remains would cause a flash.
                lock (_applyGate) restore = _restoreRequested;
                if (restore)
                {
                    _stateChanged.WaitOne(50);
                    continue;
                }
            }
            if (target < 0)
            {
                lock (_applyGate)
                {
                    if (_disposed) return;
                    if (_targetKelvin < 0 && !_restoreRequested && !_invalidateRequested)
                        _workAvailable.Reset();
                }
                continue;
            }

            bool freshEpisode = episodeTarget != target;
            episodeTarget = target;
            if (freshEpisode)
                pendingSince = DateTime.UtcNow;

            int next = FadeNext(target);
            ApplyResult result;
            var watch = System.Diagnostics.Stopwatch.StartNew();
            try
            {
                result = ApplySynchronously(next);
            }
            catch
            {
                // Display drivers can transiently reject a ramp; keep serving the latest value.
                result = ApplyResult.RetryLater;
            }
            watch.Stop();
            if (result == ApplyResult.Applied)
                _displayedKelvin = next;

            // A hung driver call below looks exactly like a lost update from the outside:
            // the UI keeps queueing while the screen freezes. Name it when it happens.
            if (watch.Elapsed > TimeSpan.FromSeconds(5))
                LogColorApply($"apply {target}K took {watch.Elapsed.TotalSeconds:F1}s (driver stall?)");

            lock (_applyGate)
            {
                if (_disposed)
                    return;

                if (_targetKelvin != target || _restoreRequested || _invalidateRequested)
                {
                    // A newer value arrived while applying: serve it immediately.
                    continue;
                }

                if (result == ApplyResult.Applied)
                {
                    // Drained: sleep until the next Apply. The reset is under the same
                    // gate as the queue write, so a value arriving between the check
                    // and the reset re-wakes the thread instead of being lost.
                    if (next == target) _workAvailable.Reset();
                    continue;
                }

                // Still failing for the same target: never abandon it (an exited worker
                // would strand a clicked value with no further input coming). Snappy
                // while the user is moving, gentle once stalled.
                if (freshEpisode)
                    LogColorApply($"apply {target}K pending (transient refusal err={_lastMagError}), retrying latest");
            }

            TimeSpan age = DateTime.UtcNow - pendingSince;
            // A new target or shutdown interrupts retry backoff. Never advance the fade on
            // a refused frame, and never queue old intermediates behind a busy compositor.
            _stateChanged.WaitOne(age < TimeSpan.FromSeconds(2) ? 50 : 250);
        }
    }

    /// <summary>Next fade step from what is on screen toward the target. A newer
    /// target queued mid-fade simply retargets from the current step, so drags
    /// glide instead of jumping.</summary>
    private int FadeNext(int target)
    {
        return _displayedKelvin + Math.Sign(target - _displayedKelvin) * Math.Min(FadeStepKelvin, Math.Abs(target - _displayedKelvin));
    }

    private ApplyResult ApplySynchronously(int kelvin)
    {
        var (redScale, greenScale, blueScale) = KelvinConversion.RgbScale(kelvin);
        lock (_gammaGate)
        {
            if (_disposed)
                return ApplyResult.Applied;

            // Select the compositor before touching gamma on modern Windows. GDI can
            // silently ignore warm ramps while returning success, and handing a tinted
            // ramp to the compositor would multiply both transforms for an intermediate frame.
            if (_useMagnification)
            {
                try
                {
                    if (!ApplyMagnification(redScale, greenScale, blueScale))
                        return ApplyResult.RetryLater;
                    LogColorApply($"apply {kelvin}K rgb=({redScale:F3},{greenScale:F3},{blueScale:F3}) via=magnification");
                    return ApplyResult.Applied;
                }
                catch (EntryPointNotFoundException) { _useMagnification = false; }
                catch (DllNotFoundException) { _useMagnification = false; }
            }

            RAMP ramp = BuildRamp(redScale, greenScale, blueScale);
            bool gammaApplied = ApplyToMonitors(ramp, out bool anyMonitorApplied);
            // The desktop DC can only be trusted when no individual panel took a
            // ramp; a desktop write after partial success could overwrite accepted panels.
            if (!gammaApplied && !anyMonitorApplied)
                gammaApplied = ApplyToDesktop(ramp);
            if (gammaApplied)
            {
                LogColorApply($"apply {kelvin}K rgb=({redScale:F3},{greenScale:F3},{blueScale:F3}) via=gamma");
                return ApplyResult.Applied;
            }

            return ApplyResult.RetryLater;
        }
    }

    /// <summary>Diagnostic trail for tint-vs-slider issues: what the app queued vs what
    /// actually reached the screen (composition or legacy gamma ramp).</summary>
    /// <remarks>Best effort; written to <c>%Temp%\Moonglow-color.log</c>. It tells a
    /// dropped/stale apply inside the app apart from a driver that ignores the ramp.
    /// </remarks>
    private static void LogColorApply(string message) => DiagnosticLog.Write(message);

    private bool ApplyToMonitors(RAMP ramp, out bool anyApplied)
    {
        anyApplied = false;
        var monitors = _monitors.Snapshot();
        bool allApplied = monitors.Count > 0;

        foreach (var monitor in monitors)
        {
            IntPtr hdc = GetDeviceContext(monitor.DeviceName);
            if (hdc == IntPtr.Zero)
            {
                allApplied = false;
                continue;
            }

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
                anyApplied = true;
            }
            else
            {
                allApplied = false;
                // A DC can go stale after a driver reset; recreate it on the next write.
                ReleaseDeviceContext(monitor.DeviceName);
            }
        }

        return allApplied;
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
    /// Initializes the magnification runtime on the dedicated display worker. Every effect
    /// update and restoration stays on this same thread. Must hold <c>_gammaGate</c>.
    /// </summary>
    private bool EnsureMagnifierInitialized()
    {
        if (_magnificationInitialized)
            return true;

        if (!MagInitialize())
        {
            _lastMagError = System.Runtime.InteropServices.Marshal.GetLastWin32Error();
            return false;
        }

        _magnificationInitialized = true;
        return true;
    }

    private bool ApplyMagnification(double redScale, double greenScale, double blueScale)
    {
        if (!EnsureMagnifierInitialized())
            return false;

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
            _magnificationApplied = true;
            return true;
        }

        _lastMagError = System.Runtime.InteropServices.Marshal.GetLastWin32Error();
        return false;
    }

    /// <summary>Removes the composition tint. Returns whether no effect is active
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
