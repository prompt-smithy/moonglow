using System.IO;
using System.Text.Json;
using Moonglow.Infrastructure;
using Moonglow.Models;

namespace Moonglow.Services;

/// <summary>Loads and saves <see cref="AppSettings"/> as JSON under %AppData%\Moonglow.</summary>
public sealed class SettingsService
{
    private static readonly string Dir =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Moonglow");

    private static readonly string FilePath = Path.Combine(Dir, "settings.json");

    /// <summary>Location used before the app was renamed from Amber.</summary>
    private static readonly string LegacyFilePath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Amber", "settings.json");

    private static readonly JsonSerializerOptions Options = new() { WriteIndented = true };

    public AppSettings Settings { get; private set; } = new();

    public void Load()
    {
        try
        {
            string source = File.Exists(FilePath) ? FilePath : LegacyFilePath;
            if (File.Exists(source))
                Settings = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(source)) ?? new AppSettings();

            Settings.RunAtStartup = StartupManager.IsEnabled();
        }
        catch
        {
            Settings = new AppSettings { RunAtStartup = StartupManager.IsEnabled() };
        }
    }

    public void Save()
    {
        try
        {
            Directory.CreateDirectory(Dir);
            File.WriteAllText(FilePath, JsonSerializer.Serialize(Settings, Options));
        }
        catch
        {
            // Persisting settings is best-effort; ignore IO failures.
        }
    }
}
