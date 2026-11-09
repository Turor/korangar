//! Alternate equipment set (`0x0A9B` and the switch add/remove/activate acks).
use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    EquipPosition, EquipSwitchAddResultPacket, EquipSwitchRemoveResultPacket, EquipSwitchReplyPacket, EquippableSwitchItemInformation,
    EquippableSwitchItemListPacket, InventoryIndex,
};

use crate::NetworkEvent;
use crate::event::NetworkEventList;

/// One item assigned to the alternate equipment set.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
pub struct EquipSwitchSlot {
    pub index: InventoryIndex,
    pub position: EquipPosition,
    pub label: String,
}

pub(crate) fn register_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: EquippableSwitchItemListPacket| NetworkEvent::SetEquipSwitch {
        entries: packet.item_information,
    })?;
    packet_handler.register(|packet: EquipSwitchAddResultPacket| {
        (packet.flag == 0).then(|| NetworkEvent::UpdateEquipSwitch {
            index: packet.inventory_index,
            position: EquipPosition::from_bits_truncate(packet.position),
        })
    })?;
    packet_handler.register(|packet: EquipSwitchRemoveResultPacket| {
        (packet.failed == 0).then(|| NetworkEvent::UpdateEquipSwitch {
            index: packet.inventory_index,
            position: EquipPosition::empty(),
        })
    })?;
    packet_handler.register(|packet: EquipSwitchReplyPacket| NetworkEvent::EquipSwitchReply {
        failed: packet.failed != 0,
    })?;
    Ok(())
}

/// Build the switch set from `0x0A9B`. Empty positions are dropped.
pub fn replace_slots(
    entries: Vec<EquippableSwitchItemInformation>,
    item_name: impl Fn(InventoryIndex) -> Option<String>,
) -> Vec<EquipSwitchSlot> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let position = EquipPosition::from_bits_truncate(entry.position);
            (!position.is_empty()).then(|| EquipSwitchSlot {
                label: slot_label(entry.index, position, item_name(entry.index)),
                index: entry.index,
                position,
            })
        })
        .collect()
}

/// Insert, replace, or remove one switch assignment. An empty position removes
/// it.
pub fn assign_slot(slots: &mut Vec<EquipSwitchSlot>, index: InventoryIndex, position: EquipPosition, item_name: Option<String>) {
    slots.retain(|slot| slot.index != index);
    if !position.is_empty() {
        slots.push(EquipSwitchSlot {
            label: slot_label(index, position, item_name),
            index,
            position,
        });
    }
}

fn slot_label(index: InventoryIndex, position: EquipPosition, item_name: Option<String>) -> String {
    let name = item_name.unwrap_or_else(|| format!("item {}", index.0));
    format!("{} - {name}", position_label(position))
}

