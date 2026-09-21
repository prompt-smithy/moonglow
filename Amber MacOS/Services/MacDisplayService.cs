using Amber.MacOS.Models;

namespace Amber.MacOS.Services;

public sealed class MacDisplayService : IDisplayService
{
    private readonly bool _supportsDdcBrightness;

    public MacDisplayService(bool supportsDdcBrightness = false)
    {
        _supportsDdcBrightness = supportsDdcBrightness;
    }

    public IReadOnlyList<DisplayInfo> Displays { get; private set; } = Array.Empty<DisplayInfo>();

    public IReadOnlyList<DisplayInfo> Refresh()
    {
        if (!OperatingSystem.IsMacOS())
            return Displays = Array.Empty<DisplayInfo>();

        if (MacNative.CGGetActiveDisplayList(0, null, out uint count) != 0 || count == 0)
            return Displays = Array.Empty<DisplayInfo>();

        var displayIds = new uint[count];
        if (MacNative.CGGetActiveDisplayList(count, displayIds, out uint actualCount) != 0)
            return Displays = Array.Empty<DisplayInfo>();

        uint mainDisplay = MacNative.CGMainDisplayID();
        Displays = displayIds
            .Take((int)actualCount)
            .Select((id, index) =>
            {
                bool supportsGammaAdjustments = MacNative.CGDisplayGammaTableCapacity(id) > 0;
                return new DisplayInfo(
                    id,
                    id == mainDisplay ? "Built-in or primary display" : $"External display {index + 1}",
                    id.ToString(),
                    id == mainDisplay,
                    SupportsBrightness: _supportsDdcBrightness && id == mainDisplay && MacNative.CGDisplayIsBuiltin(id) == 0,
                    SupportsColorTemperature: supportsGammaAdjustments);
            })
            .ToArray();

        return Displays;
    }
}