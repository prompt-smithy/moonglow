using System.Runtime.InteropServices;

namespace Amber.MacOS.Services;

internal static class MacNative
{
    private const string CoreGraphics = "/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics";
    private const string CoreDisplay = "/System/Library/Frameworks/CoreDisplay.framework/CoreDisplay";
    private const string CoreFoundation = "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation";

    [DllImport(CoreGraphics)]
    internal static extern int CGGetActiveDisplayList(uint maxDisplays, [Out] uint[]? activeDisplays, out uint displayCount);

    [DllImport(CoreGraphics)]
    internal static extern uint CGMainDisplayID();

    [DllImport(CoreGraphics)]
    internal static extern int CGDisplayIsBuiltin(uint display);

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

    [DllImport(CoreDisplay)]
    internal static extern IntPtr IOAVServiceCreate(IntPtr allocator);

    [DllImport(CoreDisplay)]
    internal static extern int IOAVServiceReadI2C(
        IntPtr service,
        uint chipAddress,
        uint dataAddress,
        byte[] outputBuffer,
        uint outputBufferSize);

    [DllImport(CoreDisplay)]
    internal static extern int IOAVServiceWriteI2C(
        IntPtr service,
        uint chipAddress,
        uint dataAddress,
        byte[] inputBuffer,
        uint inputBufferSize);

    [DllImport(CoreFoundation)]
    internal static extern void CFRelease(IntPtr value);
}