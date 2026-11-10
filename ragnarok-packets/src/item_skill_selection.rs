//! Identify, card insert, warp memo, auto spell, crafting, weapon refine, and
//! change cart for PACKETVER 20250716 MAIN.
//!
//! Item ids are 32-bit (`PACKETVER_MAIN_NUM >= 20181121`). Warp destinations
//! arrive as `ZC_WARPLIST2` (`0x0ABE`); the older fixed `0x011C` list is still
//! parsed. Auto spell is `0x0AFB` (the fixed `0x01CD` list is parsed too).
//! Auto Shadow Spell is `0x0442`. Change Cart is `CZ_REQ_CHANGECART` (`0x01AF`,
//! 4 bytes). `0x01E9` is 81 bytes and is not this skill. Layouts follow the
//! packed Hercules structs for this packetver.

use ragnarok_bytes::{ByteReader, ByteWriter, ConversionResult, FixedByteSize, FromBytes, ToBytes};

use crate::{ClientPacket, InventoryIndex, ItemId, MapServer, Packet, ServerPacket, SkillId};

/// 16-byte map name used by warp lists. Trailing NULs are not part of the name.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct MapName16(pub String);

impl FixedByteSize for MapName16 {
    fn size_in_bytes() -> usize {
        16
    }
}

impl FromBytes for MapName16 {
    fn from_bytes(byte_reader: &mut ByteReader) -> ConversionResult<Self> {
        let mut bytes = [0u8; 16];
        for slot in &mut bytes {
            *slot = u8::from_bytes(byte_reader)?;
        }
        let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
        let name = String::from_utf8_lossy(&bytes[..end]).into_owned();
        Ok(Self(name))
    }
}

impl ToBytes for MapName16 {
    fn to_bytes(&self, byte_writer: &mut ByteWriter) -> ConversionResult<usize> {
        let mut bytes = [0u8; 16];
        let raw = self.0.as_bytes();
        let count = raw.len().min(16);
        bytes[..count].copy_from_slice(&raw[..count]);
        bytes.to_bytes(byte_writer)
    }
}

/// One producible row (`ZC_MAKABLEITEMLIST`, 16 bytes on 20250716).
#[derive(Clone, Debug, PartialEq, Eq, ragnarok_bytes::ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct MakableItemEntry {
    pub item_id: ItemId,
    pub material: [ItemId; 3],
}

/// One weapon the Whitesmith refine skill can select (23 bytes on 20250716).
#[derive(Clone, Debug, PartialEq, Eq, ragnarok_bytes::ByteConvertable, FixedByteSize)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct WeaponRefineEntry {
    pub index: InventoryIndex,
    pub item_id: ItemId,
    pub refine: u8,
    pub cards: [u32; 4],
}

/// `ZC_ITEMIDENTIFY_LIST` (`0x0177`). Each index is the client inventory index
/// plus 2.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0177)]
#[variable_length]
pub struct ItemIdentifyListPacket {
    #[repeating_remaining]
    pub indexes: Vec<InventoryIndex>,
}

/// `CZ_REQ_ITEMIDENTIFY` (`0x0178`). `0xFFFF` is cancel.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0178)]
pub struct RequestItemIdentifyPacket {
    pub index: u16,
}

/// `ZC_ACK_ITEMIDENTIFY` (`0x0179`). `index` is the raw wire value (`0xFFFF`
/// when the player cancelled). `result` 0 is success.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0179)]
pub struct ItemIdentifyAckPacket {
    pub index: u16,
    pub result: u8,
}

/// `CZ_REQ_ITEMCOMPOSITION_LIST` (`0x017A`). Sent when a card is used.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x017A)]
pub struct RequestItemCompositionListPacket {
    pub card_index: InventoryIndex,
}

/// `ZC_ITEMCOMPOSITION_LIST` (`0x017B`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x017B)]
#[variable_length]
pub struct ItemCompositionListPacket {
    #[repeating_remaining]
    pub indexes: Vec<InventoryIndex>,
}

/// `CZ_REQ_ITEMCOMPOSITION` (`0x017C`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x017C)]
pub struct RequestItemCompositionPacket {
    pub card_index: InventoryIndex,
    pub equip_index: InventoryIndex,
}

/// `ZC_ACK_ITEMCOMPOSITION` (`0x017D`). `result` 0 is success.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x017D)]
pub struct ItemCompositionAckPacket {
    pub equip_index: InventoryIndex,
    pub card_index: InventoryIndex,
    pub result: u8,
}

/// `ZC_WARPLIST` (`0x011C`), four fixed 16-byte names.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x011C)]
pub struct WarpListFixedPacket {
    pub skill_id: SkillId,
    pub maps: [MapName16; 4],
}

