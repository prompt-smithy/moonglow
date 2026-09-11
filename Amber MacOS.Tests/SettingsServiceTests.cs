using System.Text.Json;
using Amber.MacOS.Models;
using Amber.MacOS.Services;

namespace Amber.MacOS.Tests;

public sealed class SettingsServiceTests : IDisposable
{
    private readonly string _directory = Path.Combine(Path.GetTempPath(), $"Amber.Tests.{Guid.NewGuid():N}");

    [Theory]
    [InlineData("Amber Dark", "Dark Amber")]
    [InlineData("Orange", "Dark Orange")]
    [InlineData("Green", "Dark Green")]
    [InlineData("Violet", "Dark Violet")]
    [InlineData("Dark", "Black")]
    [InlineData("Dark Grey", "Dark Grey")]
    [InlineData("unknown", "Dark Amber")]
    public void Load_NormalizesLegacyDarkThemeNames(string savedName, string expectedName)
    {
        var settings = new AppSettings { DarkModeTheme = savedName };
        var service = CreateService(settings);

        service.Load();

        Assert.Equal(expectedName, service.Settings.DarkModeTheme);
    }

    [Fact]
    public void Load_NormalizesRangesAndInvalidLightTheme()
    {
        var settings = new AppSettings
        {
            ManualBrightness = 0,
            ManualColorTempK = 700,
            DayColorTempK = 7000,
            NightColorTempK = 800,
            LightModeTheme = "unknown",
        };
        var service = CreateService(settings);

        service.Load();

        Assert.Equal(1, service.Settings.ManualBrightness);
        Assert.Equal(900, service.Settings.ManualColorTempK);
        Assert.Equal(6600, service.Settings.DayColorTempK);
        Assert.Equal(900, service.Settings.NightColorTempK);
        Assert.Equal("Light Amber", service.Settings.LightModeTheme);
    }

    [Fact]
    public void Load_ImportsLegacyFileWhenAmberFileDoesNotExist()
    {
        string currentPath = Path.Combine(_directory, "Amber", "settings.json");
        string legacyPath = Path.Combine(_directory, "legacy", "settings.json");
        Directory.CreateDirectory(Path.GetDirectoryName(legacyPath)!);
        File.WriteAllText(legacyPath, JsonSerializer.Serialize(new AppSettings { ManualBrightness = 42 }));
        var service = new SettingsService(currentPath, legacyPath);

        service.Load();

        Assert.Equal(42, service.Settings.ManualBrightness);
        Assert.True(File.Exists(currentPath));
    }

    private SettingsService CreateService(AppSettings settings)
    {
        string path = Path.Combine(_directory, Guid.NewGuid().ToString("N"), "settings.json");
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path, JsonSerializer.Serialize(settings));
        return new SettingsService(path, legacyFilePath: null);
    }

    public void Dispose()
    {
        if (Directory.Exists(_directory))
            Directory.Delete(_directory, recursive: true);
    }
}