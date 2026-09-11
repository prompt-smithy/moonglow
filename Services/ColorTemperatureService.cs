using static Amber.Native.NativeMethods;

namespace Amber.Services;

/// <summary>
/// Applies a color temperature to every display. Prefers the Magnification full-screen
/// color effect (a color matrix that is NOT subject to the Windows gamma-range clamp, so it
/// can reach very warm temperatures), and falls back to the GPU gamma ramp if unavailable.
/// </summary>
public sealed class ColorTemperatureService : IDisposable
{
    private readonly MonitorService _monitors;
    private readonly bool _magReady;

    public ColorTemperatureService(MonitorService monitors)
    {
        _monitors = monitors;
        _magReady = MagInitialize();
    }

    /// <summary>Applies the given color temperature (Kelvin) to all displays.</summary>
    public void Apply(int kelvin)
    {
        var (r, g, b) = KelvinToRgbScale(kelvin);

        if (_magReady && ApplyViaMagnification(r, g, b))
            return;

        ApplyViaGamma(r, g, b);
    }

    /// <summary>Restores neutral color (identity transform).</summary>
    public void Reset()
    {
        if (_magReady && ApplyViaMagnification(1.0, 1.0, 1.0))
            return;

        ApplyViaGamma(1.0, 1.0, 1.0);
    }

    public void Dispose()
    {
        if (_magReady)
            MagUninitialize();
    }

    private static bool ApplyViaMagnification(double redScale, double greenScale, double blueScale)
    {
        var effect = new MAGCOLOREFFECT
        {
            transform = new[]
            {
                (float)redScale, 0f,               0f,                0f, 0f,
                0f,              (float)greenScale, 0f,                0f, 0f,
                0f,              0f,                (float)blueScale,  0f, 0f,
                0f,              0f,                0f,                1f, 0f,
                0f,              0f,                0f,                0f, 1f,
            },
        };
        return MagSetFullscreenColorEffect(ref effect);
    }

    private void ApplyViaGamma(double redScale, double greenScale, double blueScale)
    {
        var ramp = BuildRamp(redScale, greenScale, blueScale);

        bool applied = false;
        foreach (var monitor in _monitors.Monitors)
        {
            IntPtr hdc = CreateDC("DISPLAY", monitor.DeviceName, null, IntPtr.Zero);
            if (hdc == IntPtr.Zero)
                continue;

            if (SetDeviceGammaRamp(hdc, ref ramp))
                applied = true;

            DeleteDC(hdc);
        }

        // Fall back to the whole desktop DC if per-monitor DCs were unavailable.
        if (!applied)
        {
            IntPtr hdc = GetDC(IntPtr.Zero);
            if (hdc != IntPtr.Zero)
            {
                SetDeviceGammaRamp(hdc, ref ramp);
                ReleaseDC(IntPtr.Zero, hdc);
            }
        }
    }

    private static RAMP BuildRamp(double redScale, double greenScale, double blueScale)
    {
        var ramp = new RAMP
        {
            Red = new ushort[256],
            Green = new ushort[256],
            Blue = new ushort[256],
        };

        for (int i = 0; i < 256; i++)
        {
            int level = i * 257; // scale 0..255 -> 0..65535
            ramp.Red[i] = (ushort)Math.Clamp(level * redScale, 0.0, 65535.0);
            ramp.Green[i] = (ushort)Math.Clamp(level * greenScale, 0.0, 65535.0);
            ramp.Blue[i] = (ushort)Math.Clamp(level * blueScale, 0.0, 65535.0);
        }

        return ramp;
    }

    /// <summary>
    /// Converts a color temperature to per-channel scale factors (0..1) using the
    /// Tanner Helland approximation, normalized so ~6500K is neutral white.
    /// </summary>
    public static (double R, double G, double B) KelvinToRgbScale(int kelvin)
    {
        double temp = Math.Clamp(kelvin, 900, 40000) / 100.0;
        double r, g, b;

        if (temp <= 66)
            r = 255;
        else
            r = Math.Clamp(329.698727446 * Math.Pow(temp - 60, -0.1332047592), 0, 255);

        if (temp <= 66)
            g = Math.Clamp(99.4708025861 * Math.Log(temp) - 161.1195681661, 0, 255);
        else
            g = Math.Clamp(288.1221695283 * Math.Pow(temp - 60, -0.0755148492), 0, 255);

        if (temp >= 66)
            b = 255;
        else if (temp <= 19)
            b = 0;
        else
            b = Math.Clamp(138.5177312231 * Math.Log(temp - 10) - 305.0447927307, 0, 255);

        return (r / 255.0, g / 255.0, b / 255.0);
    }
}
