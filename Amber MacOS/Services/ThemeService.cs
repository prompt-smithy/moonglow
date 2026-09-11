using Avalonia;
using Avalonia.Media;
using Avalonia.Styling;
using Amber.MacOS.Models;

namespace Amber.MacOS.Services;

public sealed class ThemeService
{
    private readonly Application _application;
    private readonly SettingsService _settings;

    public ThemeService(Application application, SettingsService settings)
    {
        _application = application;
        _settings = settings;
        _application.ActualThemeVariantChanged += (_, _) => ApplyForCurrentMode();
    }

    public void ApplyForCurrentMode()
    {
        bool isDark = _application.ActualThemeVariant == ThemeVariant.Dark;
        AppTheme theme = isDark
            ? ThemeCatalog.DarkByName(_settings.Settings.DarkModeTheme)
            : ThemeCatalog.LightByName(_settings.Settings.LightModeTheme);

        SetBrush("ThemeWindowBackground", theme.WindowBackground);
        SetBrush("ThemeCardBackground", theme.CardBackground);
        SetBrush("ThemeControlBackground", theme.ControlBackground);
        SetBrush("ThemePrimaryText", theme.PrimaryText);
        SetBrush("ThemeSecondaryText", theme.SecondaryText);
        SetBrush("ThemeBorder", theme.Border);
        SetBrush("ThemeAccent", theme.Accent);
        SetBrush("ThemeOnAccentText", theme.OnAccentText);
        SetBrush("ThemeButtonHover", theme.ButtonHover);
        SetBrush("ThemeStrongText", theme.StrongText);
    }

    private void SetBrush(string key, string color)
        => _application.Resources[key] = new SolidColorBrush(Color.Parse(color));
}