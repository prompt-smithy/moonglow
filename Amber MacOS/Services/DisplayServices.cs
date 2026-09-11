using Amber.MacOS.Models;

namespace Amber.MacOS.Services;

public interface IDisplayService
{
    IReadOnlyList<DisplayInfo> Displays { get; }
    IReadOnlyList<DisplayInfo> Refresh();
}

public interface IBrightnessService
{
    bool IsSupported { get; }
    void SetBrightness(int percent);
}

public interface IColorTemperatureService : IDisposable
{
    bool IsSupported { get; }
    bool Apply(int kelvin);
    void Reset();
}

public sealed class UnsupportedDisplayService : IDisplayService
{
    public IReadOnlyList<DisplayInfo> Displays { get; private set; } = Array.Empty<DisplayInfo>();
    public IReadOnlyList<DisplayInfo> Refresh() => Displays;
}

public sealed class UnsupportedBrightnessService : IBrightnessService
{
    public bool IsSupported => false;
    public void SetBrightness(int percent) { }
}

public sealed class UnsupportedColorTemperatureService : IColorTemperatureService
{
    public bool IsSupported => false;
    public bool Apply(int kelvin) => false;
    public void Reset() { }
    public void Dispose() { }
}