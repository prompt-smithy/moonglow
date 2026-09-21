using Amber.MacOS.Services;

namespace Amber.MacOS.Tests;

public sealed class ThemeCatalogTests
{
    [Fact]
    public void LightThemes_HaveRequiredOrder()
    {
        Assert.Equal(
            ["Light Amber", "Light Orange", "Light Green", "Light Violet", "Light Grey", "White"],
            ThemeCatalog.LightNames);
    }

    [Fact]
    public void DarkThemes_HaveRequiredOrder()
    {
        Assert.Equal(
            ["Dark Amber", "Dark Orange", "Dark Green", "Dark Violet", "Dark Grey", "Black"],
            ThemeCatalog.DarkNames);
    }

    [Fact]
    public void AmberPalettes_UseRequiredAccentColors()
    {
        Assert.Equal("#FFB300", ThemeCatalog.Light[0].Accent);
        Assert.Equal("#FFC107", ThemeCatalog.Dark[0].Accent);
    }

    [Theory]
    [InlineData("White", "#1A1A1A")]
    [InlineData("Dark Grey", "#B0B4BA")]
    [InlineData("Black", "#A4AAB2")]
    public void BluePalettes_UseNeutralControlAccents(string themeName, string expected)
    {
        var theme = ThemeCatalog.Light.Concat(ThemeCatalog.Dark).Single(theme => theme.Name == themeName);

        Assert.Equal(expected, ThemeCatalog.ControlAccent(theme));
    }

    [Theory]
    [InlineData("Light Amber")]
    [InlineData("Light Orange")]
    [InlineData("Light Green")]
    [InlineData("Light Violet")]
    public void ColoredPalettes_UseThemeAccentForControls(string themeName)
    {
        var theme = ThemeCatalog.Light.Single(theme => theme.Name == themeName);

        Assert.Equal(theme.Accent, ThemeCatalog.ControlAccent(theme));
    }

    [Fact]
    public void Themes_DoNotUsePureWhite()
    {
        foreach (var theme in ThemeCatalog.Light.Concat(ThemeCatalog.Dark))
        {
            string[] colors =
            [
                theme.WindowBackground,
                theme.CardBackground,
                theme.ControlBackground,
                theme.PrimaryText,
                theme.SecondaryText,
                theme.Border,
                theme.Accent,
                theme.OnAccentText,
                theme.ButtonHover,
                theme.StrongText,
            ];

            Assert.DoesNotContain(colors, color => color.Equals("#FFFFFF", StringComparison.OrdinalIgnoreCase));
        }
    }
}