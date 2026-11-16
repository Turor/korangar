//! Rodex compose packets: open the write form, check the receiver, attach
//! and remove items, send, and cancel. Layouts follow Hercules
//! `packets_struct.h` and roBrowser `PacketStructure.js`.

use ragnarok_bytes::{FromBytes, ToBytes};
use ragnarok_macros::{ClientPacket, MapServer, Packet, ServerPacket};

use crate::{InventoryIndex, ItemOptions};

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

/// `CZ_REQ_ADD_ITEM_RODEX` (`0x0A04`, 6 bytes). Attach `count` of one
/// inventory stack to the mail being written.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A04)]
pub struct AddRodexItemPacket {
    pub index: InventoryIndex,
    pub count: u16,
}

/// `CZ_REQ_REMOVE_RODEX_ITEM` (`0x0A06`, 6 bytes). Take `count` of an
/// attached stack back off the mail.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A06)]
pub struct RemoveRodexItemPacket {
    pub index: InventoryIndex,
    pub count: u16,
}

/// `ZC_ACK_ADD_ITEM_RODEX` (`0x0A05`, 63 bytes), before PACKETVER_MAIN_NUM
/// 20200916.
///
/// On a failure Hercules zeroes everything after `result`, so `raw_index`
/// is kept as the wire value (inventory index + 2, or 0) instead of an
/// [`InventoryIndex`], which would reject the 0. `weight` is the mail's
/// total attachment weight in displayed units.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A05)]
pub struct RodexItemAddedPacket {
    pub result: u8,
    pub raw_index: u16,
    pub count: u16,
    pub item_id: u32,
    pub item_type: u8,
    pub identified: u8,
    pub damaged: u8,
    pub refine: u8,
    pub cards: [u32; 4],
    pub options: [ItemOptions; 5],
    pub weight: u16,
    pub favorite: u8,
    pub location: u32,
}

/// `ZC_ACK_ADD_ITEM_RODEX` (`0x0B3F`, 64 bytes), PACKETVER_MAIN_NUM >=
/// 20200916 / RE >= 20200723. Refine moves to the end and a grade byte
/// follows it.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0B3F)]
pub struct RodexItemAddedPacket2 {
    pub result: u8,
    pub raw_index: u16,
    pub count: u16,
    pub item_id: u32,
    pub item_type: u8,
    pub identified: u8,
    pub damaged: u8,
    pub cards: [u32; 4],
    pub options: [ItemOptions; 5],
    pub weight: u16,
    pub favorite: u8,
    pub location: u32,
    pub refine: u8,
    pub grade: u8,
}

/// `ZC_ACK_REMOVE_ITEM_MAIL` (`0x0A07`, 9 bytes). `result` 1 means removed;
/// `count` is how many came off.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A07)]
pub struct RodexItemRemovedPacket {
    pub result: u8,
    pub raw_index: u16,
    pub count: u16,
    pub weight: u16,
}

/// Inventory index from a Rodex ack's wire index (`index + 2`). `None` for
/// the zeroed index of a failed add.
pub fn rodex_ack_index(raw_index: u16) -> Option<u16> {
    raw_index.checked_sub(2)
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

    #[test]
    fn attachment_requests_carry_the_wire_index() {
        let add = write(&AddRodexItemPacket {
            index: InventoryIndex(5),
            count: 3,
        });
        assert_eq!(add, [0x04, 0x0A, 7, 0, 3, 0]);
        let remove = write(&RemoveRodexItemPacket {
            index: InventoryIndex(0),
            count: 1,
        });
        assert_eq!(remove, [0x06, 0x0A, 2, 0, 1, 0]);
    }

    fn add_ack_prefix(header: u16, result: u8, raw_index: u16, count: u16, item_id: u32) -> Vec<u8> {
        let mut bytes = header.to_le_bytes().to_vec();
        bytes.push(result);
        bytes.extend(raw_index.to_le_bytes());
        bytes.extend(count.to_le_bytes());
        bytes.extend(item_id.to_le_bytes());
        bytes
    }

    #[test]
    fn add_acks_parse_both_layouts() {
        // 0x0A05: type, identified, damaged, refine, 4 cards, 5 options, weight,
        // favorite, location = 63 bytes.
        let mut old = add_ack_prefix(0x0A05, 0, 9, 2, 501);
        old.extend([3, 1, 0, 0]);
        old.extend([0u8; 16]);
        old.extend([0u8; 25]);
        old.extend(14u16.to_le_bytes());
        old.push(0);
        old.extend(0u32.to_le_bytes());
        assert_eq!(old.len(), 63);
        let old = parse::<RodexItemAddedPacket>(&old);
        assert_eq!(
            (old.result, old.raw_index, old.count, old.item_id, old.weight),
            (0, 9, 2, 501, 14)
        );
        assert_eq!(rodex_ack_index(old.raw_index), Some(7));

        // 0x0B3F: refine and grade move to the end = 64 bytes.
        let mut new = add_ack_prefix(0x0B3F, 0, 2, 1, 1201);
        new.extend([5, 1, 0]);
        new.extend(4001u32.to_le_bytes());
        new.extend([0u8; 12]);
        new.extend([0u8; 25]);
        new.extend(50u16.to_le_bytes());
        new.push(1);
        new.extend(2u32.to_le_bytes());
        new.extend([7, 1]);
        assert_eq!(new.len(), 64);
        let new = parse::<RodexItemAddedPacket2>(&new);
        assert_eq!(
            (new.item_id, new.cards[0], new.weight, new.location, new.refine, new.grade),
            (1201, 4001, 50, 2, 7, 1)
        );
        assert_eq!(rodex_ack_index(new.raw_index), Some(0));
    }

    #[test]
    fn a_failed_add_has_a_zero_index() {
        let mut failed = add_ack_prefix(0x0B3F, 4, 0, 0, 0);
        failed.resize(64, 0);
        let failed = parse::<RodexItemAddedPacket2>(&failed);
        assert_eq!(failed.result, 4);
        assert_eq!(rodex_ack_index(failed.raw_index), None);
    }

    #[test]
    fn remove_ack_parses() {
        let mut bytes = 0x0A07u16.to_le_bytes().to_vec();
        bytes.push(1);
        bytes.extend(7u16.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(30u16.to_le_bytes());
        assert_eq!(bytes.len(), 9);
        let removed = parse::<RodexItemRemovedPacket>(&bytes);
        assert_eq!((removed.result, removed.count, removed.weight), (1, 2, 30));
        assert_eq!(rodex_ack_index(removed.raw_index), Some(5));
    }
}
