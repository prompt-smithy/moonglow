namespace Amber.MacOS.Models;

public sealed record AppTheme(
    string Name,
    bool IsDark,
    string WindowBackground,
    string CardBackground,
    string ControlBackground,
    string PrimaryText,
    string SecondaryText,
    string Border,
    string Accent,
    string OnAccentText,
    string ButtonHover,
    string StrongText);