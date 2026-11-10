//! Identify, card insert, warp memo, auto spell, crafting, weapon refine, and
//! change cart. Incoming layouts are the 20250716 MAIN widths (32-bit item ids,
//! `0x0ABE` warp lists, `0x0AFB` auto spell). Older profiles skip these packets
//! by framing length. Registration lives on the 20250716 profile only.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    AutoSpellListFixedPacket, AutoSpellListPacket, ItemCompositionAckPacket, ItemCompositionListPacket, ItemId, ItemIdentifyAckPacket,
    ItemIdentifyListPacket, MakableItemListPacket, MakingArrowListPacket, MakingItemListPacket, MapName16, ProduceResultPacket,
    RequestChangeCartPacket, RequestItemCompositionListPacket, RequestItemCompositionPacket, RequestItemIdentifyPacket,
    RequestMakingArrowPacket, RequestMakingItemPacket, RequestProduceMixPacket, RequestWeaponRefinePacket, SelectAutoSpellPacket,
    SelectWarpPointPacket, SkillId, SkillSelectRequestPacket, SkillSelectResponsePacket, WarpListFixedPacket, WarpListPacket,
    WeaponRefineAckPacket, WeaponRefineListPacket,
};

use crate::event::NetworkEventList;
use crate::packet_versions::SupportedPacketVersion;
use crate::{NetworkEvent, NetworkingSystem, NotConnectedError, WeaponRefineOffer};

pub fn register_item_skill_selection_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: ItemIdentifyListPacket| NetworkEvent::ItemIdentifyList {
        indexes: packet.indexes,
    })?;
    packet_handler.register(|packet: ItemIdentifyAckPacket| NetworkEvent::ItemIdentifyResult {
        index: packet.index,
        result: packet.result,
    })?;
    packet_handler.register(|packet: ItemCompositionListPacket| NetworkEvent::ItemCompositionList {
        indexes: packet.indexes,
    })?;
    packet_handler.register(|packet: ItemCompositionAckPacket| NetworkEvent::ItemCompositionResult {
        equip_index: packet.equip_index,
        card_index: packet.card_index,
        result: packet.result,
    })?;
    packet_handler.register(|packet: WarpListFixedPacket| NetworkEvent::WarpList {
        skill_id: packet.skill_id,
        maps: packet.maps.into_iter().map(|map| map.0).collect(),
    })?;
    packet_handler.register(|packet: WarpListPacket| NetworkEvent::WarpList {
        skill_id: packet.skill_id,
        maps: packet.maps.into_iter().map(|map| map.0).collect(),
    })?;
    packet_handler.register(|packet: AutoSpellListFixedPacket| NetworkEvent::AutoSpellList {
        skill_ids: packet.skills.into_iter().filter(|skill_id| *skill_id != 0).collect(),
    })?;
    packet_handler.register(|packet: AutoSpellListPacket| NetworkEvent::AutoSpellList {
        skill_ids: packet.skills.into_iter().filter(|skill_id| *skill_id != 0).collect(),
    })?;
    packet_handler.register(|packet: SkillSelectRequestPacket| NetworkEvent::SkillSelectList {
        flag: packet.flag,
        skill_ids: packet.skill_ids,
    })?;
    packet_handler.register(|packet: MakingArrowListPacket| NetworkEvent::MakingArrowList {
        item_ids: packet.item_ids,
    })?;
    packet_handler.register(|packet: MakableItemListPacket| NetworkEvent::MakableItemList {
        items: packet
            .items
            .into_iter()
            .map(|entry| (entry.item_id, entry.material))
            .collect(),
    })?;
    packet_handler.register(|packet: ProduceResultPacket| NetworkEvent::ProduceResult {
        result: packet.result,
        item_id: packet.item_id,
    })?;
    packet_handler.register(|packet: MakingItemListPacket| NetworkEvent::MakingItemList {
        make_type: packet.make_type,
        item_ids: packet.item_ids,
    })?;
    packet_handler.register(|packet: WeaponRefineListPacket| NetworkEvent::WeaponRefineList {
        items: packet
            .items
            .into_iter()
            .map(|entry| WeaponRefineOffer {
                index: entry.index,
                item_id: entry.item_id,
                refine: entry.refine,
            })
            .collect(),
    })?;
    packet_handler.register(|packet: WeaponRefineAckPacket| NetworkEvent::WeaponRefineResult {
        result: packet.result,
        item_id: packet.item_id,
    })?;
    Ok(())
}

