using System;
using System.Diagnostics;
using System.IO;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using Microsoft.Win32;
using Moonglow.Core;

namespace Moonglow.Infrastructure;

/// <summary>Selects startup registration using the running process's package identity.</summary>
internal static class StartupManager
{
    public static IStartupRegistration Create()
    {
        try
        {
            return HasPackageIdentity() ? CreatePackagedRegistration() : new RegistryStartupRegistration();
        }
        catch
        {
            // An identity or API failure must not send a packaged app down the registry path.
            return new UnavailableStartupRegistration();
        }
    }

    // Keep WinRT type loading off the Windows 7 / unpackaged path, including during JIT.
    [MethodImpl(MethodImplOptions.NoInlining)]
    private static IStartupRegistration CreatePackagedRegistration() => new PackagedStartupRegistration();

    private static bool HasPackageIdentity()
    {
        try
        {
            uint length = 0;
            int result = GetCurrentPackageFullName(ref length, IntPtr.Zero);
            if (result == 15700) // APPMODEL_ERROR_NO_PACKAGE
                return false;
            if (result == 122 || result == 0) // ERROR_INSUFFICIENT_BUFFER / ERROR_SUCCESS
                return true;
            throw new Win32Exception(result);
        }
        catch (EntryPointNotFoundException)
        {
            return false; // Package identity was introduced in Windows 8.
        }
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, ExactSpelling = true)]
    private static extern int GetCurrentPackageFullName(ref uint packageFullNameLength, IntPtr packageFullName);

    private sealed class UnavailableStartupRegistration : IStartupRegistration
    {
        public bool RestorePreferenceOnLaunch => false;
        public Task<StartupState> ReadAsync() => Task.FromResult(StartupState.Unavailable);
        public Task<StartupState> SetEnabledAsync(bool enabled) => ReadAsync();
    }
}

/// <summary>Startup registration for installer and portable builds, including Windows 7.</summary>
internal sealed class RegistryStartupRegistration : IStartupRegistration
{
    private const string RunKey = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "Moonglow";
    private static readonly string[] LegacyValueNames = { "Amber", "Amber.exe", "Night Mode Controller" };

    public bool RestorePreferenceOnLaunch => true;

    public Task<StartupState> ReadAsync() => Task.FromResult(ReadState());

    public Task<StartupState> SetEnabledAsync(bool enabled)
        => Task.FromResult(Set(enabled) ? ReadState() : StartupState.Unavailable);

    private static StartupState ReadState()
    {
        try
        {
            using RegistryKey? key = Registry.CurrentUser.OpenSubKey(RunKey, writable: false);
            if (key is null)
                return StartupState.Disabled;

            string? exe = NormalizeExecutablePath(StartupExecutable());
            foreach (string valueName in new[] { ValueName })
            {
                string? value = key.GetValue(valueName) as string;
                if (string.IsNullOrWhiteSpace(value))
                    continue;

                if (string.Equals(NormalizeExecutablePath(value), exe, StringComparison.OrdinalIgnoreCase))
                    return StartupState.Enabled;
            }

            foreach (string legacyName in LegacyValueNames)
            {
                string? value = key.GetValue(legacyName) as string;
                if (string.IsNullOrWhiteSpace(value))
                    continue;

                if (string.Equals(NormalizeExecutablePath(value), exe, StringComparison.OrdinalIgnoreCase))
                    return StartupState.Enabled;
            }
        }
        catch
        {
            return StartupState.Unavailable;
        }

        return StartupState.Disabled;
    }

    /// <summary>Registers or removes the entry. Returns false when Windows refused the change
    /// (locked-down policy, unusable process path), so the caller never records intent that did
    /// not take effect.</summary>
    private static bool Set(bool enabled)
    {
        try
        {
            using RegistryKey? key = enabled
                ? Registry.CurrentUser.CreateSubKey(RunKey, writable: true)
                : Registry.CurrentUser.OpenSubKey(RunKey, writable: true);
            if (key is null)
                return !enabled;

            if (enabled)
            {
                string? exe = StartupExecutable();
                if (string.IsNullOrEmpty(exe))
                    return false;

                key.SetValue(ValueName, $"\"{exe}\"");
                foreach (string legacyName in LegacyValueNames)
                    key.DeleteValue(legacyName, throwOnMissingValue: false);
            }
            else
            {
                key.DeleteValue(ValueName, throwOnMissingValue: false);
                foreach (string legacyName in LegacyValueNames)
                    key.DeleteValue(legacyName, throwOnMissingValue: false);
            }

            return true;
        }
        catch
        {
            // Registry access can fail under locked-down policies; report the refusal.
            return false;
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

    private static string? StartupExecutable()
    {
        string? launcher = Environment.GetEnvironmentVariable("MOONGLOW_PORTABLE_LAUNCHER");
        using Process process = Process.GetCurrentProcess();
        return StartupPaths.Resolve(process.MainModule?.FileName, launcher, File.Exists);
    }
}

