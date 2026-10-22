namespace Amber.Infrastructure;

/// <summary>
/// Helpers that exist on modern .NET but not on .NET Framework 4.8.
/// <see cref="System.Math.Clamp(int, int, int)"/> was added in .NET Standard 2.1, which
/// .NET Framework 4.8 does not implement, so these stand in for it.
/// </summary>
internal static class MathUtil
{
    public static int Clamp(int value, int min, int max)
        => value < min ? min : value > max ? max : value;

    public static double Clamp(double value, double min, double max)
        => value < min ? min : value > max ? max : value;
}
