using System.Management;
using static DisplayController.Native.NativeMethods;

namespace DisplayController.Services;

/// <summary>Controls brightness: WMI for the internal panel, DDC/CI for external monitors.</summary>
public sealed class BrightnessService
{
    private readonly MonitorService _monitors;

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

    /// <summary>Sets brightness (0-100) on every controllable display.</summary>
    public void SetBrightness(int percent)
    {
        percent = Math.Clamp(percent, 0, 100);
        SetInternalBrightness(percent);
        SetExternalBrightness(percent);
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
        foreach (var monitor in _monitors.Monitors)
        {
            if (!monitor.SupportsDdcBrightness)
                continue;

            uint span = monitor.MaxBrightness - monitor.MinBrightness;
            uint value = monitor.MinBrightness + (uint)Math.Round(span * percent / 100.0);
            SetMonitorBrightness(monitor.PhysicalHandle, value);
        }
    }
}
