namespace Moonglow.Core;

/// <summary>Single-instance mutex names for the Windows app.</summary>
/// <remarks>
/// The pre-merge Windows build used the <c>Amber</c> mutex name. The merged build must keep
/// acquiring it for its whole lifetime: otherwise both generations run side by side, and
/// each owns the display gamma ramps on its own schedule ticks, so the screen tint stops
/// tracking either app's slider.
/// </remarks>
public static class InstanceNames
{
    public const string MutexName = @"Local\Moonglow.SingleInstance";

    /// <summary>Name used by the pre-merge Windows build (see <c>App.xaml.cs</c> in the
    /// original repository).</summary>
    public const string LegacyMutexName = @"Local\Amber.SingleInstance";
}
