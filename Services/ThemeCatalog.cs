using Amber.Models;

namespace Amber.Services;

/// <summary>The set of selectable themes, split into light and dark groups.</summary>
public static class ThemeCatalog
{
    public static IReadOnlyList<AppTheme> All { get; } = new List<AppTheme>
    {
        new()
        {
            Name = "Light Orange", IsDark = false,
            WindowBackground = "#FFF7E4CE", CardBackground = "#FFFBEEDD", ControlBackground = "#FFFDF5EC",
            PrimaryText = "#FF4A2F14", SecondaryText = "#FF9A6E3C", Border = "#FFEBCFA9",
            Accent = "#FFE8802A", OnAccentText = "#FFFDF5EC", ButtonHover = "#FFF3DBBE", StrongText = "#FF2E1C0A",
        },
        new()
        {
            Name = "Light Green", IsDark = false,
            WindowBackground = "#FFDDEEDD", CardBackground = "#FFE9F5E9", ControlBackground = "#FFF1F9F1",
            PrimaryText = "#FF163A22", SecondaryText = "#FF4C7A54", Border = "#FFC3E0C6",
            Accent = "#FF35A254", OnAccentText = "#FFF1F9F1", ButtonHover = "#FFD3EBD5", StrongText = "#FF0D2716",
        },
        new()
        {
            Name = "Light Violet", IsDark = false,
            WindowBackground = "#FFE9E1F6", CardBackground = "#FFF1ECFA", ControlBackground = "#FFF7F3FC",
            PrimaryText = "#FF2E1F4E", SecondaryText = "#FF6A5794", Border = "#FFDDD0F0",
            Accent = "#FF7A4FD0", OnAccentText = "#FFF7F3FC", ButtonHover = "#FFE4D9F5", StrongText = "#FF1F1339",
        },
        new()
        {
            Name = "Light Grey", IsDark = false,
            WindowBackground = "#FFEBECEE", CardBackground = "#FFF5F6F7", ControlBackground = "#FFFFFFFF",
            PrimaryText = "#FF24272B", SecondaryText = "#FF6B6F74", Border = "#FFD4D7DB",
            Accent = "#FF596471", OnAccentText = "#FFFFFFFF", ButtonHover = "#FFE3E5E8", StrongText = "#FF24272B",
        },
        new()
        {
            Name = "White", IsDark = false,
            WindowBackground = "#FFF7F8FA", CardBackground = "#FFFFFFFF", ControlBackground = "#FFFFFFFF",
            PrimaryText = "#FF1A1A1A", SecondaryText = "#FF666666", Border = "#FFE2E4E8",
            Accent = "#FF2D7FF9", OnAccentText = "#FFFFFFFF", ButtonHover = "#FFFDF0B5", StrongText = "#FF1A1A1A",
        },
        new()
        {
            Name = "Dark Orange", IsDark = true,
            WindowBackground = "#FF1A1206", CardBackground = "#FF241A0B", ControlBackground = "#FF2F2312",
            PrimaryText = "#FFE7D2B0", SecondaryText = "#FFB1926B", Border = "#FF493822",
            Accent = "#FFE8802A", OnAccentText = "#FF1A1206", ButtonHover = "#FF3C2C15", StrongText = "#FFF2E1C6",
        },
        new()
        {
            Name = "Dark Green", IsDark = true,
            WindowBackground = "#FF0D1A11", CardBackground = "#FF142317", ControlBackground = "#FF1C2E20",
            PrimaryText = "#FFC7E1CB", SecondaryText = "#FF87AC8C", Border = "#FF284A31",
            Accent = "#FF3AA655", OnAccentText = "#FF0D1A11", ButtonHover = "#FF1D3925", StrongText = "#FFDBEEDF",
        },
        new()
        {
            Name = "Dark Violet", IsDark = true,
            WindowBackground = "#FF14101E", CardBackground = "#FF1D172D", ControlBackground = "#FF271E3B",
            PrimaryText = "#FFD6CAEB", SecondaryText = "#FF9C8DBC", Border = "#FF3A2D57",
            Accent = "#FF7A4FD0", OnAccentText = "#FF14101E", ButtonHover = "#FF2D2247", StrongText = "#FFE6DDF6",
        },
        new()
        {
            Name = "Dark Grey", IsDark = true,
            WindowBackground = "#FF2B2E33", CardBackground = "#FF34383F", ControlBackground = "#FF3E434B",
            PrimaryText = "#FFB8BDC4", SecondaryText = "#FFB0B4BA", Border = "#FF4A4F57",
            Accent = "#FF6D8AC0", OnAccentText = "#FFFFFFFF", ButtonHover = "#FF565D69", StrongText = "#FFCAD0D6",
        },
        new()
        {
            Name = "Black", IsDark = true,
            WindowBackground = "#FF161719", CardBackground = "#FF1E2024", ControlBackground = "#FF26282E",
            PrimaryText = "#FFB4B9C0", SecondaryText = "#FF9AA0A8", Border = "#FF33363D",
            Accent = "#FF3D7EFF", OnAccentText = "#FFFFFFFF", ButtonHover = "#FF3A3E46", StrongText = "#FFC6CCD2",
        },
    };

    public static IReadOnlyList<string> LightNames { get; } =
        All.Where(t => !t.IsDark).Select(t => t.Name).ToList();

    public static IReadOnlyList<string> DarkNames { get; } =
        All.Where(t => t.IsDark).Select(t => t.Name).ToList();

    public static string NormalizeLightName(string? name)
    {
        string? normalized = name switch
        {
            "Amber" => "Light Grey",
            "Light" => "Light Grey",
            _ => name,
        };
        return LightNames.Contains(normalized) ? normalized! : "Light Grey";
    }

    public static string NormalizeDarkName(string? name)
    {
        string? normalized = name switch
        {
            "Amber Dark" => "Dark Grey",
            "Amber" => "Dark Grey",
            "Dark" => "Black",
            "Orange" => "Dark Orange",
            "Green" => "Dark Green",
            "Violet" => "Dark Violet",
            _ => name,
        };
        return DarkNames.Contains(normalized) ? normalized! : "Dark Grey";
    }

    public static AppTheme ByName(string? name) =>
        All.FirstOrDefault(t => t.Name == name) ?? All.First();
}
