using Amber.MacOS.Models;

namespace Amber.MacOS.Services;

public static class ThemeCatalog
{
    public static IReadOnlyList<AppTheme> Light { get; } =
    [
        new("Light Amber", false, "#FFF8E1", "#FFFBF2", "#FFFFFF", "#2B2110", "#6D5A2F", "#FFD978", "#FFB300", "#241A00", "#FFECB3", "#1F1708"),
        new("Light Orange", false, "#F7E4CE", "#FBEEDD", "#FDF5EC", "#4A2F14", "#9A6E3C", "#EBCFA9", "#E8802A", "#FDF5EC", "#F3DBBE", "#2E1C0A"),
        new("Light Green", false, "#DDEEDD", "#E9F5E9", "#F1F9F1", "#163A22", "#4C7A54", "#C3E0C6", "#35A254", "#F1F9F1", "#D3EBD5", "#0D2716"),
        new("Light Violet", false, "#E9E1F6", "#F1ECFA", "#F7F3FC", "#2E1F4E", "#6A5794", "#DDD0F0", "#7A4FD0", "#F7F3FC", "#E4D9F5", "#1F1339"),
        new("Light Grey", false, "#EBECEE", "#F5F6F7", "#FFFFFF", "#24272B", "#6B6F74", "#D4D7DB", "#596471", "#FFFFFF", "#E3E5E8", "#24272B"),
        new("White", false, "#F7F8FA", "#FFFFFF", "#FFFFFF", "#1A1A1A", "#666666", "#E2E4E8", "#2D7FF9", "#FFFFFF", "#FDF0B5", "#1A1A1A"),
    ];

    public static IReadOnlyList<AppTheme> Dark { get; } =
    [
        new("Dark Amber", true, "#18150D", "#211D12", "#2A2517", "#F5E7C1", "#C1AA72", "#4A3E20", "#FFC107", "#211800", "#393019", "#FFF3D1"),
        new("Dark Orange", true, "#1A1206", "#241A0B", "#2F2312", "#E7D2B0", "#B1926B", "#493822", "#E8802A", "#1A1206", "#3C2C15", "#F2E1C6"),
        new("Dark Green", true, "#0D1A11", "#142317", "#1C2E20", "#C7E1CB", "#87AC8C", "#284A31", "#3AA655", "#0D1A11", "#1D3925", "#DBEEDF"),
        new("Dark Violet", true, "#14101E", "#1D172D", "#271E3B", "#D6CAEB", "#9C8DBC", "#3A2D57", "#7A4FD0", "#14101E", "#2D2247", "#E6DDF6"),
        new("Dark Grey", true, "#2B2E33", "#34383F", "#3E434B", "#D8DDE4", "#B0B4BA", "#4A4F57", "#6D8AC0", "#FFFFFF", "#565D69", "#E2E8EE"),
        new("Black", true, "#101112", "#18191C", "#222428", "#E4E7EB", "#A4AAB2", "#34373D", "#3D7EFF", "#FFFFFF", "#30343A", "#F2F4F7"),
    ];

    public static IReadOnlyList<string> LightNames { get; } = Light.Select(theme => theme.Name).ToArray();
    public static IReadOnlyList<string> DarkNames { get; } = Dark.Select(theme => theme.Name).ToArray();

    public static AppTheme LightByName(string? name) =>
        Light.FirstOrDefault(theme => theme.Name == name) ?? Light[0];

    public static AppTheme DarkByName(string? name) =>
        Dark.FirstOrDefault(theme => theme.Name == name) ?? Dark[0];
}