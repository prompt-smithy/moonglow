using System.Diagnostics;
using Microsoft.Win32;

namespace Amber.Infrastructure;

/// <summary>Adds or removes the app from the per-user "run at sign-in" registry key.</summary>
internal static class StartupManager
{
    private const string RunKey = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "Amber";

    public static void Set(bool enabled)
    {
        try
        {
            using RegistryKey? key = Registry.CurrentUser.OpenSubKey(RunKey, writable: true);
            if (key is null)
                return;

            if (enabled)
            {
                // Environment.ProcessPath is .NET 6+; on .NET Framework the running exe path
                // comes from the process module.
                string? exe = Process.GetCurrentProcess().MainModule?.FileName;
                if (!string.IsNullOrEmpty(exe))
                    key.SetValue(ValueName, $"\"{exe}\"");
            }
            else
            {
                key.DeleteValue(ValueName, throwOnMissingValue: false);
            }
        }
        catch
        {
            // Registry access can fail under locked-down policies; ignore.
        }
    }
}