/// `ZC_WARPLIST2` (`0x0ABE`). This is what 20250716 MAIN sends.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0ABE)]
#[variable_length]
pub struct WarpListPacket {
    pub skill_id: SkillId,
    #[repeating_remaining]
    pub maps: Vec<MapName16>,
}

/// `CZ_SELECT_WARPPOINT` (`0x011B`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x011B)]
pub struct SelectWarpPointPacket {
    pub skill_id: SkillId,
    pub map_name: MapName16,
}

/// `ZC_AUTOSPELLLIST` (`0x01CD`): seven skill ids, unused slots are 0.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01CD)]
pub struct AutoSpellListFixedPacket {
    pub skills: [i32; 7],
}

/// `ZC_AUTOSPELLLIST` (`0x0AFB`) for PACKETVER_MAIN_NUM >= 20181128.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0AFB)]
#[variable_length]
pub struct AutoSpellListPacket {
    #[repeating_remaining]
    pub skills: Vec<i32>,
}

/// `CZ_SELECTAUTOSPELL` (`0x01CE`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01CE)]
pub struct SelectAutoSpellPacket {
    pub skill_id: i32,
}

/// `ZC_SKILL_SELECT_REQUEST` (`0x0442`). `flag` 1 is Auto Shadow Spell.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0442)]
#[variable_length]
pub struct SkillSelectRequestPacket {
    pub flag: i32,
    #[repeating_remaining]
    pub skill_ids: Vec<i16>,
}

/// `CZ_SKILL_SELECT_RESPONSE` (`0x0443`). Skill id 0 is cancel.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0443)]
pub struct SkillSelectResponsePacket {
    pub flag: i32,
    pub skill_id: i16,
}

/// `ZC_MAKINGARROW_LIST` (`0x01AD`). Item ids are 32-bit on 20250716.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01AD)]
#[variable_length]
pub struct MakingArrowListPacket {
    #[repeating_remaining]
    pub item_ids: Vec<ItemId>,
}

/// `CZ_REQ_MAKINGARROW` (`0x01AE`, 6 bytes).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01AE)]
pub struct RequestMakingArrowPacket {
    pub item_id: ItemId,
}

/// `ZC_MAKABLEITEMLIST` (`0x018D`). Pharmacy, forging, and cooking mixes.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x018D)]
#[variable_length]
pub struct MakableItemListPacket {
    #[repeating_remaining]
    pub items: Vec<MakableItemEntry>,
}

/// `CZ_REQMAKINGITEM` (`0x018E`, 18 bytes).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x018E)]
pub struct RequestProduceMixPacket {
    pub item_id: ItemId,
    pub material: [ItemId; 3],
}

/// `ZC_ACK_REQMAKINGITEM` (`0x018F`, 8 bytes). Even results succeed.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x018F)]
pub struct ProduceResultPacket {
    pub result: u16,
    pub item_id: ItemId,
}

/// `ZC_MAKINGITEM_LIST` (`0x025A`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x025A)]
#[variable_length]
pub struct MakingItemListPacket {
    pub make_type: u16,
    #[repeating_remaining]
    pub item_ids: Vec<ItemId>,
}

/// `CZ_REQ_MAKINGITEM` (`0x025B`, 8 bytes).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x025B)]
pub struct RequestMakingItemPacket {
    pub make_type: u16,
    pub item_id: ItemId,
}

/// `ZC_NOTIFY_WEAPONITEMLIST` (`0x0221`).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0221)]
#[variable_length]
pub struct WeaponRefineListPacket {
    #[repeating_remaining]
    pub items: Vec<WeaponRefineEntry>,
}

/// `CZ_REQ_WEAPONREFINE` (`0x0222`). Zero or negative cancels. A chosen index
/// is the inventory index plus 2.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0222)]
pub struct RequestWeaponRefinePacket {
    pub index: i32,
}

/// `ZC_ACK_WEAPONREFINE` (`0x0223`, 10 bytes on 20250716).
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0223)]
pub struct WeaponRefineAckPacket {
    pub result: i32,
    pub item_id: ItemId,
}

/// `CZ_REQ_CHANGECART` (`0x01AF`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01AF)]
pub struct RequestChangeCartPacket {
    pub cart_type: u16,
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;
    use crate::PacketExt;

    fn assert_round_trip<T>(bytes: &[u8], decode: impl FnOnce(&mut ByteReader<'_>) -> T, check: impl FnOnce(&T))
    where
        T: PacketExt,
    {
        let mut reader = ByteReader::without_metadata(bytes);
        let packet = decode(&mut reader);
        assert!(reader.is_empty(), "unread bytes remain");
        check(&packet);
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), bytes);
    }

