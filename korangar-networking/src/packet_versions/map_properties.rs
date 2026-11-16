//! Map property, map type, and one-cell gat updates.
//!
//! `0x099B` is Hercules `clif_maptypeproperty2` (`ZC_MAPPROPERTY_R2`): the
//! type word is `0x28` and the flags carry PvP, GvG, siege, and battleground.
//! `0x01D6` is `clif_map_type` (`ZC_NOTIFY_MAPPROPERTY2`). `0x0192` is
//! `clif_changemapcell` (`ZC_UPDATE_MAPINFO`).

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{ChangeMapCellPacket, MapTypePacket, NotifyMapProperty2Packet};

use crate::event::{NetworkEvent, NetworkEventList};

pub fn register_map_property_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: MapTypePacket| NetworkEvent::MapProperty {
        property_type: packet.map_type,
        flags: packet.flags,
    })?;
    packet_handler.register(|packet: NotifyMapProperty2Packet| NetworkEvent::MapType { map_type: packet.map_type })?;
    packet_handler.register(|packet: ChangeMapCellPacket| NetworkEvent::ChangeMapCell {
        position: packet.position,
        cell_type: packet.cell_type,
        map_name: packet.map_name,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};
    use ragnarok_packets::handler::{HandlerResult, NoPacketCallback, PacketHandler};
    use ragnarok_packets::{NotifyMapProperty2Packet, PacketExt, TilePosition};

    use super::register_map_property_packets;
    use crate::NetworkEvent;
    use crate::event::NetworkEventList;

    fn process(bytes: &[u8]) -> Vec<NetworkEvent> {
        let mut handler = PacketHandler::<NetworkEventList, NoPacketCallback>::with_callback(NoPacketCallback).without_callbacks();
        register_map_property_packets(&mut handler).unwrap();
        let mut reader = ByteReader::without_metadata(bytes);
        match handler.process_one(&mut reader) {
            HandlerResult::Ok(events) => events.0,
            HandlerResult::UnhandledPacket => panic!("map property packet was not handled"),
            HandlerResult::PacketCutOff => panic!("map property packet was cut off"),
            HandlerResult::InternalError(error) => panic!("map property packet failed: {error:?}"),
        }
    }

    #[test]
    fn map_property_r2_keeps_type_and_flag_bits() {
        // Hercules `packet_maptypeproperty2`: opcode, type u16, flags u32.
        // `clif_maptypeproperty2` always writes type `0x28`. Bit 0 is PvP.
        let events = process(&[0x9B, 0x09, 0x28, 0x00, 0x01, 0x00, 0x00, 0x00]);
        assert!(matches!(events.as_slice(), [NetworkEvent::MapProperty {
            property_type: 0x28,
            flags: 1,
        }]));
    }

    #[test]
    fn notify_map_property2_is_the_map_type() {
        // `0x01D6`: opcode, then Hercules `enum map_type` as a `u16`. 19 is
        // battlefield.
        let events = process(&[0xD6, 0x01, 0x13, 0x00]);
        assert!(matches!(events.as_slice(), [NetworkEvent::MapType { map_type: 19 }]));

        let mut writer = ByteWriter::new();
        NotifyMapProperty2Packet { map_type: 19 }.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xD6, 0x01, 0x13, 0x00]);
    }

    #[test]
    fn change_map_cell_keeps_the_gat_type_and_map_name() {
        // `0x0192`: x u16, y u16, gat type u16, map name 16 bytes. Type 5 is an ice
        // wall.
        let mut bytes = vec![0x92, 0x01, 0x03, 0x00, 0x00, 0x00, 0x05, 0x00];
        bytes.extend_from_slice(b"prontera.gat");
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        assert_eq!(bytes.len(), 24);
        let events = process(&bytes);
        assert!(matches!(events.as_slice(), [NetworkEvent::ChangeMapCell {
            position: TilePosition { x: 3, y: 0 },
            cell_type: 5,
            map_name,
        }] if map_name == "prontera.gat"));
    }
}
