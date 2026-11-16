//! Map type from `ZC_NOTIFY_MAPPROPERTY2` (`0x01D6`).
//!
//! Hercules `clif_map_type` writes a 4-byte packet: opcode, then the
//! `map_type` enum as a `u16` (village, dungeon, agit, battlefield, …).

use ragnarok_macros::{MapServer, Packet, ServerPacket};

/// `ZC_NOTIFY_MAPPROPERTY2` (`0x01D6`). `map_type` is Hercules `enum map_type`.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x01D6)]
pub struct NotifyMapProperty2Packet {
    pub map_type: u16,
}
