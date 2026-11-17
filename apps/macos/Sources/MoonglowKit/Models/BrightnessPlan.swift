import Foundation

/// Pure routing calculations for dual hardware and software brightness.
public struct BrightnessPlan: Sendable {
    public struct RoutingOutput: Equatable, Sendable {
        public let hardware: Int?
        public let software: Int

        public init(hardware: Int?, software: Int) {
            self.hardware = hardware
            self.software = software
        }
    }

    public struct SplitOutput: Equatable, Sendable {
        public let hardware: Int
        public let software: Int

        public init(hardware: Int, software: Int) {
            self.hardware = hardware
            self.software = software
        }
    }

    public static func computeRouting(
        master: Int,
        split: Bool,
        manualHardware: Int,
        manualSoftware: Int,
        hardwareAvailable: Bool,
        softwareAvailable: Bool,
        recoveryRegistered: Bool
    ) -> RoutingOutput {
        let floor = recoveryRegistered ? 1 : AppSettings.softwareBrightnessFloor

        if split {
            let hw = hardwareAvailable ? AppSettings.clampBrightness(manualHardware) : nil
            let sw = softwareAvailable ? max(AppSettings.clampBrightness(manualSoftware), floor) : 100
            return RoutingOutput(hardware: hw, software: sw)
        }

        let clampedMaster = AppSettings.clampBrightness(master)
        if hardwareAvailable && softwareAvailable {
            if clampedMaster >= 50 {
                let hw = 1 + Int((Double(clampedMaster - 50) * 99.0 / 50.0).rounded())
                return RoutingOutput(hardware: hw, software: 100)
            } else {
                let sw: Int
                if floor == 1 {
                    sw = 1 + Int((Double(clampedMaster - 1) * 99.0 / 49.0).rounded())
                } else {
                    sw = 50 + Int((Double(clampedMaster - 1) * 49.0 / 48.0).rounded())
                }
                return RoutingOutput(hardware: 1, software: sw)
            }
        } else if hardwareAvailable {
            return RoutingOutput(hardware: clampedMaster, software: 100)
        } else if softwareAvailable {
            return RoutingOutput(hardware: nil, software: max(clampedMaster, floor))
        } else {
            return RoutingOutput(hardware: nil, software: 100)
        }
    }

    public static func unifiedToSplit(
        master: Int,
        hardwareAvailable: Bool,
        softwareAvailable: Bool,
        recoveryRegistered: Bool
    ) -> SplitOutput {
        let routing = computeRouting(
            master: master,
            split: false,
            manualHardware: 80,
            manualSoftware: 100,
            hardwareAvailable: hardwareAvailable,
            softwareAvailable: softwareAvailable,
            recoveryRegistered: recoveryRegistered
        )
        let hw = routing.hardware ?? 80
        let sw = routing.software
        return SplitOutput(hardware: hw, software: sw)
    }

    /// Re-expresses an applied output after the usable engines or floor change.
    /// The new safety floor still takes precedence over continuity.
    public static func masterPreservingOutput(_ output: RoutingOutput, hardwareAvailable: Bool, softwareAvailable: Bool, recoveryRegistered: Bool) -> Int {
        let hardware = output.hardware ?? 100
        if hardwareAvailable && softwareAvailable {
            return splitToUnified(hardware: hardware, software: output.software, hardwareAvailable: true, softwareAvailable: true, recoveryRegistered: recoveryRegistered)
        }
        let effective = Int((Double(hardware * output.software) / 100.0).rounded())
        return max(AppSettings.clampBrightness(effective), softwareAvailable && !recoveryRegistered ? AppSettings.softwareBrightnessFloor : 1)
    }

    public static func splitToUnified(
        hardware: Int,
        software: Int,
        hardwareAvailable: Bool,
        softwareAvailable: Bool,
        recoveryRegistered: Bool
    ) -> Int {
        if hardwareAvailable && softwareAvailable {
            let hw = AppSettings.clampBrightness(hardware)
            let sw = max(AppSettings.clampBrightness(software), recoveryRegistered ? 1 : AppSettings.softwareBrightnessFloor)
            let effective = Double(hw * sw) / 100.0
            if effective > 1 {
                return 50 + Int(((effective - 1) * 50.0 / 99.0).rounded())
            }
            let shade = max(Int((effective * 100).rounded()), recoveryRegistered ? 1 : AppSettings.softwareBrightnessFloor)
            if shade >= 100 { return 50 }
            if recoveryRegistered {
                return 1 + Int((Double(shade - 1) * 49.0 / 99.0).rounded())
            }
            return 1 + Int((Double(shade - 50) * 48.0 / 49.0).rounded())
        } else if hardwareAvailable {
            return AppSettings.clampBrightness(hardware)
        } else if softwareAvailable {
            return AppSettings.clampBrightness(software)
        } else {
            return 80
        }
    }
}
