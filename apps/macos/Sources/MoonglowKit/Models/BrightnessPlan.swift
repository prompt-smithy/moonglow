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

    public static func splitToUnified(
        hardware: Int,
        software: Int,
        hardwareAvailable: Bool,
        softwareAvailable: Bool,
        recoveryRegistered: Bool
    ) -> Int {
        if hardwareAvailable && softwareAvailable {
            if hardware > 1 {
                return 50 + Int((Double(hardware - 1) * 50.0 / 99.0).rounded())
            }
            if recoveryRegistered {
                return 1 + Int((Double(software - 1) * 49.0 / 99.0).rounded())
            } else {
                let clampedSw = max(software, AppSettings.softwareBrightnessFloor)
                return 1 + Int((Double(clampedSw - 50) * 48.0 / 49.0).rounded())
            }
        } else if hardwareAvailable {
            return AppSettings.clampBrightness(hardware)
        } else if softwareAvailable {
            return AppSettings.clampBrightness(software)
        } else {
            return 80
        }
    }
}
