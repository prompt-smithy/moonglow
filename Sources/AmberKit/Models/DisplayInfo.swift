import Foundation

/// One active display and the controls Amber believes it can drive.
///
/// Ported from `Models/DisplayInfo.cs`. `nativeId` is the CoreGraphics display ID.
public struct DisplayInfo: Identifiable, Equatable, Sendable {
    public let nativeId: UInt32
    public let name: String
    public let identifier: String
    public let isPrimary: Bool
    public let supportsBrightness: Bool
    public let supportsColorTemperature: Bool

    public var id: UInt32 { nativeId }

    public init(
        nativeId: UInt32,
        name: String,
        identifier: String,
        isPrimary: Bool,
        supportsBrightness: Bool,
        supportsColorTemperature: Bool
    ) {
        self.nativeId = nativeId
        self.name = name
        self.identifier = identifier
        self.isPrimary = isPrimary
        self.supportsBrightness = supportsBrightness
        self.supportsColorTemperature = supportsColorTemperature
    }

    /// Human-readable capability line shown under the display name.
    public var capabilitySummary: String {
        var capabilities: [String] = []
        if supportsBrightness { capabilities.append("brightness") }
        if supportsColorTemperature { capabilities.append("color temperature") }

        guard !capabilities.isEmpty else { return "No compatible controls" }

        let prefix = isPrimary ? "Primary, " : ""
        return prefix + capabilities.joined(separator: " and ")
    }
}
