//! Turoran alive-MVP minimap (`0x0EF5`).
//!
//! Wire layout, after the opcode and total length: `flags:u8`, `count:u16`,
//! then `count` rows of `entity_id:u32`, `class_id:u16`, `x:u16`, `y:u16`.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{MvpMinimapEntry, MvpMinimapPacket, TilePosition};

use crate::event::{NetworkEvent, NetworkEventList};

pub fn register_mvp_minimap_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: MvpMinimapPacket| NetworkEvent::MvpMinimap {
        flags: packet.flags,
        markers: packet.markers,
    })?;
    Ok(())
}

/// Map a tile onto a square minimap. Gat Y grows north; the returned Y grows
/// down. `None` when the map size is unknown or the tile is outside the map.
pub fn minimap_point(map_width: u16, map_height: u16, position: TilePosition, size: f32) -> Option<(f32, f32)> {
    if map_width == 0 || map_height == 0 || size <= 0.0 {
        return None;
    }
    if position.x >= map_width || position.y >= map_height {
        return None;
    }
    let x = (f32::from(position.x) + 0.5) / f32::from(map_width) * size;
    let y = size - (f32::from(position.y) + 0.5) / f32::from(map_height) * size;
    Some((x, y))
}

pub fn format_mvp_markers(markers: &[MvpMinimapEntry]) -> String {
    if markers.is_empty() {
        return "No alive MVPs.".to_owned();
    }
    let mut lines = Vec::with_capacity(markers.len());
    for marker in markers {
        lines.push(format!(
            "class {class} at {x}, {y}",
            class = marker.class_id,
            x = marker.position.x,
            y = marker.position.y,
        ));
    }
    lines.join("\n")
}
