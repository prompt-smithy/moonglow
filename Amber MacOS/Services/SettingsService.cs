using System.Text.Json;
using Amber.MacOS.Models;

namespace Amber.MacOS.Services;

public sealed class SettingsService
{
    private static readonly JsonSerializerOptions Options = new() { WriteIndented = true };
    private readonly string _filePath;
    private readonly string? _legacyFilePath;

    public SettingsService(string? filePath = null, string? legacyFilePath = null)
    {
        string applicationData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        _filePath = filePath ?? Path.Combine(applicationData, "Amber", "settings.json");
        _legacyFilePath = legacyFilePath ?? Path.Combine(applicationData, "Display" + "Controller", "settings.json");
    }

    public AppSettings Settings { get; private set; } = new();

    public void Load()
    {
        try
        {
            string? sourcePath = File.Exists(_filePath)
                ? _filePath
                : _legacyFilePath is not null && File.Exists(_legacyFilePath)
                    ? _legacyFilePath
                    : null;

            if (sourcePath is not null)
                Settings = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(sourcePath)) ?? new AppSettings();

            Normalize();
            if (sourcePath is not null && sourcePath != _filePath)
                Save();
        }
        catch
        {
            Settings = new AppSettings();
        }
    }

    private void Normalize()
    {
        Settings.ManualBrightness = Math.Clamp(Settings.ManualBrightness, 1, 100);
        Settings.ManualColorTempK = Math.Clamp(Settings.ManualColorTempK, 900, 6600);
        Settings.DayColorTempK = Math.Clamp(Settings.DayColorTempK, 900, 6600);
        Settings.NightColorTempK = Math.Clamp(Settings.NightColorTempK, 900, 6600);

        Settings.LightModeTheme = ThemeCatalog.LightNames.Contains(Settings.LightModeTheme)
            ? Settings.LightModeTheme
            : "Light Amber";

        Settings.DarkModeTheme = Settings.DarkModeTheme switch
        {
            "Amber Dark" => "Dark Amber",
            "Orange" => "Dark Orange",
            "Green" => "Dark Green",
            "Violet" => "Dark Violet",
            "Dark" => "Black",
            var current when ThemeCatalog.DarkNames.Contains(current) => current,
            _ => "Dark Amber",
        };
    }

    public void Save()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(_filePath)!);
            File.WriteAllText(_filePath, JsonSerializer.Serialize(Settings, Options));
        }
        catch
        {
        }
    }
}