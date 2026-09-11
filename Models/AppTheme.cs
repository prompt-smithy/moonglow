namespace Amber.Models;

/// <summary>A named color palette applied to the app UI via dynamic resources.</summary>
public sealed class AppTheme
{
    public string Name { get; init; } = string.Empty;
    public bool IsDark { get; init; }

    public string WindowBackground { get; init; } = string.Empty;
    public string CardBackground { get; init; } = string.Empty;
    public string ControlBackground { get; init; } = string.Empty;
    public string PrimaryText { get; init; } = string.Empty;
    public string SecondaryText { get; init; } = string.Empty;
    public string Border { get; init; } = string.Empty;
    public string Accent { get; init; } = string.Empty;
    public string OnAccentText { get; init; } = string.Empty;
    public string ButtonHover { get; init; } = string.Empty;
    public string StrongText { get; init; } = string.Empty;

    public override string ToString() => Name;
}
