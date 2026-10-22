using System.Runtime.InteropServices;
using Moonglow.Models;
using static Moonglow.Native.NativeMethods;

namespace Moonglow.Services;

/// <summary>Enumerates physical displays and holds their DDC/CI handles.</summary>
public sealed class MonitorService : IDisposable
{
    private readonly List<MonitorInfo> _monitors = new();
    private readonly List<PHYSICAL_MONITOR> _physical = new();
    private readonly object _sync = new();

    public IReadOnlyList<MonitorInfo> Monitors => _monitors;

    public void Refresh()
    {
        lock (_sync)
        {
            ReleaseHandles();
            _monitors.Clear();
            EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, OnMonitor, IntPtr.Zero);
        }
    }

    /// <summary>Runs an action against each DDC-capable monitor while holding the handle lock,
    /// so a concurrent <see cref="Refresh"/> cannot destroy handles mid-use.</summary>
    public void ForEachDdcMonitor(Action<MonitorInfo> apply)
    {
        lock (_sync)
        {
            foreach (var monitor in _monitors)
                if (monitor.SupportsDdcBrightness)
                    apply(monitor);
        }
    }

    private bool OnMonitor(IntPtr hMonitor, IntPtr hdc, ref RECT rect, IntPtr data)
    {
        var mi = new MONITORINFOEX { cbSize = Marshal.SizeOf<MONITORINFOEX>() };
        if (!GetMonitorInfo(hMonitor, ref mi))
            return true;

        var info = new MonitorInfo
        {
            DeviceName = mi.szDevice,
            Description = mi.szDevice,
            IsPrimary = (mi.dwFlags & MONITORINFOF_PRIMARY) != 0,
        };

        if (GetNumberOfPhysicalMonitorsFromHMONITOR(hMonitor, out uint count) && count > 0)
        {
            var array = new PHYSICAL_MONITOR[count];
            if (GetPhysicalMonitorsFromHMONITOR(hMonitor, count, array))
            {
                var primary = array[0];
                info.PhysicalHandle = primary.hPhysicalMonitor;
                if (!string.IsNullOrWhiteSpace(primary.szPhysicalMonitorDescription))
                    info.Description = primary.szPhysicalMonitorDescription;

                _physical.AddRange(array);

                if (GetMonitorBrightness(primary.hPhysicalMonitor, out uint min, out uint _, out uint max))
                {
                    info.SupportsDdcBrightness = true;
                    info.MinBrightness = min;
                    info.MaxBrightness = max;
                }
            }
        }

        _monitors.Add(info);
        return true;
    }

    private void ReleaseHandles()
    {
        if (_physical.Count > 0)
        {
            DestroyPhysicalMonitors((uint)_physical.Count, _physical.ToArray());
            _physical.Clear();
        }
    }

    public void Dispose()
    {
        lock (_sync)
        {
            ReleaseHandles();
        }
    }
}
