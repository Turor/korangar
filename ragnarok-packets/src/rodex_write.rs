//! Rodex compose packets: open the write form, check the receiver, send,
//! and cancel. Layouts follow Hercules `packets_struct.h` and roBrowser
//! `PacketStructure.js`.

use ragnarok_bytes::{FromBytes, ToBytes};
use ragnarok_macros::{ClientPacket, MapServer, Packet, ServerPacket};

/// Longest Rodex title Hercules keeps (`RODEX_TITLE_LENGTH` minus the NUL).
pub const RODEX_TITLE_MAX: usize = 39;
/// Longest Rodex body Hercules keeps (`RODEX_BODY_LENGTH` minus the NUL).
pub const RODEX_BODY_MAX: usize = 499;

/// `CZ_REQ_OPEN_WRITE_MAIL` (`0x0A08`, 26 bytes).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A08)]
pub struct OpenRodexWritePacket {
    #[length(24)]
    pub receiver_name: String,
}

/// `ZC_ACK_OPEN_WRITE_MAIL` (`0x0A12`, 27 bytes). `result` 1 means the form
/// may open.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A12)]
pub struct RodexWriteOpenedPacket {
    #[length(24)]
    pub receiver_name: String,
    pub result: u8,
}

/// `CZ_REQ_CANCEL_WRITE_MAIL` (`0x0A03`, 2 bytes).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A03)]
pub struct CancelRodexWritePacket {}

/// `CZ_CHECK_RECEIVE_CHARACTER_NAME` (`0x0A13`, 26 bytes), before
/// PACKETVER_MAIN_NUM 20201104.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A13)]
pub struct CheckRodexReceiverPacket {
    #[length(24)]
    pub name: String,
}

/// `CZ_CHECKNAME2` (`0x0B97`, 27 bytes), PACKETVER_MAIN_NUM >= 20201104.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B97)]
pub struct CheckRodexReceiverPacket2 {
    #[length(24)]
    pub name: String,
    pub own_character: u8,
}

/// `ZC_CHECKNAME` (`0x0A51`, 34 bytes). `character_id` 0 means no such
/// character; the rest is then unset.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A51)]
pub struct RodexReceiverPacket {
    pub character_id: u32,
    pub class: u16,
    pub base_level: u16,
    #[length(24)]
    pub name: String,
}

/// `CZ_REQ_SEND_RODEX2` (`0x0A6E`). `contents` is the NUL-ended title then
/// the NUL-ended body; the two lengths count those NULs.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A6E)]
#[variable_length]
pub struct SendRodexPacket {
    #[length(24)]
    pub receiver_name: String,
    #[length(24)]
    pub sender_name: String,
    pub zeny: i64,
    pub title_length: u16,
    pub body_length: u16,
    pub receiver_character_id: u32,
    #[length_remaining]
    pub contents: Vec<u8>,
}

impl SendRodexPacket {
    /// Clips title and body to what Hercules keeps, on character
    /// boundaries, and lays them out with their NULs.
    pub fn compose(receiver_name: &str, sender_name: &str, receiver_character_id: u32, zeny: i64, title: &str, body: &str) -> Self {
        let title = clip_bytes(title, RODEX_TITLE_MAX);
        let body = clip_bytes(body, RODEX_BODY_MAX);
        let mut contents = Vec::with_capacity(title.len() + body.len() + 2);
        contents.extend_from_slice(title.as_bytes());
        contents.push(0);
        contents.extend_from_slice(body.as_bytes());
        contents.push(0);
        Self {
            receiver_name: receiver_name.to_owned(),
            sender_name: sender_name.to_owned(),
            zeny,
            title_length: (title.len() + 1) as u16,
            body_length: (body.len() + 1) as u16,
            receiver_character_id,
            contents,
        }
    }
}

/// Longest prefix of `text` that fits in `max` bytes.
pub fn clip_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;
    use crate::PacketExt;

    fn write<T: crate::Packet>(packet: &T) -> Vec<u8> {
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).expect("write rodex packet");
        writer.into_inner()
    }

    fn parse<T: crate::Packet>(bytes: &[u8]) -> T {
        let mut reader = ByteReader::without_metadata(bytes);
        let packet = T::packet_from_bytes(&mut reader).expect("parse rodex packet");
        assert!(reader.remaining_bytes().is_empty());
        packet
    }

    #[test]
    fn send_mail_lays_out_header_title_and_body() {
        let bytes = write(&SendRodexPacket::compose("Bob", "Alice", 150001, 25, "Hi", "Hello Bob"));
        assert_eq!(&bytes[0..2], &0x0A6Eu16.to_le_bytes());
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]) as usize, bytes.len());
        assert_eq!(&bytes[4..7], b"Bob");
        assert_eq!(bytes[7], 0);
        assert_eq!(&bytes[28..33], b"Alice");
        assert_eq!(&bytes[52..60], &25i64.to_le_bytes());
        assert_eq!(&bytes[60..62], &3u16.to_le_bytes());
        assert_eq!(&bytes[62..64], &10u16.to_le_bytes());
        assert_eq!(&bytes[64..68], &150001u32.to_le_bytes());
        assert_eq!(&bytes[68..], b"Hi\0Hello Bob\0");
    }

    #[test]
    fn long_text_is_clipped_on_a_character_boundary() {
        let title = "é".repeat(30);
        let packet = SendRodexPacket::compose("Bob", "Alice", 1, 0, &title, "x");
        assert_eq!(packet.title_length as usize, 38 + 1);
        assert!(std::str::from_utf8(&packet.contents[..38]).is_ok());
        assert_eq!(clip_bytes("abc", 10), "abc");
    }

    #[test]
    fn fixed_compose_packets_have_their_table_sizes() {
        assert_eq!(
            write(&OpenRodexWritePacket {
                receiver_name: String::new()
            })
            .len(),
            26
        );
        assert_eq!(write(&CancelRodexWritePacket {}).len(), 2);
        assert_eq!(write(&CheckRodexReceiverPacket { name: "Bob".to_owned() }).len(), 26);
        let check = write(&CheckRodexReceiverPacket2 {
            name: "Bob".to_owned(),
            own_character: 0,
        });
        assert_eq!(check.len(), 27);
        assert_eq!(&check[0..2], &0x0B97u16.to_le_bytes());
    }

    #[test]
    fn receiver_and_open_replies_parse() {
        let mut bytes = Vec::new();
        bytes.extend(0x0A51u16.to_le_bytes());
        bytes.extend(150001u32.to_le_bytes());
        bytes.extend(4001u16.to_le_bytes());
        bytes.extend(99u16.to_le_bytes());
        let mut name = b"Bob".to_vec();
        name.resize(24, 0);
        bytes.extend(&name);
        let receiver = parse::<RodexReceiverPacket>(&bytes);
        assert_eq!((receiver.character_id, receiver.class, receiver.base_level), (150001, 4001, 99));
        assert_eq!(receiver.name, "Bob");

        let mut opened = 0x0A12u16.to_le_bytes().to_vec();
        opened.extend(&name);
        opened.push(1);
        let opened = parse::<RodexWriteOpenedPacket>(&opened);
        assert_eq!((opened.receiver_name.as_str(), opened.result), ("Bob", 1));
    }
}
