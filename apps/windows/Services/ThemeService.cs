using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using Microsoft.Win32;
using static Amber.Native.NativeMethods;

namespace Amber.Services;

/// <summary>
/// Applies the app's greyscale palette (black, white and shades of grey) and follows the
/// Windows light/dark mode. Only two modes exist: light and dark.
/// </summary>
public sealed class ThemeService
{
    private const string PersonalizeKey = @"SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize";

    private string _windowBackground = "#FFF4F4F4";
    private string _primaryText = "#FF1A1A1A";

    /// <summary>True when the dark palette is currently applied.</summary>
    public bool IsDark { get; private set; }

    /// <summary>Raised after the palette changes so open windows can refresh their chrome.</summary>
    public event Action? ThemeChanged;

    /// <summary>Applies the palette that matches the current Windows light/dark mode.</summary>
    public void ApplyForCurrentWindowsMode() => Apply(!IsWindowsLight());

    private void Apply(bool dark)
    {
        var res = Application.Current.Resources;

        if (dark)
        {
            _windowBackground = "#FF1C1C1C";
            _primaryText = "#FFF0F0F0";

            // Dark mode: near-black surfaces, white text, mid-grey controls.
            res["ThemeWindowBackground"] = Brush("#FF1C1C1C");
            res["ThemeCardBackground"] = Brush("#FF262626");
            res["ThemeControlBackground"] = Brush("#FF303030");
            res["ThemePrimaryText"] = Brush("#FFF0F0F0");
            res["ThemeSecondaryText"] = Brush("#FFA8A8A8");
            res["ThemeBorder"] = Brush("#FF3F3F3F");
            res["ThemeAccent"] = Brush("#FFBDBDBD");
            res["ThemeOnAccentText"] = Brush("#FF1C1C1C");
            res["ThemeButtonHover"] = Brush("#FF3A3A3A");
            res["ThemeStrongText"] = Brush("#FFFAFAFA");
        }
        else
        {
            _windowBackground = "#FFF4F4F4";
            _primaryText = "#FF1A1A1A";

            // Light mode: white surfaces, near-black text, grey controls.
            res["ThemeWindowBackground"] = Brush("#FFF4F4F4");
            res["ThemeCardBackground"] = Brush("#FFFFFFFF");
            res["ThemeControlBackground"] = Brush("#FFFFFFFF");
            res["ThemePrimaryText"] = Brush("#FF1A1A1A");
            res["ThemeSecondaryText"] = Brush("#FF6B6B6B");
            res["ThemeBorder"] = Brush("#FFD6D6D6");
            res["ThemeAccent"] = Brush("#FF4A4A4A");
            res["ThemeOnAccentText"] = Brush("#FFFFFFFF");
            res["ThemeButtonHover"] = Brush("#FFE8E8E8");
            res["ThemeStrongText"] = Brush("#FF111111");
        }

        IsDark = dark;
        ThemeChanged?.Invoke();
    }

    /// <summary>Colors the OS title bar to match the palette (dark caption in dark mode).</summary>
    public void ApplyWindowTitleBar(Window window)
    {
        IntPtr hwnd = new WindowInteropHelper(window).Handle;
        if (hwnd == IntPtr.Zero)
            return;

        int useDark = IsDark ? 1 : 0;
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, ref useDark, sizeof(int));

        int caption = ToColorRef(_windowBackground);
        DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, ref caption, sizeof(int));
        int text = ToColorRef(_primaryText);
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
