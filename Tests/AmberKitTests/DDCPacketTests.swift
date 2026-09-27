import Testing
@testable import AmberKit

/// Ported from `DdcBrightnessTests` in the .NET test project.
@Suite("DDC packet")
struct DDCPacketTests {

    @Test(
        "The write packet clamps the value and computes the checksum",
        arguments: [(-10, 0x01, 0xA9), (0, 0x01, 0xA9), (1, 0x01, 0xA9), (50, 0x32, 0x9A), (100, 0x64, 0xCC), (150, 0x64, 0xCC)]
    )
    func writePacketClampsValueAndComputesChecksum(percent: Int, expectedValue: UInt8, expectedChecksum: UInt8) {
        let packet = DDCPacket.buildWritePacket(percent: percent)

        #expect(packet.count == 6)
        #expect(packet[0] == 0x84)
        #expect(packet[1] == 0x03)
        #expect(packet[2] == 0x10)
        #expect(packet[3] == 0x00)
        #expect(packet[4] == expectedValue)
        #expect(packet[5] == expectedChecksum)
    }

    @Test("The checksum matches an independent XOR of the payload")
    func checksumMatchesIndependentXor() {
        let packet = DDCPacket.buildWritePacket(percent: 73)

        let expected = 0x6E ^ UInt8(DDCPacket.dataAddress)
            ^ packet[0] ^ packet[1] ^ packet[2] ^ packet[3] ^ packet[4]

        #expect(packet[5] == expected)
    }

    @Test("Addresses match the MCCS luminance convention")
    func addressesMatchMccsConvention() {
        #expect(DDCPacket.chipAddress == 0x37)
        #expect(DDCPacket.dataAddress == 0x51)
        #expect(DDCPacket.probeRequest == [0x82, 0x01, 0x10, 0xFD])
    }

    @Test("The probe request is a four-byte luminance query")
    func probeRequestShape() {
        // 0x82 = get VCP, 0x01 = length, 0x10 = luminance, 0xFD = checksum seed for reads
        #expect(DDCPacket.probeRequest.count == 4)
        #expect(DDCPacket.probeRequest[2] == 0x10)
    }
}
