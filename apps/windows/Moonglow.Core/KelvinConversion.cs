namespace Moonglow.Core;

/// <summary>
/// Converts a color temperature to per-channel scale factors (0..1) using the
/// Tanner Helland approximation, normalized so ~6500K is neutral white.
/// </summary>
/// <remarks>
/// The golden vectors in <c>spec/vectors/kelvin.json</c> pin this curve. The clamp is wider
/// than the persisted settings range because the approximation is also exercised at the
/// extremes by the vectors.
/// </remarks>
public static class KelvinConversion
{
    public const int MinKelvin = 900;
    public const int MaxKelvin = 40000;

    public static (double R, double G, double B) RgbScale(int kelvin)
    {
        double temp = MathUtil.Clamp(kelvin, MinKelvin, MaxKelvin) / 100.0;
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
