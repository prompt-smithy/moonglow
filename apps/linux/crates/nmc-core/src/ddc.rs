//! Pure DDC/CI packet construction, ported from `Services/DDCPacket.swift`.
//!
//! Only the byte layout lives here. Transport (Linux `/dev/i2c-*` via `i2cdev`)
//! is deliberately kept out so the packet format can be asserted in tests.

use crate::settings::AppSettings;
use std::time::Duration;

/// DDC/CI chip address (7-bit form of `0x6E`).
pub const CHIP_ADDRESS: u16 = 0x37;
/// DDC/CI data address (source address in the payload).
pub const DATA_ADDRESS: u8 = 0x51;
/// Delay before each I2C transaction. The MCCS protocol needs settling time.
pub const WRITE_DELAY: Duration = Duration::from_millis(50);
/// Coalescing window for slider-driven writes.
pub const DEBOUNCE_DELAY: Duration = Duration::from_millis(40);
/// Max attempts per transaction.
pub const MAX_ATTEMPTS: u32 = 2;

const CHECKSUM_SEED: u8 = 0x6E;

/// MCCS luminance (`VCP 0x10`) query sent before the first write.
pub const PROBE_REQUEST: [u8; 4] = [0x82, 0x01, 0x10, 0xFD];

/// Builds the six-byte MCCS luminance write packet, clamping the value to
/// `1...100` and computing the XOR checksum.
pub fn build_write_packet(percent: i32) -> [u8; 6] {
    let value = AppSettings::clamp_brightness(percent) as u16;
    let mut packet = [
        0x84,
        0x03,
        0x10,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
        0,
    ];
    packet[5] = checksum(&packet);
    packet
}

/// Builds the Linux `/dev/i2c-*` transaction payload, including the DDC source
/// address. Same shape as the macOS Intel IOKit payload: I2C transport on both
/// platforms prefixes the packet with the data address.
pub fn build_linux_write_payload(percent: i32) -> [u8; 7] {
    let packet = build_write_packet(percent);
    [
        DATA_ADDRESS,
        packet[0],
        packet[1],
        packet[2],
        packet[3],
        packet[4],
        packet[5],
    ]
}

/// Linux I2C reads include the DDC source address in the outgoing payload.
pub fn linux_probe_payload() -> [u8; 5] {
    let mut payload = [DATA_ADDRESS, 0x82, 0x01, 0x10, 0];
    payload[4] = checksum(&payload[1..]);
    payload
}

/// XOR checksum over the seed, the data address, and the first five payload bytes.
pub fn checksum(packet: &[u8]) -> u8 {
    let mut value = CHECKSUM_SEED ^ DATA_ADDRESS;
    for byte in packet.iter().take(5) {
        value ^= byte;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_packet_clamps_value_and_computes_checksum() {
        for (percent, expected_value, expected_checksum) in [
            (-10, 0x01, 0xA9),
            (0, 0x01, 0xA9),
            (1, 0x01, 0xA9),
            (50, 0x32, 0x9A),
            (100, 0x64, 0xCC),
            (150, 0x64, 0xCC),
        ] {
            let packet = build_write_packet(percent);
            assert_eq!(packet.len(), 6);
            assert_eq!(packet[0], 0x84);
            assert_eq!(packet[1], 0x03);
            assert_eq!(packet[2], 0x10);
            assert_eq!(packet[3], 0x00);
            assert_eq!(packet[4], expected_value, "percent={percent}");
            assert_eq!(packet[5], expected_checksum, "percent={percent}");
        }
    }

    #[test]
    fn checksum_matches_independent_xor() {
        let packet = build_write_packet(73);
        let expected =
            0x6E ^ DATA_ADDRESS ^ packet[0] ^ packet[1] ^ packet[2] ^ packet[3] ^ packet[4];
        assert_eq!(packet[5], expected);
    }

    #[test]
    fn addresses_match_mccs_convention() {
        assert_eq!(CHIP_ADDRESS, 0x37);
        assert_eq!(DATA_ADDRESS, 0x51);
        assert_eq!(PROBE_REQUEST, [0x82, 0x01, 0x10, 0xFD]);
    }

    #[test]
    fn linux_payloads_include_address_and_checksum() {
        let write = build_linux_write_payload(50);
        let probe = linux_probe_payload();
        assert_eq!(write[0], DATA_ADDRESS);
        assert_eq!(&write[1..], &build_write_packet(50));
        assert_eq!(probe, [0x51, 0x82, 0x01, 0x10, 0xAC]);
    }
}