pub fn position_label(mut position: EquipPosition) -> String {
    const NAMED: &[(EquipPosition, &str)] = &[
        (EquipPosition::LEFT_RIGHT_HAND, "Two-hand"),
        (EquipPosition::LEFT_RIGHT_ACCESSORY, "Accessory"),
        (EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY, "Shadow accessory"),
        (EquipPosition::HEAD_LOWER, "Head lower"),
        (EquipPosition::HEAD_MIDDLE, "Head middle"),
        (EquipPosition::HEAD_TOP, "Head top"),
        (EquipPosition::RIGHT_HAND, "Right hand"),
        (EquipPosition::LEFT_HAND, "Left hand"),
        (EquipPosition::ARMOR, "Armor"),
        (EquipPosition::SHOES, "Shoes"),
        (EquipPosition::GARMENT, "Garment"),
        (EquipPosition::LEFT_ACCESSORY, "Left accessory"),
        (EquipPosition::RIGTH_ACCESSORY, "Right accessory"),
        (EquipPosition::COSTUME_HEAD_TOP, "Costume head top"),
        (EquipPosition::COSTUME_HEAD_MIDDLE, "Costume head middle"),
        (EquipPosition::COSTUME_HEAD_LOWER, "Costume head lower"),
        (EquipPosition::COSTUME_GARMENT, "Costume garment"),
        (EquipPosition::AMMO, "Ammo"),
        (EquipPosition::SHADOW_ARMOR, "Shadow armor"),
        (EquipPosition::SHADOW_WEAPON, "Shadow weapon"),
        (EquipPosition::SHADOW_SHIELD, "Shadow shield"),
        (EquipPosition::SHADOW_SHOES, "Shadow shoes"),
        (EquipPosition::SHADOW_RIGHT_ACCESSORY, "Shadow right accessory"),
        (EquipPosition::SHADOW_LEFT_ACCESSORY, "Shadow left accessory"),
    ];

    let mut names = Vec::new();
    for (flag, name) in NAMED {
        if position.contains(*flag) {
            names.push(*name);
            position.remove(*flag);
        }
    }
    if !position.is_empty() {
        names.push("Other");
    }
    if names.is_empty() { "Empty".to_string() } else { names.join(", ") }
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};
    use ragnarok_packets::handler::{HandlerResult, NoPacketCallback, PacketHandler};
    use ragnarok_packets::{EquippableSwitchItemListPacket, PacketExt};

    use super::*;

    fn name_of(index: InventoryIndex) -> Option<String> {
        (index.0 == 3).then(|| "Knife".to_string())
    }

    #[test]
    fn list_packet_roundtrip_becomes_labeled_slots() {
        let packet = EquippableSwitchItemListPacket {
            item_information: vec![
                EquippableSwitchItemInformation {
                    index: InventoryIndex(3),
                    position: EquipPosition::RIGHT_HAND.bits(),
                },
                EquippableSwitchItemInformation {
                    index: InventoryIndex(4),
                    position: 0,
                },
            ],
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();

        let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::default();
        register_packets(&mut handler).unwrap();
        let mut reader = ByteReader::without_metadata(writer.as_slice());
        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("switch list was not handled");
        };
        let NetworkEvent::SetEquipSwitch { entries } = &events.0[0] else {
            panic!("expected SetEquipSwitch");
        };
        let slots = replace_slots(entries.clone(), name_of);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].index, InventoryIndex(3));
        assert_eq!(slots[0].position, EquipPosition::RIGHT_HAND);
        assert_eq!(slots[0].label, "Right hand - Knife");
    }

    #[test]
    fn failed_add_is_ignored_and_successful_remove_clears_the_slot() {
        let mut slots = replace_slots(
            vec![EquippableSwitchItemInformation {
                index: InventoryIndex(3),
                position: EquipPosition::ARMOR.bits(),
            }],
            name_of,
        );

        let mut add = ByteWriter::new();
        EquipSwitchAddResultPacket {
            inventory_index: InventoryIndex(9),
            position: EquipPosition::SHOES.bits(),
            flag: 1,
        }
        .packet_to_bytes(&mut add)
        .unwrap();
        let mut remove = ByteWriter::new();
        EquipSwitchRemoveResultPacket {
            inventory_index: InventoryIndex(3),
            position: EquipPosition::ARMOR.bits(),
            failed: 0,
        }
        .packet_to_bytes(&mut remove)
        .unwrap();

        let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::default();
        register_packets(&mut handler).unwrap();

        let mut reader = ByteReader::without_metadata(add.as_slice());
        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("add ack was not handled");
        };
        assert!(events.0.is_empty(), "failed add must not change the set");

        let mut reader = ByteReader::without_metadata(remove.as_slice());
        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("remove ack was not handled");
        };
        let NetworkEvent::UpdateEquipSwitch { index, position } = &events.0[0] else {
            panic!("expected UpdateEquipSwitch");
        };
        assign_slot(&mut slots, *index, *position, None);
        assert!(slots.is_empty());
    }

    #[test]
    fn assign_replaces_the_same_index_without_touching_other_slots() {
        let mut slots = replace_slots(
            vec![EquippableSwitchItemInformation {
                index: InventoryIndex(3),
                position: EquipPosition::RIGHT_HAND.bits(),
            }],
            name_of,
        );
        assign_slot(&mut slots, InventoryIndex(8), EquipPosition::LEFT_HAND, Some("Guard".into()));
        assign_slot(
            &mut slots,
            InventoryIndex(3),
            EquipPosition::LEFT_RIGHT_HAND,
            Some("Lance".into()),
        );
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].index, InventoryIndex(8));
        assert_eq!(slots[1].label, "Two-hand - Lance");
    }

    #[test]
    fn reply_packet_reports_failure() {
        let mut writer = ByteWriter::new();
        EquipSwitchReplyPacket { failed: 1 }.packet_to_bytes(&mut writer).unwrap();
        let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::default();
        register_packets(&mut handler).unwrap();
        let mut reader = ByteReader::without_metadata(writer.as_slice());
        let HandlerResult::Ok(events) = handler.process_one(&mut reader) else {
            panic!("reply was not handled");
        };
        assert!(matches!(events.0[0], NetworkEvent::EquipSwitchReply { failed: true }));
    }
}
