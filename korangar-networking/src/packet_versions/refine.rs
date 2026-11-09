//! Refine UI packets shared by the 20190530, 20220406, and 20250716 map
//! servers. `0x0AA0` opens the window, `0x0AA2` lists ores and chances, and
//! `0x0188` reports the result.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{RefineMaterialListPacket, RefineOpenPacket, RefineResultPacket};

use crate::event::NetworkEventList;
use crate::{NetworkEvent, RefineMaterialOffer};

pub fn register_refine_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|_packet: RefineOpenPacket| NetworkEvent::RefineOpened)?;
    packet_handler.register(|packet: RefineMaterialListPacket| NetworkEvent::RefineMaterials {
        index: packet.index,
        blacksmith_blessing: packet.blacksmith_blessing,
        materials: packet
            .materials
            .into_iter()
            .map(|material| RefineMaterialOffer {
                item_id: material.item_id,
                chance_percent: material.chance_percent,
                zeny: material.zeny,
            })
            .collect(),
    })?;
    packet_handler.register(|packet: RefineResultPacket| NetworkEvent::RefineResult {
        result: packet.result,
        index: packet.index,
        refine_level: packet.refine_level,
    })?;
    Ok(())
}
