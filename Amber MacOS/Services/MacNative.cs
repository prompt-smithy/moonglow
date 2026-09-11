using System.Runtime.InteropServices;

namespace Amber.MacOS.Services;

internal static class MacNative
{
    private const string CoreGraphics = "/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics";

    [DllImport(CoreGraphics)]
    internal static extern int CGGetActiveDisplayList(uint maxDisplays, [Out] uint[]? activeDisplays, out uint displayCount);

    [DllImport(CoreGraphics)]
    internal static extern uint CGMainDisplayID();

    [DllImport(CoreGraphics)]
    internal static extern uint CGDisplayGammaTableCapacity(uint display);

    [DllImport(CoreGraphics)]
    internal static extern int CGSetDisplayTransferByTable(
        uint display,
        uint tableSize,
        float[] redTable,
        float[] greenTable,
        float[] blueTable);

    [DllImport(CoreGraphics)]
    internal static extern void CGDisplayRestoreColorSyncSettings();
}