impl<Callback: PacketCallback + Send> NetworkingSystem<Callback> {
    /// `CZ_REQ_ITEMIDENTIFY`. `0xFFFF` cancels.
    pub fn request_item_identify(&mut self, wire_index: u16) -> Result<(), NotConnectedError> {
        self.send_any(RequestItemIdentifyPacket { index: wire_index })
    }

    /// `CZ_REQ_ITEMCOMPOSITION_LIST`. Sent when a card is used.
    pub fn request_item_composition_list(&mut self, card_index: ragnarok_packets::InventoryIndex) -> Result<(), NotConnectedError> {
        self.send_any(RequestItemCompositionListPacket { card_index })
    }

    /// `CZ_REQ_ITEMCOMPOSITION`.
    pub fn request_item_composition(
        &mut self,
        card_index: ragnarok_packets::InventoryIndex,
        equip_index: ragnarok_packets::InventoryIndex,
    ) -> Result<(), NotConnectedError> {
        self.send_any(RequestItemCompositionPacket {
            card_index,
            equip_index,
        })
    }

    /// `CZ_SELECT_WARPPOINT`. The map name `cancel` leaves the menu.
    pub fn select_warp_point(&mut self, skill_id: SkillId, map_name: String) -> Result<(), NotConnectedError> {
        self.send_any(SelectWarpPointPacket {
            skill_id,
            map_name: MapName16(map_name),
        })
    }

    /// `CZ_SELECTAUTOSPELL`. Skill id 0 does not clear the menu on the server.
    pub fn select_auto_spell(&mut self, skill_id: i32) -> Result<(), NotConnectedError> {
        self.send_any(SelectAutoSpellPacket { skill_id })
    }

    /// `CZ_SKILL_SELECT_RESPONSE`. Skill id 0 cancels Auto Shadow Spell.
    pub fn select_skill_menu(&mut self, flag: i32, skill_id: i16) -> Result<(), NotConnectedError> {
        self.send_any(SkillSelectResponsePacket { flag, skill_id })
    }

    /// `CZ_REQ_MAKINGARROW` (6 bytes, 32-bit item id). 20250716 only.
    pub fn request_making_arrow(&mut self, item_id: ItemId) -> Result<(), NotConnectedError> {
        self.send_current(RequestMakingArrowPacket { item_id })
    }

    /// `CZ_REQMAKINGITEM` (18 bytes). Materials are echoed from the list.
    pub fn request_produce_mix(&mut self, item_id: ItemId, material: [ItemId; 3]) -> Result<(), NotConnectedError> {
        self.send_current(RequestProduceMixPacket { item_id, material })
    }

    /// `CZ_REQ_MAKINGITEM` (8 bytes).
    pub fn request_making_item(&mut self, make_type: u16, item_id: ItemId) -> Result<(), NotConnectedError> {
        self.send_current(RequestMakingItemPacket { make_type, item_id })
    }

    /// `CZ_REQ_WEAPONREFINE`. Zero cancels. A choice is the inventory index plus 2.
    pub fn request_weapon_refine(&mut self, index: i32) -> Result<(), NotConnectedError> {
        self.send_any(RequestWeaponRefinePacket { index })
    }

    /// `CZ_REQ_CHANGECART`.
    pub fn request_change_cart(&mut self, cart_type: u16) -> Result<(), NotConnectedError> {
        self.send_any(RequestChangeCartPacket { cart_type })
    }

    fn send_any(&mut self, packet: impl ragnarok_packets::MapServerPacket) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(packet)
            }
        }
    }

    fn send_current(&mut self, packet: impl ragnarok_packets::MapServerPacket) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20250716 => self.send_map_server_packet(packet),
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 => Ok(()),
        }
    }
}
