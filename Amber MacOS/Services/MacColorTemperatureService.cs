namespace Amber.MacOS.Services;

public sealed class MacColorTemperatureService : IColorTemperatureService
{
    private readonly IDisplayService _displays;

    public MacColorTemperatureService(IDisplayService displays)
    {
        _displays = displays;
    }

    public bool IsSupported => OperatingSystem.IsMacOS();

    public bool Apply(int kelvin)
    {
        if (!IsSupported)
            return false;

        var (redScale, greenScale, blueScale) = KelvinToRgbScale(kelvin);
        bool applied = false;

        foreach (var display in _displays.Displays.Where(display => display.SupportsColorTemperature))
        {
            uint capacity = MacNative.CGDisplayGammaTableCapacity(display.NativeId);
            if (capacity == 0)
                continue;

            BuildRamp(capacity, redScale, greenScale, blueScale, out var red, out var green, out var blue);
            applied |= MacNative.CGSetDisplayTransferByTable(display.NativeId, capacity, red, green, blue) == 0;
        }

        return applied;
    }

    public void Reset()
    {
        if (IsSupported)
            MacNative.CGDisplayRestoreColorSyncSettings();
    }

    public static (double Red, double Green, double Blue) KelvinToRgbScale(int kelvin)
    {
        double temperature = Math.Clamp(kelvin, 900, 40000) / 100.0;
        double red = temperature <= 66
            ? 255
            : Math.Clamp(329.698727446 * Math.Pow(temperature - 60, -0.1332047592), 0, 255);
        double green = temperature <= 66
            ? Math.Clamp((99.4708025861 * Math.Log(temperature)) - 161.1195681661, 0, 255)
            : Math.Clamp(288.1221695283 * Math.Pow(temperature - 60, -0.0755148492), 0, 255);
        double blue = temperature >= 66
            ? 255
            : temperature <= 19
                ? 0
                : Math.Clamp((138.5177312231 * Math.Log(temperature - 10)) - 305.0447927307, 0, 255);

        return (red / 255.0, green / 255.0, blue / 255.0);
    }

    private static void BuildRamp(
        uint capacity,
        double redScale,
        double greenScale,
        double blueScale,
        out float[] red,
        out float[] green,
        out float[] blue)
    {
        red = new float[capacity];
        green = new float[capacity];
        blue = new float[capacity];

        for (int index = 0; index < capacity; index++)
        {
            double level = capacity == 1 ? 1 : index / (capacity - 1.0);
            red[index] = (float)Math.Clamp(level * redScale, 0, 1);
            green[index] = (float)Math.Clamp(level * greenScale, 0, 1);
            blue[index] = (float)Math.Clamp(level * blueScale, 0, 1);
        }
    }

    public void Dispose()
    {
    }
}