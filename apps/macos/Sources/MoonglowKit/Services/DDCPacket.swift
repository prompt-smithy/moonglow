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
    /// Max probe rounds before the panel is declared non-DDC.
    ///
    /// A capable panel may drop the first transactions while its DDC bridge wakes up; one
    /// round is the common success path and the rest only cost time when answering late.
    public static let probeRounds = 3

    private static let checksumSeed: UInt8 = 0x6E

    /// MCCS luminance (`VCP 0x10`) query sent before the first write.
    ///
    /// The driver takes the destination (`0x6E`) as the chip address and the source (`0x51`)
    /// as the offset, so the buffer starts at the length byte — but the checksum still covers
    /// the two omitted bytes, exactly like a luminance write. Derived rather than literal so
    /// the query and the Intel payload cannot drift apart again.
    public static var probeRequest: [UInt8] {
        let body: [UInt8] = [0x82, 0x01, 0x10]
        return body + [checksum(of: body)]
    }

    /// Expected length of the GetVCP reply to ``probeRequest``.
    public static let probeReplyLength = 11

    /// Validates a GetVCP reply to the luminance probe.
    ///
    /// A completed I2C read is not proof the panel speaks DDC/CI — the transport can succeed
    /// while the monitor never answers, which must fall back to software dimming rather than
    /// claim hardware brightness. The reply is accepted only when it has the GetVCP-response
    /// shape (`0x02`), a success result (`0x00`), echoes the luminance VCP (`0x10`), and carries
    /// a valid XOR checksum seeded with the host address (`0x50`).
    public static func isValidProbeReply(_ reply: [UInt8]) -> Bool {
        guard reply.count == probeReplyLength,
              reply[2] == 0x02,
              reply[3] == 0x00,
              reply[4] == 0x10
        else {
            return false
        }

        let expectedChecksum = reply.prefix(probeReplyLength - 1).reduce(UInt8(0x50)) { $0 ^ $1 }
        return reply[probeReplyLength - 1] == expectedChecksum
    }

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
