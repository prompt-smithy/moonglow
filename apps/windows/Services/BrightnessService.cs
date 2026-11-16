using System.Management;
using System.Threading.Tasks;
using Moonglow.Core;
using static Moonglow.Native.NativeMethods;

namespace Moonglow.Services;

/// <summary>Controls brightness: WMI for the internal panel, DDC/CI for external monitors.</summary>
public sealed class BrightnessService
{
    /// <summary>Lowest brightness the app will apply; keeps the screen usable so it never blacks out.</summary>
    public const int MinBrightnessPercent = 1;

    private readonly MonitorService _monitors;

    // A DDC transaction can hang on a monitor that is waking or being unplugged. After this
    // long, a new request starts a fresh worker instead of waiting on the stuck one forever.
    private static readonly TimeSpan ApplyTimeout = TimeSpan.FromSeconds(5);

    private readonly object _applyGate = new();
    private int _targetPercent = -1;
    private bool _applying;
    private DateTime _applyStartedUtc;
    private int _generation;

    private readonly object _stateGate = new();
    private bool _internalPanelAvailable;
    private int _activeWmiBrightnessDisplayCount;

    public bool InternalPanelAvailable
    {
        get { lock (_stateGate) return _internalPanelAvailable; }
        private set { lock (_stateGate) _internalPanelAvailable = value; }
    }

    private int ActiveWmiBrightnessDisplayCount
    {
        get { lock (_stateGate) return _activeWmiBrightnessDisplayCount; }
        set { lock (_stateGate) _activeWmiBrightnessDisplayCount = value; }
    }

    /// <summary>Whether any attached display can take a hardware brightness write.</summary>
    public bool IsSupported
        => InternalPanelAvailable || _monitors.Snapshot().Any(m => m.SupportsDdcBrightness);

    /// <summary>Whether every active display is covered by WMI or DDC/CI.</summary>
    public bool AllActiveDisplaysSupportHardware
    {
        get
        {
            var monitors = _monitors.Snapshot();
            if (monitors.Count == 0)
                return InternalPanelAvailable;

            int ddcDisplays = monitors.Count(m => m.SupportsDdcBrightness);
            int displaysWithoutDdc = monitors.Count - ddcDisplays;
            // WMI can cover only brightness-capable displays that WMI reports as
            // active. A stale/inactive panel must not count for an unrelated
            // active monitor that has no DDC/CI support.
            return ActiveWmiBrightnessDisplayCount >= displaysWithoutDdc;
        }
    }

    public BrightnessService(MonitorService monitors)
    {
        _monitors = monitors;
        DetectInternalPanel();
    }

    /// <summary>Re-detects support, e.g. after a display or dock change. Monitors are
    /// re-enumerated separately by <see cref="MonitorService.Refresh"/>.</summary>
    public void RefreshSupport() => DetectInternalPanel();

    private void DetectInternalPanel()
    {
        int count = 0;
        try
        {
            using var searcher = new ManagementObjectSearcher(
                "root\\WMI",
                "SELECT * FROM WmiMonitorBrightness WHERE Active = TRUE");
            using var results = searcher.Get();
            foreach (ManagementObject monitor in results)
            {
                using (monitor)
                {
                    if (Convert.ToBoolean(monitor["Active"]))
                        count++;
                }
            }
        }
        catch
        {
            count = 0;
        }

        lock (_stateGate)
        {
            _activeWmiBrightnessDisplayCount = count;
            _internalPanelAvailable = count > 0;
        }
    }

    public int? GetInternalBrightness()
    {
        lock (_stateGate)
        {
            if (!_internalPanelAvailable)
                return null;
        }

        try
        {
            using var searcher = new ManagementObjectSearcher(
                "root\\WMI",
                "SELECT * FROM WmiMonitorBrightness WHERE Active = TRUE");
            using var results = searcher.Get();
            foreach (ManagementObject obj in results)
            {
                using (obj)
                {
                    return Convert.ToInt32(obj["CurrentBrightness"]);
                }
            }
        }
        catch
        {
            // fall through
        }

        return null;
    }

    /// <summary>Requests a brightness (clamped to a usable range); applied on a background worker
    /// that coalesces rapid changes so dragging never blocks the UI thread.</summary>
    public void SetBrightness(int percent)
    {
        percent = MathUtil.Clamp(percent, MinBrightnessPercent, 100);

        int generation;
        bool startWorker;
        lock (_applyGate)
        {
            _targetPercent = percent;

            bool stuck = _applying
                && DateTime.UtcNow - _applyStartedUtc >= ApplyTimeout;
            startWorker = !_applying || stuck;
            if (!startWorker)
                return;

            _applying = true;
            _applyStartedUtc = DateTime.UtcNow;
            generation = ++_generation;
        }

        Task.Run(() => ApplyLoop(generation));
    }

    private void ApplyLoop(int generation)
    {
        while (true)
        {
            int target;
            lock (_applyGate)
            {
                target = _targetPercent;
                // Measure the timeout from the start of this attempt, not the worker's birth,
                // so a long drag through many values never looks like a hung transaction.
                _applyStartedUtc = DateTime.UtcNow;
            }

            try
            {
                SetInternalBrightness(target);
                SetExternalBrightness(target);
            }
            catch
            {
                // Hardware can transiently reject a value; keep serving the latest request.
            }

            lock (_applyGate)
            {
                if (_targetPercent == target)
                {
                    // A worker that timed out was replaced; it must not clear the flag owned
                    // by the newer worker.
                    if (generation == _generation)
                        _applying = false;
                    return;
                }
            }
        }
    }

    private void SetInternalBrightness(int percent)
    {
        lock (_stateGate)
        {
            if (!_internalPanelAvailable)
                return;
        }

        try
        {
            using var searcher = new ManagementObjectSearcher(
                "root\\WMI",
                "SELECT * FROM WmiMonitorBrightnessMethods WHERE Active = TRUE");
            using var results = searcher.Get();
            foreach (ManagementObject method in results)
            {
                using (method)
                {
                    method.InvokeMethod("WmiSetBrightness", new object[] { (uint)1, (byte)percent });
                }
            }
        }
        catch
        {
            // Some panels reject WMI brightness; ignore.
        }
    }

    private void SetExternalBrightness(int percent)
    {
        _monitors.ForEachDdcMonitor(monitor =>
        {
            uint span = monitor.MaxBrightness - monitor.MinBrightness;
            uint value = monitor.MinBrightness + (uint)Math.Round(span * percent / 100.0);
            SetMonitorBrightness(monitor.PhysicalHandle, value);
        });
    }
}
