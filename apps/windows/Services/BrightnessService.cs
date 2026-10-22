using System.Management;
using System.Threading.Tasks;
using Moonglow.Infrastructure;
using static Moonglow.Native.NativeMethods;

namespace Moonglow.Services;

/// <summary>Controls brightness: WMI for the internal panel, DDC/CI for external monitors.</summary>
public sealed class BrightnessService
{
    /// <summary>Lowest brightness the app will apply; keeps the screen usable so it never blacks out.</summary>
    public const int MinBrightnessPercent = 1;

    private readonly MonitorService _monitors;

    private readonly object _applyGate = new();
    private int _targetPercent = -1;
    private bool _applying;

    public bool InternalPanelAvailable { get; private set; }

    public BrightnessService(MonitorService monitors)
    {
        _monitors = monitors;
        DetectInternalPanel();
    }

    private void DetectInternalPanel()
    {
        try
        {
            using var searcher = new ManagementObjectSearcher("root\\WMI", "SELECT * FROM WmiMonitorBrightness");
            using var results = searcher.Get();
            foreach (var _ in results)
            {
                InternalPanelAvailable = true;
                break;
            }
        }
        catch
        {
            InternalPanelAvailable = false;
        }
    }

    public int? GetInternalBrightness()
    {
        if (!InternalPanelAvailable)
            return null;

        try
        {
            using var searcher = new ManagementObjectSearcher("root\\WMI", "SELECT * FROM WmiMonitorBrightness");
            using var results = searcher.Get();
            foreach (ManagementObject obj in results)
                return Convert.ToInt32(obj["CurrentBrightness"]);
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
        lock (_applyGate)
        {
            _targetPercent = percent;
            if (_applying)
                return;
            _applying = true;
        }
        Task.Run(ApplyLoop);
    }

    private void ApplyLoop()
    {
        while (true)
        {
            int target;
            lock (_applyGate)
            {
                target = _targetPercent;
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
                    _applying = false;
                    return;
                }
            }
        }
    }

    private void SetInternalBrightness(int percent)
    {
        if (!InternalPanelAvailable)
            return;

        try
        {
            using var searcher = new ManagementObjectSearcher("root\\WMI", "SELECT * FROM WmiMonitorBrightnessMethods");
            using var results = searcher.Get();
            foreach (ManagementObject method in results)
                method.InvokeMethod("WmiSetBrightness", new object[] { (uint)1, (byte)percent });
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
