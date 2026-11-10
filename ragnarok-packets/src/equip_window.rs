//! View-equipment request and the "show my equipment" config toggle.
//!
//! `CZ_EQUIPWIN_MICROSCOPE` (`0x02D6`) is 6 bytes. `CZ_CONFIG` (`0x02D8`) is 10
//! bytes. Hercules `PACKETVER_MAIN_NUM >= 20250716` answers the microscope
//! request with `ZC_EQUIPWIN_MICROSCOPE` `0x0B37`, defined next to the other
//! 20250716 item layouts.

use crate::*;

/// Ask to see another player's equipment (`CZ_EQUIPWIN_MICROSCOPE`, `0x02D6`).
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02D6)]
pub struct RequestViewPlayerEquipPacket {
    pub account_id: AccountId,
}

/// Change a client config (`CZ_CONFIG`, `0x02D8`).
///
/// Type `0` is "open equipment window" (show my equipment to other players).
/// Value `0` hides it and `1` shows it.
#[derive(Debug, Clone, Packet, ClientPacket, MapServer)]
#[cfg_attr(feature = "interface", derive(rust_state::RustState, korangar_interface::element::StateElement))]
#[header(0x02D8)]
pub struct SetConfigurationPacket {
    pub config_type: u32,
    pub value: u32,
}

/// `CZ_CONFIG` type for the view-equipment checkbox.
pub const CONFIG_OPEN_EQUIPMENT_WINDOW: u32 = 0;

#[cfg(test)]
mod tests {
    use ragnarok_bytes::{ByteReader, ByteWriter};

    use super::*;

    #[test]
    fn view_equip_request_is_six_bytes() {
        let mut writer = ByteWriter::new();
        RequestViewPlayerEquipPacket {
            account_id: AccountId(0x0102_0304),
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice(), &[0xD6, 0x02, 0x04, 0x03, 0x02, 0x01]);
    }

    #[test]
    fn show_equipment_config_is_ten_bytes() {
        let mut writer = ByteWriter::new();
        SetConfigurationPacket {
            config_type: CONFIG_OPEN_EQUIPMENT_WINDOW,
            value: 1,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 10);
        assert_eq!(writer.as_slice(), &[0xD8, 0x02, 0, 0, 0, 0, 1, 0, 0, 0]);
        let mut reader = ByteReader::without_metadata(writer.as_slice());
        let packet = SetConfigurationPacket::packet_from_bytes(&mut reader).unwrap();
        assert!(reader.is_empty());
        assert_eq!(packet.config_type, 0);
        assert_eq!(packet.value, 1);
    }
}
