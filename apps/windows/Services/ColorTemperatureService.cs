using Amber.Infrastructure;
using static Amber.Native.NativeMethods;

namespace Amber.Services;

/// <summary>
/// Applies a color temperature to every display through its per-channel GPU gamma LUT.
/// </summary>
/// <remarks>
/// Gamma-LUT support varies by display driver. The original ramp is captured before the first
/// change and restored when Amber exits or when the session unlocks.
/// </remarks>
public sealed class ColorTemperatureService : IDisposable
{
    private readonly MonitorService _monitors;
    private readonly Dictionary<string, RAMP> _savedGammaRamps = new(StringComparer.OrdinalIgnoreCase);
    private RAMP? _savedDesktopGammaRamp;

    public ColorTemperatureService(MonitorService monitors)
    {
        _monitors = monitors;
    }

    /// <summary>Applies the given color temperature (Kelvin) to all displays.</summary>
    public void Apply(int kelvin)
    {
        var (r, g, b) = KelvinToRgbScale(kelvin);
        ApplyViaGamma(r, g, b);
    }

    /// <summary>Restores the gamma ramps captured before Amber changed them.</summary>
    public void RestoreOriginalGammaRamps()
    {
        foreach (var entry in _savedGammaRamps.ToArray())
        {
            IntPtr hdc = CreateDC("DISPLAY", entry.Key, null, IntPtr.Zero);
            if (hdc == IntPtr.Zero)
                continue;

            RAMP originalRamp = entry.Value;
            if (SetDeviceGammaRamp(hdc, ref originalRamp))
                _savedGammaRamps.Remove(entry.Key);

            DeleteDC(hdc);
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

    public void Dispose()
    {
        RestoreOriginalGammaRamps();
    }

    private void ApplyViaGamma(double redScale, double greenScale, double blueScale)
    {
        var ramp = BuildRamp(redScale, greenScale, blueScale);
        bool appliedToMonitor = false;

        foreach (var monitor in _monitors.Monitors)
        {
            IntPtr hdc = CreateDC("DISPLAY", monitor.DeviceName, null, IntPtr.Zero);
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

            DeleteDC(hdc);
        }

        if (appliedToMonitor)
            return;

        IntPtr desktopHdc = GetDC(IntPtr.Zero);
        if (desktopHdc == IntPtr.Zero)
            return;

        bool desktopCaptured = _savedDesktopGammaRamp.HasValue;
        RAMP originalDesktopRamp = _savedDesktopGammaRamp ?? CreateEmptyRamp();
        if (desktopCaptured || GetDeviceGammaRamp(desktopHdc, ref originalDesktopRamp))
        {
            if (SetDeviceGammaRamp(desktopHdc, ref ramp))
                _savedDesktopGammaRamp = originalDesktopRamp;
        }

        ReleaseDC(IntPtr.Zero, desktopHdc);
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

    /// <summary>
    /// Converts a color temperature to per-channel scale factors (0..1) using the
    /// Tanner Helland approximation, normalized so ~6500K is neutral white.
    /// </summary>
    public static (double R, double G, double B) KelvinToRgbScale(int kelvin)
    {
        double temp = MathUtil.Clamp(kelvin, 900, 40000) / 100.0;
        double r, g, b;

        if (temp <= 66)
            r = 255;
        else
            r = MathUtil.Clamp(329.698727446 * Math.Pow(temp - 60, -0.1332047592), 0, 255);

        if (temp <= 66)
            g = MathUtil.Clamp(99.4708025861 * Math.Log(temp) - 161.1195681661, 0, 255);
        else
            g = MathUtil.Clamp(288.1221695283 * Math.Pow(temp - 60, -0.0755148492), 0, 255);

        if (temp >= 66)
            b = 255;
        else if (temp <= 19)
            b = 0;
        else
            b = MathUtil.Clamp(138.5177312231 * Math.Log(temp - 10) - 305.0447927307, 0, 255);

        return (r / 255.0, g / 255.0, b / 255.0);
    }
}
