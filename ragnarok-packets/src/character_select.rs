//! Timed character deletion for packetver 20250716.
//!
//! Hercules `char_parse_char_delete2_*` (`src/char/char.c`):
//! `0x0827` reserves deletion (character id only), `0x0828` answers with the
//! character id, a result, and the remaining seconds. `0x0829` confirms with
//! exactly six bytes (official clients send YYMMDD; this TEST server compares
//! those bytes to `delete`). `0x082A` is the confirm result. `0x082B` cancels
//! and `0x082C` answers. Sizes match `framing_20250716`.

use ragnarok_macros::{CharacterServer, ClientPacket, Packet, ServerPacket};

use crate::CharacterId;

/// `CH_DELETE_CHAR3_RESERVED` (`0x0827`). Queue a character for deletion.
#[derive(Debug, Clone, Packet, ClientPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0827)]
pub struct ReserveDeleteCharacterPacket {
    pub character_id: CharacterId,
}

/// `HC_DELETE_CHAR3_RESERVED` (`0x0828`).
///
/// `remaining_seconds` is `delete_date - now` on packetver >= 20130000.
/// Result `1` means the reservation was stored.
#[derive(Debug, Clone, Packet, ServerPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0828)]
pub struct ReserveDeleteCharacterResultPacket {
    pub character_id: CharacterId,
    pub result: u32,
    pub remaining_seconds: u32,
}

/// `CH_DELETE_CHAR3` (`0x0829`). Six confirmation bytes, not a padded string.
#[derive(Debug, Clone, Packet, ClientPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0829)]
pub struct ConfirmDeleteCharacterPacket {
    pub character_id: CharacterId,
    pub confirmation: [u8; 6],
}

/// `HC_DELETE_CHAR3` (`0x082A`).
#[derive(Debug, Clone, Packet, ServerPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x082A)]
pub struct ConfirmDeleteCharacterResultPacket {
    pub character_id: CharacterId,
    pub result: u32,
}

/// `CH_DELETE_CHAR3_CANCEL` (`0x082B`).
#[derive(Debug, Clone, Packet, ClientPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x082B)]
pub struct CancelDeleteCharacterPacket {
    pub character_id: CharacterId,
}

/// `HC_DELETE_CHAR3_CANCEL` (`0x082C`).
#[derive(Debug, Clone, Packet, ServerPacket, CharacterServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x082C)]
pub struct CancelDeleteCharacterResultPacket {
    pub character_id: CharacterId,
    pub result: u32,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;
    use crate::{CreateCharacterPacket, JobId, PacketExt, Sex};

    #[test]
    fn create_character_lays_out_20250716_fields() {
        let packet = CreateCharacterPacket::new("Test".to_owned(), 2, 3, 5, JobId(4218), Sex::Female);
        let mut writer = ByteWriter::new();
        assert_eq!(packet.packet_to_bytes(&mut writer).unwrap(), 36);
        let bytes = writer.as_slice();
        assert_eq!(&bytes[0..2], &[0x39, 0x0A]);
        assert_eq!(&bytes[2..6], b"Test");
        assert!(bytes[6..26].iter().all(|byte| *byte == 0), "name is 24 bytes");
        assert_eq!(bytes[26], 2, "slot");
        assert_eq!(&bytes[27..29], &3u16.to_le_bytes(), "hair color");
        assert_eq!(&bytes[29..31], &5u16.to_le_bytes(), "hair style");
        assert_eq!(&bytes[31..33], &4218u16.to_le_bytes(), "start job");
        assert_eq!(&bytes[33..35], &[0, 0], "unknown bytes stay zero");
        assert_eq!(bytes[35], 0, "female");
    }

    #[test]
    fn create_character_male_is_one() {
        let packet = CreateCharacterPacket::new("Nova".to_owned(), 0, 0, 1, JobId(0), Sex::Male);
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        let bytes = writer.as_slice();
        assert_eq!(bytes.len(), 36);
        assert_eq!(&bytes[31..33], &0u16.to_le_bytes());
        assert_eq!(bytes[35], 1);
    }

    #[test]
    fn reserve_delete_is_character_id_only() {
        let packet = ReserveDeleteCharacterPacket::new(CharacterId(0x0403_0201));
        let mut writer = ByteWriter::new();
        assert_eq!(packet.packet_to_bytes(&mut writer).unwrap(), 6);
        assert_eq!(writer.as_slice(), &[0x27, 0x08, 0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn reserve_result_is_id_result_and_remaining_seconds() {
        let bytes = [0x28, 0x08, 0x01, 0x02, 0x03, 0x04, 0x01, 0x00, 0x00, 0x00, 0x80, 0x51, 0x01, 0x00];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = ReserveDeleteCharacterResultPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.character_id, CharacterId(0x0403_0201));
        assert_eq!(packet.result, 1);
        assert_eq!(packet.remaining_seconds, 86_400);
        assert_eq!(bytes.len(), 14);
    }

    #[test]
    fn confirm_delete_writes_six_raw_bytes() {
        let packet = ConfirmDeleteCharacterPacket::new(CharacterId(7), *b"delete");
        let mut writer = ByteWriter::new();
        assert_eq!(packet.packet_to_bytes(&mut writer).unwrap(), 12);
        assert_eq!(writer.as_slice(), &[
            0x29, 0x08, 0x07, 0x00, 0x00, 0x00, b'd', b'e', b'l', b'e', b't', b'e'
        ]);
    }

    #[test]
    fn confirm_result_is_ten_bytes() {
        let bytes = [0x2A, 0x08, 0x09, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = ConfirmDeleteCharacterResultPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(packet.character_id, CharacterId(9));
        assert_eq!(packet.result, 5);
    }

    #[test]
    fn cancel_delete_round_trip() {
        let packet = CancelDeleteCharacterPacket::new(CharacterId(3));
        let mut writer = ByteWriter::new();
        assert_eq!(packet.packet_to_bytes(&mut writer).unwrap(), 6);
        assert_eq!(writer.as_slice(), &[0x2B, 0x08, 0x03, 0x00, 0x00, 0x00]);

        let bytes = [0x2C, 0x08, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let mut reader = ByteReader::without_metadata(&bytes);
        let result = CancelDeleteCharacterResultPacket::packet_from_bytes(&mut reader).unwrap();
        assert_eq!(result.character_id, CharacterId(3));
        assert_eq!(result.result, 1);
        assert_eq!(bytes.len(), 10);
    }
}
