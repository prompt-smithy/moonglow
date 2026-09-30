import Foundation

/// Pure DDC/CI packet construction, ported from `Services/MacDdcBrightnessService.cs`.
///
/// Only the byte layout lives here. Transport (the private `IOAVService*` calls) is
/// deliberately kept out so the packet format can be asserted in tests.
public enum DDCPacket {

    /// DDC/CI chip address.
    public static let chipAddress: UInt32 = 0x37
    /// DDC/CI data address.
    public static let dataAddress: UInt32 = 0x51
    /// Delay before each I2C transaction. The MCCS protocol needs settling time.
    public static let writeDelay: TimeInterval = 0.05
    /// Coalescing window for slider-driven writes.
    public static let debounceDelay: TimeInterval = 0.04
    /// Max attempts per transaction.
    public static let maxAttempts = 2

    private static let checksumSeed: UInt8 = 0x6E

    /// MCCS luminance (`VCP 0x10`) query sent before the first write.
    public static let probeRequest: [UInt8] = [0x82, 0x01, 0x10, 0xFD]

    /// Builds the six-byte MCCS luminance write packet, clamping the value to `1...100`
    /// and computing the XOR checksum.
    public static func buildWritePacket(percent: Int) -> [UInt8] {
        let value = UInt16(AppSettings.clampBrightness(percent))
        var packet: [UInt8] = [
            0x84,
            0x03,
            0x10,
            UInt8((value >> 8) & 0xFF),
            UInt8(value & 0xFF),
            0,
        ]

        packet[5] = checksum(of: packet)
        return packet
    }

    /// Builds the IOKit Intel transaction payload, including the DDC data address.
    public static func buildIntelWritePayload(percent: Int) -> [UInt8] {
        [UInt8(dataAddress)] + buildWritePacket(percent: percent)
    }

    /// Intel IOKit reads include the DDC data address in the outgoing I2C payload.
    public static var intelProbePayload: [UInt8] {
        var payload: [UInt8] = [UInt8(dataAddress), 0x82, 0x01, 0x10, 0]
        payload[4] = checksum(of: Array(payload.dropFirst()))
        return payload
    }

    /// XOR checksum over the seed, the data address, and the first five payload bytes.
    public static func checksum(of packet: [UInt8]) -> UInt8 {
        var value = checksumSeed ^ UInt8(dataAddress)
        for byte in packet.prefix(5) {
            value ^= byte
        }
        return value
    }
}
