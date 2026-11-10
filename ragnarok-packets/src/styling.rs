//! Styling shop packets for PACKETVER 20250716.
//!
//! Hercules opens the shop with `ZC_UI_OPEN` (`0x0AE2`) type
//! [`STYLIST_UI_TYPE`], not a dedicated open packet. Buys use
//! `CZ_REQ_STYLE_CHANGE2` (`0x0AFC`) since PACKETVER >= 20180516. `0x0A46` is
//! the older buy packet, still registered on the server. `0x0A47` is the
//! result and `0x0A48` closes the UI.

use crate::*;

/// `out_ui_type` / `ZC_STYLIST_UI` (`openstylist`).
pub const STYLIST_UI_TYPE: u8 = 1;

/// `CZ_REQ_STYLE_CHANGE` (`0x0A46`, 14 bytes). Indexes are 1-based; 0 keeps
/// the current style. Superseded by [`RequestStyleChange2Packet`] on 20250716.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A46)]
pub struct RequestStyleChangePacket {
    pub head_palette: i16,
    pub head_style: i16,
    pub body_palette: i16,
    pub top_accessory: i16,
    pub mid_accessory: i16,
    pub bottom_accessory: i16,
}

/// `CZ_REQ_STYLE_CHANGE2` (`0x0AFC`, 16 bytes). Same indexes as
/// [`RequestStyleChangePacket`] plus an unused second-costume slot.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0AFC)]
pub struct RequestStyleChange2Packet {
    pub head_palette: i16,
    pub head_style: i16,
    pub body_palette: i16,
    pub top_accessory: i16,
    pub mid_accessory: i16,
    pub bottom_accessory: i16,
    pub body_style: i16,
}

/// `ZC_STYLE_CHANGE_RES` (`0x0A47`, 3 bytes). `0` is success.
#[derive(Debug, Clone, Packet, ServerPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A47)]
pub struct StyleChangeResultPacket {
    pub flag: u8,
}

/// `CZ_CLOSE_UI_STYLINGSHOP` (`0x0A48`, 2 bytes).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x0A48)]
pub struct CloseStyleShopPacket {}
