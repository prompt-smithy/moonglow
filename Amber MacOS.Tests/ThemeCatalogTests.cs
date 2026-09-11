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
}