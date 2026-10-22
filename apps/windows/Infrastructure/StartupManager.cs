using System;
using System.Diagnostics;
using System.IO;
using Microsoft.Win32;

namespace Amber.Infrastructure;

/// <summary>Adds or removes the app from the per-user "run at sign-in" registry key.</summary>
internal static class StartupManager
{
    private const string RunKey = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "Night Mode Controller";
    private static readonly string[] LegacyValueNames = { "Amber", "Amber.exe" };

    public static bool IsEnabled()
    {
        try
        {
            using RegistryKey? key = Registry.CurrentUser.OpenSubKey(RunKey, writable: false);
            if (key is null)
                return false;

            Process process = Process.GetCurrentProcess();
            string? exe = NormalizeExecutablePath(process.MainModule == null ? null : process.MainModule.FileName);
            foreach (string valueName in new[] { ValueName })
            {
                string? value = key.GetValue(valueName) as string;
                if (string.IsNullOrWhiteSpace(value))
                    continue;

                if (string.Equals(NormalizeExecutablePath(value), exe, StringComparison.OrdinalIgnoreCase))
                    return true;
            }

            foreach (string legacyName in LegacyValueNames)
            {
                string? value = key.GetValue(legacyName) as string;
                if (string.IsNullOrWhiteSpace(value))
                    continue;

                if (string.Equals(NormalizeExecutablePath(value), exe, StringComparison.OrdinalIgnoreCase))
                    return true;
            }
        }
        catch
        {
            // Registry access can fail under locked-down policies; ignore.
        }

        return false;
    }

    public static void Set(bool enabled)
    {
        try
        {
            using RegistryKey? key = Registry.CurrentUser.OpenSubKey(RunKey, writable: true);
            if (key is null)
                return;

            if (enabled)
            {
                Process process = Process.GetCurrentProcess();
                string? exe = process.MainModule == null ? null : process.MainModule.FileName;
                if (!string.IsNullOrEmpty(exe))
                {
                    key.SetValue(ValueName, $"\"{exe}\"");
                    foreach (string legacyName in LegacyValueNames)
                        key.DeleteValue(legacyName, throwOnMissingValue: false);
                }
            }
            else
            {
                key.DeleteValue(ValueName, throwOnMissingValue: false);
                foreach (string legacyName in LegacyValueNames)
                    key.DeleteValue(legacyName, throwOnMissingValue: false);
            }
        }
        catch
        {
            // Registry access can fail under locked-down policies; ignore.
        }
    }

    private static string? NormalizeExecutablePath(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
            return null;

        string trimmed = value!.Trim();
        if (trimmed.Length >= 2 && trimmed.StartsWith("\"") && trimmed.EndsWith("\""))
            trimmed = trimmed.Substring(1, trimmed.Length - 2);

        try
        {
            return Path.GetFullPath(trimmed);
        }
        catch
        {
            return trimmed;
        }
    }
}

