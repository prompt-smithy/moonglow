using System.IO;
using System.Text.Json;
using Moonglow.Models;

namespace Moonglow.Services;

/// <summary>Loads and saves <see cref="AppSettings"/> as JSON under %AppData%\Moonglow.</summary>
/// <remarks>
/// Decoding is lenient like the macOS and Linux ports: a missing or malformed value falls
/// back to its default instead of failing the whole load. A file that is not JSON at all is
/// moved aside as <c>settings.json.corrupt</c> before defaults are used, so the next save
/// cannot silently destroy it.
/// </remarks>
public sealed class SettingsService
{
    private static readonly string Dir =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Moonglow");

    private static readonly string FilePath = Path.Combine(Dir, "settings.json");

    /// <summary>Location used before the app was renamed from Amber.</summary>
    private static readonly string LegacyFilePath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Amber", "settings.json");

    private static readonly JsonSerializerOptions Options = new()
    {
        WriteIndented = true,
        // The shared on-disk shape is camelCase (spec/settings.schema.json); reads still
        // accept the PascalCase spelling earlier Windows builds wrote.
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
    };

    private readonly string _filePath;
    private readonly string _legacyFilePath;

    public AppSettings Settings { get; private set; } = new();

    public SettingsService()
        : this(FilePath, LegacyFilePath)
    {
    }

    /// <summary>Points the store at a specific file, for tests.</summary>
    public SettingsService(string filePath, string? legacyFilePath = null)
    {
        _filePath = filePath;
        _legacyFilePath = legacyFilePath ?? string.Empty;
    }

    public void Load()
    {
        string? source = File.Exists(_filePath) ? _filePath
            : !string.IsNullOrEmpty(_legacyFilePath) && File.Exists(_legacyFilePath) ? _legacyFilePath
            : null;

        if (source is null)
        {
            Settings = new AppSettings();
            return;
        }

        try
        {
            Settings = Parse(File.ReadAllText(source)).Normalized();
        }
        catch (Exception)
        {
            if (source == _filePath)
                PreserveCorruptFile();
            Settings = new AppSettings();
        }
    }

    public void Save()
    {
        try
        {
            string? directory = Path.GetDirectoryName(_filePath);
            if (!string.IsNullOrEmpty(directory))
                Directory.CreateDirectory(directory);
            File.WriteAllText(_filePath, JsonSerializer.Serialize(Settings.Normalized(), Options));
        }
        catch
        {
            // Persisting settings is best-effort; ignore IO failures.
        }
    }

    /// <summary>Reads each value independently so one bad field cannot discard the rest.</summary>
    private static AppSettings Parse(string json)
    {
        using JsonDocument document = JsonDocument.Parse(json);
        JsonElement root = document.RootElement;
        if (root.ValueKind != JsonValueKind.Object)
            throw new JsonException("settings root is not an object");

        var defaults = new AppSettings();
        return new AppSettings
        {
            DayColorTempK = ReadInt(root, "dayColorTempK", defaults.DayColorTempK),
            NightColorTempK = ReadInt(root, "nightColorTempK", defaults.NightColorTempK),
            MorningStart = ReadTime(root, "morningStart", defaults.MorningStart),
            MorningEnd = ReadTime(root, "morningEnd", defaults.MorningEnd),
            EveningStart = ReadTime(root, "eveningStart", defaults.EveningStart),
            EveningEnd = ReadTime(root, "eveningEnd", defaults.EveningEnd),
            AutoColorTemperature = ReadBool(root, "autoColorTemperature", defaults.AutoColorTemperature),
            ManualBrightness = ReadInt(root, "manualBrightness", defaults.ManualBrightness),
            ManualColorTempK = ReadInt(root, "manualColorTempK", defaults.ManualColorTempK),
            LaunchAtLogin = ReadBool(root, "launchAtLogin", defaults.LaunchAtLogin, legacyName: "runAtStartup"),
        };
    }

    /// <summary>
    /// Finds a setting under its canonical camelCase name, falling back to the PascalCase
    /// spelling earlier Windows builds wrote, so an existing settings file survives the
    /// move to the shared on-disk shape.
    /// </summary>
    private static bool TryGet(
        JsonElement root,
        string name,
        string? legacyName,
        out JsonElement element)
        => root.TryGetProperty(name, out element)
            || root.TryGetProperty(PascalCase(name), out element)
            || (legacyName is not null
                && (root.TryGetProperty(legacyName, out element)
                    || root.TryGetProperty(PascalCase(legacyName), out element)));

    private static string PascalCase(string name)
        => name.Length > 0
            ? char.ToUpperInvariant(name[0]) + name.Substring(1)
            : name;

    private static int ReadInt(JsonElement root, string name, int fallback)
        => TryGet(root, name, legacyName: null, out JsonElement element)
            && element.ValueKind == JsonValueKind.Number
            && element.TryGetInt32(out int value)
            ? value
            : fallback;

    private static bool ReadBool(JsonElement root, string name, bool fallback, string? legacyName = null)
        => TryGet(root, name, legacyName, out JsonElement element)
            && (element.ValueKind == JsonValueKind.True || element.ValueKind == JsonValueKind.False)
            ? element.GetBoolean()
            : fallback;

    private static string ReadTime(JsonElement root, string name, string fallback)
        => TryGet(root, name, legacyName: null, out JsonElement element)
            && element.ValueKind == JsonValueKind.String
            ? AppSettings.NormalizeTime(element.GetString(), fallback)
            : fallback;

    /// <summary>Moves an unparseable settings file aside, replacing any previous preservation.</summary>
    private void PreserveCorruptFile()
    {
        try
        {
            string corruptPath = _filePath + ".corrupt";
            if (File.Exists(corruptPath))
                File.Delete(corruptPath);
            File.Move(_filePath, corruptPath);
        }
        catch
        {
            // Best effort, like everything else here.
        }
    }
}
