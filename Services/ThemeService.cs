using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using Amber.Models;
using Microsoft.Win32;
using static Amber.Native.NativeMethods;

namespace Amber.Services;

/// <summary>Applies themes to the app resources and follows the Windows light/dark mode.</summary>
public sealed class ThemeService
{
    private const string PersonalizeKey = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    private readonly SettingsService _settings;

    public AppTheme Current { get; private set; } = ThemeCatalog.ByName("Light Amber");

    public event Action<AppTheme>? ThemeChanged;

    public ThemeService(SettingsService settings) => _settings = settings;

    /// <summary>Applies the user's chosen theme for the current Windows light/dark mode.</summary>
    public void ApplyForCurrentWindowsMode()
    {
        string name = IsWindowsLight()
            ? _settings.Settings.LightModeTheme
            : _settings.Settings.DarkModeTheme;
        Apply(ThemeCatalog.ByName(name));
    }

    public void Apply(AppTheme theme)
    {
        var res = Application.Current.Resources;
        res["ThemeWindowBackground"] = Brush(theme.WindowBackground);
        res["ThemeCardBackground"] = Brush(theme.CardBackground);
        res["ThemeControlBackground"] = Brush(theme.ControlBackground);
        res["ThemePrimaryText"] = Brush(theme.PrimaryText);
        res["ThemeSecondaryText"] = Brush(theme.SecondaryText);
        res["ThemeBorder"] = Brush(theme.Border);
        res["ThemeAccent"] = Brush(theme.Accent);
        res["ThemeOnAccentText"] = Brush(theme.OnAccentText);
        res["ThemeButtonHover"] = Brush(theme.ButtonHover);
        res["ThemeStrongText"] = Brush(theme.StrongText);

        Current = theme;
        ThemeChanged?.Invoke(theme);
    }

    /// <summary>Colors the OS title bar (dark caption for dark themes) to avoid a bright white bar.</summary>
    public void ApplyWindowTitleBar(Window window)
    {
        IntPtr hwnd = new WindowInteropHelper(window).Handle;
        if (hwnd == IntPtr.Zero)
            return;

        int useDark = Current.IsDark ? 1 : 0;
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ref useDark, sizeof(int));

        int caption = ToColorRef(Current.WindowBackground);
        DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, ref caption, sizeof(int));
        int text = ToColorRef(Current.PrimaryText);
        DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, ref text, sizeof(int));
    }

    private static int ToColorRef(string hex)
    {
        var c = (Color)ColorConverter.ConvertFromString(hex);
        return c.R | (c.G << 8) | (c.B << 16);
    }

    public static bool IsWindowsLight()
    {
        try
        {
            using RegistryKey? key = Registry.CurrentUser.OpenSubKey(PersonalizeKey);
            return key?.GetValue("AppsUseLightTheme") is int value ? value != 0 : true;
        }
        catch
        {
            return true;
        }
    }

    private static SolidColorBrush Brush(string hex)
    {
        var color = (Color)ColorConverter.ConvertFromString(hex);
        var brush = new SolidColorBrush(color);
        brush.Freeze();
        return brush;
    }
}
