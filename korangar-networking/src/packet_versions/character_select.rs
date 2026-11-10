//! Character-select packets that used to be ignored: slot pages, bans, and the
//! timed delete trio (`0x0828`, `0x082A`, `0x082C`).

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    CancelDeleteCharacterResultPacket, CharacterBanListPacket, CharacterSlotPagePacket, ConfirmDeleteCharacterResultPacket,
    ReserveDeleteCharacterResultPacket,
};

use crate::event::{NetworkEvent, NetworkEventList};

pub fn register_character_select_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: CharacterSlotPagePacket| NetworkEvent::CharacterSlotPages {
        page_count: packet.page_quantity,
    })?;
    packet_handler.register(|packet: CharacterBanListPacket| NetworkEvent::CharacterBanList {
        bans: packet
            .character_information
            .into_iter()
            .map(|ban| (ban.character_id, ban.ban_time))
            .collect(),
    })?;
    packet_handler.register(
        |packet: ReserveDeleteCharacterResultPacket| NetworkEvent::CharacterDeleteReserved {
            character_id: packet.character_id,
            result: packet.result,
            remaining_seconds: packet.remaining_seconds,
        },
    )?;
    packet_handler.register(
        |packet: ConfirmDeleteCharacterResultPacket| NetworkEvent::CharacterDeleteAccepted {
            character_id: packet.character_id,
            result: packet.result,
        },
    )?;
    packet_handler.register(
        |packet: CancelDeleteCharacterResultPacket| NetworkEvent::CharacterDeleteCanceled {
            character_id: packet.character_id,
            result: packet.result,
        },
    )?;
    Ok(())
}
