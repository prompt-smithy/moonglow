using System.IO;
using System.Text.Json;
using Amber.Models;

namespace Amber.Services;

/// <summary>Loads and saves <see cref="AppSettings"/> as JSON under %AppData%\DisplayController.</summary>
public sealed class SettingsService
{
    private static readonly string Dir =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "DisplayController");

    private static readonly string FilePath = Path.Combine(Dir, "settings.json");

    private static readonly JsonSerializerOptions Options = new() { WriteIndented = true };

    public AppSettings Settings { get; private set; } = new();

    public void Load()
    {
        try
        {
            if (File.Exists(FilePath))
                Settings = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(FilePath)) ?? new AppSettings();
        }
        catch
        {
            Settings = new AppSettings();
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
