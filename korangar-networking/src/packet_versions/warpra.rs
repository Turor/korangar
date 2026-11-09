//! Turoran Warpra menu (`0x0EF4`, `0x0EFE`, `0x0EFF`).
//!
//! Layouts follow the Turoran browser builders. A destination list may arrive
//! in chunks; `flags` bit 0 means the catalog is not finished.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{WarpraDestinationEntry, WarpraDestinationListPacket, WarpraResultPacket};

use crate::event::{NetworkEvent, NetworkEventList};

pub const WARPRA_FLAG_MORE: u8 = 0x01;

pub fn register_warpra_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: WarpraDestinationListPacket| NetworkEvent::WarpraList {
        zeny: packet.zeny,
        price: packet.price,
        flags: packet.flags,
        total: packet.total,
        destinations: packet.destinations,
    })?;
    packet_handler.register(|packet: WarpraResultPacket| NetworkEvent::WarpraResult {
        result: packet.result,
        kind: packet.kind,
        location_id: packet.location_id,
        summary: format_warpra_result(packet.result, packet.kind, packet.location_id, None, None),
    })?;
    Ok(())
}

pub fn warpra_kind_name(kind: u8) -> &'static str {
    match kind {
        0 => "Town",
        1 => "Field",
        2 => "Dungeon",
        3 => "Special",
        _ => "Other",
    }
}

/// Level-gated names look like `Payon Dungeon (Lv 55+)`.
pub fn warpra_level_requirement(name: &str) -> Option<u32> {
    let start = name.rfind("(Lv ")?;
    let rest = &name[start + 4..];
    let digits = rest.split('+').next()?;
    let level = digits.trim().parse().ok()?;
    if rest[digits.len()..].starts_with('+') { Some(level) } else { None }
}

pub fn format_warpra_destinations(entries: &[WarpraDestinationEntry]) -> String {
    if entries.is_empty() {
        return "No destinations.".to_owned();
    }
    entries
        .iter()
        .map(|entry| {
            let state = if entry.unlocked == 0 { "locked" } else { "unlocked" };
            format!(
                "{kind} #{id} {name} [{map}] ({x},{y}) group {group} {group_name} {state}",
                kind = warpra_kind_name(entry.kind),
                id = entry.id,
                name = entry.name,
                map = entry.map,
                x = entry.x,
                y = entry.y,
                group = entry.group,
                group_name = entry.group_name,
                state = state,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn format_warpra_result(result: i16, kind: i16, location_id: i16, name: Option<&str>, price: Option<i32>) -> String {
    let place = name.unwrap_or("that location");
    match result {
        0 => format!("Warpra accepted {place} (kind {kind}, id {location_id})."),
        1 => format!("Warp to {place} failed."),
        2 => format!("{place} is locked."),
        3 => match price {
            Some(price) => format!("Need {price} zeny to unlock {place}."),
            None => format!("Not enough zeny to unlock {place}."),
        },
        4 => format!("Warpra rejected {place}."),
        5 => "You cannot warp right now.".to_owned(),
        6 => format!("{place} requires a higher base level."),
        other => format!("Warpra result {other} for kind {kind}, id {location_id}."),
    }
}

/// Fold catalog chunks. Returns the finished list once a chunk arrives without
/// the more-data flag.
///
/// `pending` is one in-flight transfer. A chunk that cannot extend that count
/// (a new catalog, a retry, or a chunk after the previous total was already
/// reached) replaces it instead of appending.
pub fn push_warpra_chunk(
    pending: &mut Vec<WarpraDestinationEntry>,
    flags: u8,
    total: u16,
    entries: Vec<WarpraDestinationEntry>,
) -> Option<Vec<WarpraDestinationEntry>> {
    let more = flags & WARPRA_FLAG_MORE != 0;
    let total_len = usize::from(total);
    let incoming = entries.len();
    let held = pending.len();
    let continues = held > 0
        && total_len > 0
        && held < total_len
        && held + incoming <= total_len
        && (more || held + incoming == total_len);
    if held > 0 && !continues {
        pending.clear();
    }
    if more {
        pending.extend(entries);
        return None;
    }
    if pending.is_empty() {
        Some(entries)
    } else {
        pending.extend(entries);
        Some(std::mem::take(pending))
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::WarpraDestinationEntry;

    use super::{format_warpra_result, push_warpra_chunk, warpra_level_requirement};

    fn row(id: i16, name: &str) -> WarpraDestinationEntry {
        WarpraDestinationEntry {
            kind: 0,
            id,
            unlocked: 1,
            x: 1,
            y: 2,
            group: 0,
            map: "prontera".to_owned(),
            name: name.to_owned(),
            group_name: String::new(),
        }
    }

    #[test]
    fn level_gate_reads_the_name_suffix() {
        assert_eq!(warpra_level_requirement("Payon Dungeon (Lv 55+)"), Some(55));
        assert_eq!(warpra_level_requirement("Prontera"), None);
        assert_eq!(warpra_level_requirement("Almost (Lv 3)"), None);
    }

    #[test]
    fn chunks_wait_for_the_final_packet() {
        let mut pending = Vec::new();
        assert!(push_warpra_chunk(&mut pending, 1, 2, vec![row(1, "A")]).is_none());
        let done = push_warpra_chunk(&mut pending, 0, 2, vec![row(2, "B")]).unwrap();
        assert_eq!(done.len(), 2);
        assert!(pending.is_empty());
        assert_eq!(
            format_warpra_result(3, 0, 1, Some("Prontera"), Some(50_000_000)),
            "Need 50000000 zeny to unlock Prontera."
        );
    }

    #[test]
    fn a_new_catalog_replaces_an_unfinished_one() {
        let mut pending = vec![row(1, "stale")];
        let done = push_warpra_chunk(&mut pending, 0, 1, vec![row(9, "fresh")]).unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].id, 9);
        assert!(pending.is_empty());

        let mut pending = vec![row(1, "A"), row(2, "B")];
        assert!(push_warpra_chunk(&mut pending, 1, 1, vec![row(3, "C")]).is_none());
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, 3);
    }
}
