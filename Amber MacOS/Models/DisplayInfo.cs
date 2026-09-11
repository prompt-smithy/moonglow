namespace Amber.MacOS.Models;

public sealed record DisplayInfo(
    uint NativeId,
    string Name,
    string Identifier,
    bool IsPrimary,
    bool SupportsBrightness,
    bool SupportsColorTemperature)
{
    public string CapabilitySummary
    {
        get
        {
            var capabilities = new List<string>();
            if (SupportsBrightness)
                capabilities.Add("brightness");
            if (SupportsColorTemperature)
                capabilities.Add("color temperature");
            if (capabilities.Count == 0)
                return "No compatible controls";

            return $"{(IsPrimary ? "Primary, " : string.Empty)}{string.Join(" and ", capabilities)}";
        }
    }
}