    #[test]
    fn item_identify_list_and_requests_match_hercules() {
        let list = [0x77, 0x01, 0x08, 0x00, 0x02, 0x00, 0x05, 0x00];
        assert_round_trip(
            &list,
            |reader| ItemIdentifyListPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.indexes, vec![InventoryIndex(0), InventoryIndex(3)]);
            },
        );

        let request = RequestItemIdentifyPacket { index: 5 };
        let mut writer = ByteWriter::new();
        request.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x78, 0x01, 0x05, 0x00]);

        let cancel = RequestItemIdentifyPacket { index: 0xFFFF };
        let mut writer = ByteWriter::new();
        cancel.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x78, 0x01, 0xFF, 0xFF]);

        let ack = [0x79, 0x01, 0x05, 0x00, 0x00];
        assert_eq!(ack.len(), 5);
        assert_round_trip(
            &ack,
            |reader| ItemIdentifyAckPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.index, 5);
                assert_eq!(packet.result, 0);
            },
        );
    }

    #[test]
    fn card_composition_packets_match_hercules() {
        let request = RequestItemCompositionListPacket {
            card_index: InventoryIndex(4),
        };
        let mut writer = ByteWriter::new();
        request.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x7A, 0x01, 0x06, 0x00]);

        let list = [0x7B, 0x01, 0x06, 0x00, 0x03, 0x00];
        assert_round_trip(
            &list,
            |reader| ItemCompositionListPacket::packet_from_bytes(reader).unwrap(),
            |packet| assert_eq!(packet.indexes, vec![InventoryIndex(1)]),
        );

        let insert = RequestItemCompositionPacket {
            card_index: InventoryIndex(4),
            equip_index: InventoryIndex(1),
        };
        let mut writer = ByteWriter::new();
        insert.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x7C, 0x01, 0x06, 0x00, 0x03, 0x00]);

        let ack = [0x7D, 0x01, 0x03, 0x00, 0x06, 0x00, 0x00];
        assert_eq!(ack.len(), 7);
        assert_round_trip(
            &ack,
            |reader| ItemCompositionAckPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.equip_index, InventoryIndex(1));
                assert_eq!(packet.card_index, InventoryIndex(4));
                assert_eq!(packet.result, 0);
            },
        );
    }

    #[test]
    fn warp_lists_and_select_match_hercules() {
        let mut fixed = vec![0x1C, 0x01, 0x1B, 0x00];
        fixed.extend(b"prontera\0\0\0\0\0\0\0\0");
        fixed.extend(b"Random\0\0\0\0\0\0\0\0\0\0");
        fixed.extend([0u8; 32]);
        assert_eq!(fixed.len(), 68);
        assert_round_trip(
            &fixed,
            |reader| WarpListFixedPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.skill_id, SkillId(27));
                assert_eq!(packet.maps[0].0, "prontera");
                assert_eq!(packet.maps[1].0, "Random");
                assert_eq!(packet.maps[2].0, "");
            },
        );

        let mut variable = vec![0xBE, 0x0A, 0x16, 0x00, 0x1B, 0x00];
        variable.extend(b"prontera\0\0\0\0\0\0\0\0");
        assert_eq!(variable.len(), 22);
        assert_round_trip(
            &variable,
            |reader| WarpListPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.skill_id, SkillId(27));
                assert_eq!(packet.maps.len(), 1);
                assert_eq!(packet.maps[0].0, "prontera");
            },
        );

        let select = SelectWarpPointPacket {
            skill_id: SkillId(27),
            map_name: MapName16("prontera".to_owned()),
        };
        let mut writer = ByteWriter::new();
        select.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 20);
        assert_eq!(&writer.as_slice()[..4], &[0x1B, 0x01, 0x1B, 0x00]);
        assert_eq!(&writer.as_slice()[4..12], b"prontera");
        assert!(writer.as_slice()[12..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn autospell_and_shadow_spell_packets_match_hercules() {
        let mut fixed = vec![0xCD, 0x01];
        fixed.extend(28u32.to_le_bytes());
        fixed.extend([0u8; 24]);
        assert_eq!(fixed.len(), 30);
        assert_round_trip(
            &fixed,
            |reader| AutoSpellListFixedPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.skills[0], 28);
                assert_eq!(packet.skills[1], 0);
            },
        );

        let variable = [0xFB, 0x0A, 0x0C, 0x00, 0x1C, 0x00, 0x00, 0x00, 0x1D, 0x00, 0x00, 0x00];
        assert_round_trip(
            &variable,
            |reader| AutoSpellListPacket::packet_from_bytes(reader).unwrap(),
            |packet| assert_eq!(packet.skills, vec![28, 29]),
        );

        let select = SelectAutoSpellPacket { skill_id: 28 };
        let mut writer = ByteWriter::new();
        select.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xCE, 0x01, 0x1C, 0x00, 0x00, 0x00]);

        let request = [0x42, 0x04, 0x0C, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0A, 0x00, 0x0B, 0x00];
        assert_round_trip(
            &request,
            |reader| SkillSelectRequestPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.flag, 1);
                assert_eq!(packet.skill_ids, vec![10, 11]);
            },
        );

        let response = SkillSelectResponsePacket { flag: 1, skill_id: 0 };
        let mut writer = ByteWriter::new();
        response.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x43, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(writer.as_slice().len(), 8);
    }

    #[test]
    fn arrow_and_produce_packets_match_hercules() {
        let arrows = [0xAD, 0x01, 0x0C, 0x00, 0xAF, 0x06, 0x00, 0x00, 0xB0, 0x06, 0x00, 0x00];
        assert_round_trip(
            &arrows,
            |reader| MakingArrowListPacket::packet_from_bytes(reader).unwrap(),
            |packet| assert_eq!(packet.item_ids, vec![ItemId(0x06AF), ItemId(0x06B0)]),
        );

        let arrow = RequestMakingArrowPacket { item_id: ItemId(0x06AF) };
        let mut writer = ByteWriter::new();
        arrow.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xAE, 0x01, 0xAF, 0x06, 0x00, 0x00]);
        assert_eq!(writer.as_slice().len(), 6);

        let mut mix = vec![0x8D, 0x01, 0x14, 0x00];
        mix.extend(501u32.to_le_bytes());
        mix.extend(713u32.to_le_bytes());
        mix.extend(0u32.to_le_bytes());
        mix.extend(0u32.to_le_bytes());
        assert_eq!(mix.len(), 20);
        assert_round_trip(
            &mix,
            |reader| MakableItemListPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.items.len(), 1);
                assert_eq!(packet.items[0].item_id, ItemId(501));
                assert_eq!(packet.items[0].material[0], ItemId(713));
            },
        );

        let produce = RequestProduceMixPacket {
            item_id: ItemId(501),
            material: [ItemId(713), ItemId(0), ItemId(0)],
        };
        let mut writer = ByteWriter::new();
        produce.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice().len(), 18);
        assert_eq!(&writer.as_slice()[..6], &[0x8E, 0x01, 0xF5, 0x01, 0x00, 0x00]);

        let result = [0x8F, 0x01, 0x00, 0x00, 0xF5, 0x01, 0x00, 0x00];
        assert_eq!(result.len(), 8);
        assert_round_trip(
            &result,
            |reader| ProduceResultPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.result, 0);
                assert_eq!(packet.item_id, ItemId(501));
            },
        );

        let cooking = [0x5A, 0x02, 0x0A, 0x00, 0x01, 0x00, 0x2C, 0x04, 0x00, 0x00];
        assert_round_trip(
            &cooking,
            |reader| MakingItemListPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.make_type, 1);
                assert_eq!(packet.item_ids, vec![ItemId(1068)]);
            },
        );

        let cook = RequestMakingItemPacket {
            make_type: 1,
            item_id: ItemId(1068),
        };
        let mut writer = ByteWriter::new();
        cook.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x5B, 0x02, 0x01, 0x00, 0x2C, 0x04, 0x00, 0x00]);
        assert_eq!(writer.as_slice().len(), 8);
    }

    #[test]
    fn weapon_refine_and_change_cart_packets_match_hercules() {
        let mut list = vec![0x21, 0x02, 0x1B, 0x00];
        list.extend(4u16.to_le_bytes());
        list.extend(1201u32.to_le_bytes());
        list.push(7);
        list.extend(4001u32.to_le_bytes());
        list.extend([0u8; 12]);
        assert_eq!(list.len(), 4 + 23);
        assert_round_trip(
            &list,
            |reader| WeaponRefineListPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.items.len(), 1);
                assert_eq!(packet.items[0].index, InventoryIndex(2));
                assert_eq!(packet.items[0].item_id, ItemId(1201));
                assert_eq!(packet.items[0].refine, 7);
                assert_eq!(packet.items[0].cards[0], 4001);
            },
        );

        let refine = RequestWeaponRefinePacket { index: 4 };
        let mut writer = ByteWriter::new();
        refine.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0x22, 0x02, 0x04, 0x00, 0x00, 0x00]);
        assert_eq!(writer.as_slice().len(), 6);

        let ack = [0x23, 0x02, 0x00, 0x00, 0x00, 0x00, 0xB1, 0x04, 0x00, 0x00];
        assert_eq!(ack.len(), 10);
        assert_round_trip(
            &ack,
            |reader| WeaponRefineAckPacket::packet_from_bytes(reader).unwrap(),
            |packet| {
                assert_eq!(packet.result, 0);
                assert_eq!(packet.item_id, ItemId(1201));
            },
        );

        let cart = RequestChangeCartPacket { cart_type: 5 };
        let mut writer = ByteWriter::new();
        cart.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xAF, 0x01, 0x05, 0x00]);
        assert_eq!(writer.as_slice().len(), 4);
    }
}
