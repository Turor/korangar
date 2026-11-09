//! Item reform, enchant grade, and Laphine synthesis/upgrade.
//!
//! Layouts match PACKETVER 20250716 MAIN. Reform and grade are not registered
//! on the 20190530 profile. Laphine synthesis and upgrade are on all three
//! profiles this client speaks.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    ENCHANT_GRADE_UI_TYPE, GradeEnchantBroadcastPacket, GradeEnchantMaterialListPacket, GradeEnchantResultPacket, ItemReformOpenPacket,
    ItemReformResultPacket, LaphineSynthesisOpenPacket, LaphineSynthesisResultPacket, LaphineUpgradeOpenPacket, LaphineUpgradeResultPacket,
    UiOpenPacket,
};

use crate::event::NetworkEventList;
use crate::{GradeBlessingOffer, GradeMaterialOffer, NetworkEvent};

pub fn register_reform_and_grade<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: UiOpenPacket| {
        if packet.ui_type == ENCHANT_GRADE_UI_TYPE {
            Some(NetworkEvent::EnchantGradeOpened { data: packet.data })
        } else {
            None
        }
    })?;
    packet_handler.register(|packet: GradeEnchantMaterialListPacket| {
        let stored_index = (packet.index >= 2).then_some(packet.index as u16 - 2);
        NetworkEvent::EnchantGradeMaterials {
            index: stored_index,
            success_chance: packet.success_chance,
            blessing: GradeBlessingOffer {
                item_id: packet.blessing.item_id,
                amount: packet.blessing.amount,
                max_blessing: packet.blessing.max_blessing,
                bonus: packet.blessing.bonus,
            },
            protect_item_id: packet.protect_item_id,
            protect_amount: packet.protect_amount,
            materials: packet
                .materials
                .into_iter()
                .map(|material| GradeMaterialOffer {
                    item_id: material.item_id,
                    amount: material.amount,
                    price: material.price,
                    downgrade: material.downgrade != 0,
                    breakable: material.breakable != 0,
                })
                .collect(),
        }
    })?;
    packet_handler.register(|packet: GradeEnchantResultPacket| NetworkEvent::EnchantGradeResult {
        index: packet.index,
        grade: packet.grade,
        result: packet.result,
    })?;
    packet_handler.register(|packet: GradeEnchantBroadcastPacket| NetworkEvent::EnchantGradeBroadcast {
        name: packet.name,
        item_id: packet.item_id,
        grade: packet.grade,
        success: packet.status != 0,
    })?;
    packet_handler.register(|packet: ItemReformOpenPacket| NetworkEvent::ItemReformOpened { item_id: packet.item_id })?;
    packet_handler.register(|packet: ItemReformResultPacket| NetworkEvent::ItemReformResult {
        index: packet.index,
        result: packet.result,
    })?;
    Ok(())
}

pub fn register_laphine<Callback>(packet_handler: &mut PacketHandler<NetworkEventList, Callback>) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: LaphineSynthesisOpenPacket| NetworkEvent::LaphineSynthesisOpened { item_id: packet.item_id })?;
    packet_handler.register(|packet: LaphineSynthesisResultPacket| NetworkEvent::LaphineSynthesisResult { result: packet.result })?;
    packet_handler.register(|packet: LaphineUpgradeOpenPacket| NetworkEvent::LaphineUpgradeOpened { item_id: packet.item_id })?;
    packet_handler.register(|packet: LaphineUpgradeResultPacket| NetworkEvent::LaphineUpgradeResult { result: packet.result })?;
    Ok(())
}
