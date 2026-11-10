//! View-equipment microscope and the "show my equipment" config echo.
//!
//! `ZC_EQUIPWIN_MICROSCOPE` on PACKETVER 20250716 is `0x0B37`. `0x02DA` is the
//! login notify and `0x02D9` type 0 is the config echo. Other config types are
//! ignored.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::version_20250716::EquipWindowMicroscopePacket;
use ragnarok_packets::{CONFIG_OPEN_EQUIPMENT_WINDOW, EquippableItemFlags, UpdateConfigurationPacket, UpdateShowEquipPacket};

use crate::event::{NetworkEvent, NetworkEventList, ViewedEquipPiece};

pub fn register_show_equip_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: UpdateShowEquipPacket| NetworkEvent::ShowEquipment {
        enabled: packet.open_equip_window != 0,
    })?;
    packet_handler.register(|packet: UpdateConfigurationPacket| {
        if packet.config_type == CONFIG_OPEN_EQUIPMENT_WINDOW {
            Some(NetworkEvent::ShowEquipment {
                enabled: packet.value != 0,
            })
        } else {
            None
        }
    })?;
    Ok(())
}

pub fn register_view_equip_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: EquipWindowMicroscopePacket| {
        let items = packet
            .items
            .into_iter()
            .map(|item| ViewedEquipPiece {
                item_id: item.item_id.0,
                // `WearState` (`it->equip`) is the socket the item is worn in.
                // `location` is the item-db mask (`pc->equippoint`), which is
                // 136 for every ring and 34 for each dual-wield weapon.
                location: item.equipped_position.bits(),
                refine: item.refinement_level,
                identified: item.flags.contains(EquippableItemFlags::IDENTIFIED),
            })
            .collect();
        NetworkEvent::PlayerEquipment {
            name: packet.character_name,
            job: packet.job,
            sex: packet.sex,
            items,
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};
    use ragnarok_packets::handler::{HandlerResult, NoPacketCallback, PacketHandler};
    use ragnarok_packets::{CONFIG_OPEN_EQUIPMENT_WINDOW, PacketExt, UpdateConfigurationPacket, UpdateShowEquipPacket};

    use super::{register_show_equip_packets, register_view_equip_packets};
    use crate::NetworkEvent;
    use crate::event::NetworkEventList;

    fn process_show(bytes: &[u8]) -> Vec<NetworkEvent> {
        let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::with_callback(NoPacketCallback).without_callbacks();
        register_show_equip_packets(&mut handler).unwrap();
        let mut reader = ByteReader::without_metadata(bytes);
        match handler.process_one(&mut reader) {
            HandlerResult::Ok(events) => events.0,
            HandlerResult::UnhandledPacket => panic!("show-equip packet was not handled"),
            HandlerResult::PacketCutOff => panic!("show-equip packet was cut off"),
            HandlerResult::InternalError(_) => panic!("show-equip packet failed inside the handler"),
        }
    }

    #[test]
    fn show_equip_notify_toggles_the_option() {
        let mut writer = ByteWriter::new();
        UpdateShowEquipPacket { open_equip_window: 1 }.packet_to_bytes(&mut writer).unwrap();
        let events = process_show(writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::ShowEquipment { enabled: true }]));

        let mut writer = ByteWriter::new();
        UpdateShowEquipPacket { open_equip_window: 0 }.packet_to_bytes(&mut writer).unwrap();
        let events = process_show(writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::ShowEquipment { enabled: false }]));
    }

    #[test]
    fn config_type_zero_is_show_equipment_and_other_types_are_ignored() {
        let mut writer = ByteWriter::new();
        UpdateConfigurationPacket {
            config_type: CONFIG_OPEN_EQUIPMENT_WINDOW,
            value: 1,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        let events = process_show(writer.as_slice());
        assert!(matches!(events.as_slice(), [NetworkEvent::ShowEquipment { enabled: true }]));

        let mut writer = ByteWriter::new();
        UpdateConfigurationPacket { config_type: 2, value: 1 }
            .packet_to_bytes(&mut writer)
            .unwrap();
        assert!(process_show(writer.as_slice()).is_empty());
    }

    #[test]
    fn microscope_packet_becomes_player_equipment() {
        use ragnarok_packets::version_20250716::{EquipWindowMicroscopePacket, EquippableItemInformation};
        use ragnarok_packets::{EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions};

        let option = || ItemOptions {
            index: 0,
            value: 0,
            parameter: 0,
        };
        let mut writer = ByteWriter::new();
        EquipWindowMicroscopePacket {
            character_name: "Alice".to_owned(),
            job: 4001,
            head: 1,
            accessory: 0,
            accessory2: 0,
            accessory3: 0,
            robe: 0,
            head_palette: 0,
            body_palette: 0,
            body: 0,
            sex: 0,
            items: vec![EquippableItemInformation {
                index: InventoryIndex(2),
                item_id: ItemId(501),
                item_type: 5,
                equip_position: EquipPosition::ARMOR,
                equipped_position: EquipPosition::ARMOR,
                slot: [0; 4],
                hire_expiration_date: 0,
                bind_on_equip_type: 0,
                w_item_sprite_number: 0,
                option_count: 0,
                option_data: std::array::from_fn(|_| option()),
                forge_data: std::array::from_fn(|_| option()),
                refinement_level: 7,
                enchantment_level: 0,
                flags: EquippableItemFlags::IDENTIFIED,
            }],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();

        let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::with_callback(NoPacketCallback).without_callbacks();
        register_view_equip_packets(&mut handler).unwrap();
        let mut reader = ByteReader::without_metadata(writer.as_slice());
        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("microscope packet was not handled");
        };
        assert!(reader.remaining_bytes().is_empty());
        match events.0.as_slice() {
            [NetworkEvent::PlayerEquipment { name, job, sex, items }] => {
                assert_eq!(name, "Alice");
                assert_eq!(*job, 4001);
                assert_eq!(*sex, 0);
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].item_id, 501);
                assert_eq!(items[0].location, EquipPosition::ARMOR.bits());
                assert_eq!(items[0].refine, 7);
                assert!(items[0].identified);
            }
            other => panic!("unexpected events: {other:?}"),
        }
    }

    #[test]
    fn microscope_places_rings_and_dual_wield_by_worn_slot() {
        use ragnarok_packets::version_20250716::{EquipWindowMicroscopePacket, EquippableItemInformation};
        use ragnarok_packets::{EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions};

        let option = || ItemOptions {
            index: 0,
            value: 0,
            parameter: 0,
        };
        let item = |index: u16, id: u32, location: EquipPosition, worn: EquipPosition| EquippableItemInformation {
            index: InventoryIndex(index),
            item_id: ItemId(id),
            item_type: 5,
            equip_position: location,
            equipped_position: worn,
            slot: [0; 4],
            hire_expiration_date: 0,
            bind_on_equip_type: 0,
            w_item_sprite_number: 0,
            option_count: 0,
            option_data: std::array::from_fn(|_| option()),
            forge_data: std::array::from_fn(|_| option()),
            refinement_level: 0,
            enchantment_level: 0,
            flags: EquippableItemFlags::IDENTIFIED,
        };
        let mut writer = ByteWriter::new();
        EquipWindowMicroscopePacket {
            character_name: "Bob".to_owned(),
            job: 12,
            head: 1,
            accessory: 0,
            accessory2: 0,
            accessory3: 0,
            robe: 0,
            head_palette: 0,
            body_palette: 0,
            body: 0,
            sex: 1,
            items: vec![
                item(2, 2601, EquipPosition::LEFT_RIGHT_ACCESSORY, EquipPosition::LEFT_ACCESSORY),
                item(3, 2602, EquipPosition::LEFT_RIGHT_ACCESSORY, EquipPosition::RIGTH_ACCESSORY),
                item(4, 1201, EquipPosition::LEFT_RIGHT_HAND, EquipPosition::RIGHT_HAND),
                item(5, 1202, EquipPosition::LEFT_RIGHT_HAND, EquipPosition::LEFT_HAND),
            ],
        }
        .packet_to_bytes(&mut writer)
        .unwrap();

        let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::with_callback(NoPacketCallback).without_callbacks();
        register_view_equip_packets(&mut handler).unwrap();
        let mut reader = ByteReader::without_metadata(writer.as_slice());
        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("microscope packet was not handled");
        };
        let [NetworkEvent::PlayerEquipment { items, .. }] = events.0.as_slice() else {
            panic!("unexpected events");
        };
        let locations: Vec<u32> = items.iter().map(|piece| piece.location).collect();
        assert_eq!(locations, vec![
            EquipPosition::LEFT_ACCESSORY.bits(),
            EquipPosition::RIGTH_ACCESSORY.bits(),
            EquipPosition::RIGHT_HAND.bits(),
            EquipPosition::LEFT_HAND.bits(),
        ]);
    }
}
