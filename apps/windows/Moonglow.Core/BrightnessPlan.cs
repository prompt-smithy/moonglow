namespace Moonglow.Core;

/// <summary>Pure routing calculations for dual hardware and software brightness.</summary>
public static class BrightnessPlan
{
    public readonly record struct RoutingOutput(int? Hardware, int Software);
    public readonly record struct SplitOutput(int Hardware, int Software);

    public static RoutingOutput ComputeRouting(
        int master,
        bool split,
        int manualHardware,
        int manualSoftware,
        bool hardwareAvailable,
        bool softwareAvailable,
        bool recoveryRegistered)
    {
        int floor = recoveryRegistered ? 1 : AppSettings.SoftwareBrightnessFloor;

        if (split)
        {
            int? hw = hardwareAvailable ? AppSettings.ClampBrightness(manualHardware) : null;
            int sw = softwareAvailable ? Math.Max(AppSettings.ClampBrightness(manualSoftware), floor) : 100;
            return new RoutingOutput(hw, sw);
        }

        int clampedMaster = AppSettings.ClampBrightness(master);
        if (hardwareAvailable && softwareAvailable)
        {
            if (clampedMaster >= 50)
            {
                int hw = 1 + (int)Math.Round((clampedMaster - 50) * 99.0 / 50.0, MidpointRounding.AwayFromZero);
                return new RoutingOutput(hw, 100);
            }
            else
            {
                int sw;
                if (floor == 1)
                {
                    sw = 1 + (int)Math.Round((clampedMaster - 1) * 99.0 / 49.0, MidpointRounding.AwayFromZero);
                }
                else
                {
                    sw = 50 + (int)Math.Round((clampedMaster - 1) * 49.0 / 48.0, MidpointRounding.AwayFromZero);
                }
                return new RoutingOutput(1, sw);
            }
        }
        else if (hardwareAvailable)
        {
            return new RoutingOutput(clampedMaster, 100);
        }
        else if (softwareAvailable)
        {
            return new RoutingOutput(null, Math.Max(clampedMaster, floor));
        }
        else
        {
            return new RoutingOutput(null, 100);
        }
    }

    public static SplitOutput UnifiedToSplit(
        int master,
        bool hardwareAvailable,
        bool softwareAvailable,
        bool recoveryRegistered)
    {
        var routing = ComputeRouting(
            master,
            split: false,
            manualHardware: 80,
            manualSoftware: 100,
            hardwareAvailable: hardwareAvailable,
            softwareAvailable: softwareAvailable,
            recoveryRegistered: recoveryRegistered);
        int hw = routing.Hardware ?? 80;
        int sw = routing.Software;
        return new SplitOutput(hw, sw);
    }

    public static int SplitToUnified(
        int hardware,
        int software,
        bool hardwareAvailable,
        bool softwareAvailable,
        bool recoveryRegistered)
    {
        if (hardwareAvailable && softwareAvailable)
        {
            if (hardware > 1)
            {
                return 50 + (int)Math.Round((hardware - 1) * 50.0 / 99.0, MidpointRounding.AwayFromZero);
            }
            if (recoveryRegistered)
            {
                return 1 + (int)Math.Round((software - 1) * 49.0 / 99.0, MidpointRounding.AwayFromZero);
            }
            else
            {
                int clampedSw = Math.Max(software, AppSettings.SoftwareBrightnessFloor);
                return 1 + (int)Math.Round((clampedSw - 50) * 48.0 / 49.0, MidpointRounding.AwayFromZero);
            }
        }
        else if (hardwareAvailable)
        {
            return AppSettings.ClampBrightness(hardware);
        }
        else if (softwareAvailable)
        {
            return AppSettings.ClampBrightness(software);
        }
        else
        {
            return 80;
        }
    }
}
