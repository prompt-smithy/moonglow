import Testing
@testable import MoonglowKit

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
        #expect(DDCPacket.probeRequest == [0x82, 0x01, 0x10, 0xAC])
    }

    @Test("The probe request is a four-byte luminance query")
    func probeRequestShape() {
        // 0x82 = get VCP, 0x01 = length, 0x10 = luminance, last byte = checksum
        #expect(DDCPacket.probeRequest.count == 4)
        #expect(DDCPacket.probeRequest[2] == 0x10)
    }

    @Test("The probe request checksum covers the destination and source bytes")
    func probeRequestChecksumCoversWireAddresses() {
        // IOAVServiceWriteI2C takes the destination (0x6E) as the chip address and the
        // source (0x51) as the offset, so the buffer starts at the length byte but the
        // checksum still covers the two omitted bytes — same as a luminance write.
        let body = Array(DDCPacket.probeRequest.dropLast())
        #expect(DDCPacket.probeRequest.last == DDCPacket.checksum(of: body))
    }

    @Test("The luminance query carries the same checksum on both transports")
    func probeChecksumMatchesIntelPayload() {
        // The Intel payload spells out the source byte the Apple Silicon driver takes as
        // the offset parameter; the query on the wire — and its checksum — is identical.
        #expect(DDCPacket.probeRequest.last == DDCPacket.intelProbePayload.last)
    }

    @Test("Intel I2C payloads include the data address and valid checksum")
    func intelPayloadsIncludeAddressAndChecksum() {
        let write = DDCPacket.buildIntelWritePayload(percent: 50)
        let probe = DDCPacket.intelProbePayload

        #expect(write.first == UInt8(DDCPacket.dataAddress))
        #expect(Array(write.dropFirst()) == DDCPacket.buildWritePacket(percent: 50))
        #expect(probe == [0x51, 0x82, 0x01, 0x10, 0xAC])
    }

    @Test("A well-formed GetVCP reply validates the luminance probe")
    func wellFormedProbeReplyIsValid() {
        // current = 50, max = 100. Second byte is the 0x80|8 length byte the driver returns.
        let reply: [UInt8] = [0x6E, 0x88, 0x02, 0x00, 0x10, 0x00, 0x00, 0x64, 0x00, 0x32, 0xF2]

        #expect(reply.count == DDCPacket.probeReplyLength)
        #expect(DDCPacket.isValidProbeReply(reply))
    }

    @Test(
        "A reply that is not a successful luminance answer fails the probe",
        arguments: [
            [UInt8](repeating: 0, count: 11), // transport ok, panel silent
            [0x00, 0x82, 0x01, 0x10, 0xFD, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // request echo, panel silent
            [UInt8](repeating: 0, count: 12), // stale oversized read
            [0x6E, 0x88, 0x02, 0x00, 0x10, 0x00, 0x00, 0x64, 0x00, 0x32, 0x00], // bad checksum
            [0x6E, 0x88, 0x03, 0x00, 0x10, 0x00, 0x00, 0x64, 0x00, 0x32, 0xF2], // not a GetVCP response
            [0x6E, 0x88, 0x02, 0x01, 0x10, 0x00, 0x00, 0x64, 0x00, 0x32, 0xF2], // error result
            [0x6E, 0x88, 0x02, 0x00, 0x12, 0x00, 0x00, 0x64, 0x00, 0x32, 0xF2], // wrong VCP echoed
            [0x6E, 0x88, 0x02, 0x00, 0x10, 0x00, 0x00, 0x64, 0x00], // truncated
        ]
    )
    func malformedProbeReplyIsInvalid(reply: [UInt8]) {
        #expect(!DDCPacket.isValidProbeReply(reply))
    }
}
