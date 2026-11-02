using System.IO;

namespace Moonglow.Core;

public static class StartupPaths
{
    /// <summary>A portable child lives in a temporary directory; its launcher is stable.</summary>
    public static string? Resolve(string? processPath, string? launcherPath, Func<string, bool> exists)
        => !string.IsNullOrWhiteSpace(launcherPath) && Path.IsPathRooted(launcherPath) && exists(launcherPath!)
            ? launcherPath : processPath;
}
