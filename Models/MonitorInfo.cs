namespace Amber.Models;

/// <summary>Describes a single physical display and its control capabilities.</summary>
public sealed class MonitorInfo
{
    public string DeviceName { get; init; } = string.Empty;   // e.g. \\.\DISPLAY1
    public string Description { get; set; } = string.Empty;
    public bool IsPrimary { get; init; }

    public IntPtr PhysicalHandle { get; set; }
    public bool SupportsDdcBrightness { get; set; }
    public uint MinBrightness { get; set; }
    public uint MaxBrightness { get; set; } = 100;
}
