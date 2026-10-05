using System.IO;
using System.Text.Json;
using Amber.Infrastructure;
using Amber.Models;

namespace Amber.Services;

/// <summary>Loads and saves <see cref="AppSettings"/> as JSON under %AppData%\Amber.</summary>
public sealed class SettingsService
{
    private static readonly string Dir =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Amber");

    private static readonly string FilePath = Path.Combine(Dir, "settings.json");

    private static readonly JsonSerializerOptions Options = new() { WriteIndented = true };

    public AppSettings Settings { get; private set; } = new();

    public void Load()
    {
        try
        {
            if (File.Exists(FilePath))
                Settings = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(FilePath)) ?? new AppSettings();

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
