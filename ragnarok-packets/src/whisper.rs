//! Whisper packets for Hercules PACKETVER 20250716.
//!
//! `CZ_WHISPER` `0x0096` is the client send layout from
//! `packet_whisper_message` (24-byte name plus a NUL-terminated message).
//!
//! `ZC_WHISPER` `0x0097` is the layout Hercules documents for `PACKETVER >=
//! 20091104` (24-byte sender, `int32` `isAdmin`, message). Live MAIN 20250716
//! actually emits `0x09DE` (`senderGID`, 24-byte name, `uint8` `isAdmin`)
//! because `PACKETVER_MAIN_NUM >= 20131204`. Both are accepted.
//!
//! `ZC_ACK_WHISPER` is `0x0098` (result byte) before 20131223 and `0x09DF`
//! (result byte plus account id, 7 bytes) afterwards. MAIN 20250716 sends
//! `0x09DF`.
#[cfg(not(feature = "derive"))]
use ragnarok_macros::{ClientPacket, MapServer, Packet, ServerPacket};

use crate::*;

pub const WHISPER_NAME_LENGTH: usize = 24;

/// `CZ_WHISPER` (`0x0096`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0096)]
#[variable_length]
pub struct WhisperMessagePacket {
    #[length(24)]
    pub target: String,
    #[length_remaining]
    pub message: String,
}

/// `ZC_WHISPER` (`0x0097`) with `isAdmin`.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0097)]
#[variable_length]
pub struct WhisperReceivedPacket {
    #[length(24)]
    pub sender: String,
    pub is_admin: i32,
    #[length_remaining]
    pub message: String,
}

/// `ZC_WHISPER` (`0x09DE`) used by Hercules MAIN since 20131204, including
/// 20250716.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x09DE)]
#[variable_length]
pub struct WhisperReceived2Packet {
    pub sender_id: u32,
    #[length(24)]
    pub sender: String,
    pub is_admin: u8,
    #[length_remaining]
    pub message: String,
}

/// `ZC_ACK_WHISPER` (`0x0098`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0098)]
pub struct WhisperAckPacket {
    pub result: u8,
}

/// `ZC_ACK_WHISPER` (`0x09DF`) for PACKETVER >= 20131223.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x09DF)]
pub struct WhisperAck2Packet {
    pub result: u8,
    pub account_id: u32,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;

    fn padded_name(name: &str) -> [u8; WHISPER_NAME_LENGTH] {
        let mut bytes = [0u8; WHISPER_NAME_LENGTH];
        let raw = name.as_bytes();
        bytes[..raw.len()].copy_from_slice(raw);
        bytes
    }

    #[test]
    fn cz_whisper_is_variable_with_a_24_byte_name() {
        let packet = WhisperMessagePacket::new("Alice".to_string(), "hello".to_string());
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.into_inner();

        let mut expected = vec![0x96, 0x00, 0, 0];
        expected.extend_from_slice(&padded_name("Alice"));
        expected.extend_from_slice(b"hello\0");
        let length = expected.len() as u16;
        expected[2..4].copy_from_slice(&length.to_le_bytes());

        assert_eq!(bytes, expected);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), 2 + 2 + 24 + 6);

        let mut reader = ByteReader::without_metadata(&bytes);
        let parsed = WhisperMessagePacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.target, "Alice");
        assert_eq!(parsed.message, "hello");
    }

    #[test]
    fn zc_whisper_0097_includes_is_admin() {
        let mut bytes = vec![0x97, 0x00, 0, 0];
        bytes.extend_from_slice(&padded_name("Bob"));
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(b"hey\0");
        let length = bytes.len() as u16;
        bytes[2..4].copy_from_slice(&length.to_le_bytes());

        let mut reader = ByteReader::without_metadata(&bytes);
        let parsed = WhisperReceivedPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.sender, "Bob");
        assert_eq!(parsed.is_admin, 1);
        assert_eq!(parsed.message, "hey");
        assert_eq!(bytes.len(), 2 + 2 + 24 + 4 + 4);
    }

    #[test]
    fn zc_whisper_09de_matches_hercules_20250716() {
        let mut bytes = vec![0xDE, 0x09, 0, 0];
        bytes.extend_from_slice(&77u32.to_le_bytes());
        bytes.extend_from_slice(&padded_name("Cara"));
        bytes.push(0);
        bytes.extend_from_slice(b"ping\0");
        let length = bytes.len() as u16;
        bytes[2..4].copy_from_slice(&length.to_le_bytes());

        let mut reader = ByteReader::without_metadata(&bytes);
        let parsed = WhisperReceived2Packet::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.sender_id, 77);
        assert_eq!(parsed.sender, "Cara");
        assert_eq!(parsed.is_admin, 0);
        assert_eq!(parsed.message, "ping");
    }

    #[test]
    fn whisper_ack_results_match_both_headers() {
        let legacy = [0x98, 0x00, 2];
        let mut reader = ByteReader::without_metadata(&legacy);
        let parsed = WhisperAckPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.result, 2);

        let mut modern = vec![0xDF, 0x09, 1];
        modern.extend_from_slice(&2001u32.to_le_bytes());
        assert_eq!(modern.len(), 7);
        let mut reader = ByteReader::without_metadata(&modern);
        let parsed = WhisperAck2Packet::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(parsed.result, 1);
        assert_eq!(parsed.account_id, 2001);
    }
}
