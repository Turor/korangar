//! Outgoing Rodex compose actions. Replies are registered in
//! `packet_versions::rodex`.

use ragnarok_packets::handler::PacketCallback;
use ragnarok_packets::{
    AddRodexItemPacket, CancelRodexWritePacket, CheckRodexReceiverPacket, CheckRodexReceiverPacket2, InventoryIndex, OpenRodexWritePacket,
    RemoveRodexItemPacket, SendRodexPacket,
};

use crate::server::NotConnectedError;
use crate::{NetworkingSystem, SupportedPacketVersion};

impl<Callback> NetworkingSystem<Callback>
where
    Callback: PacketCallback + Send,
{
    /// `CZ_REQ_OPEN_WRITE_MAIL` (`0x0A08`). The server answers `0x0A12`.
    pub fn open_rodex_write(&mut self, receiver_name: &str) -> Result<(), NotConnectedError> {
        let packet = OpenRodexWritePacket {
            receiver_name: receiver_name.trim().to_owned(),
        };
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(packet)
            }
        }
    }

    /// `CZ_REQ_CANCEL_WRITE_MAIL` (`0x0A03`).
    pub fn cancel_rodex_write(&mut self) -> Result<(), NotConnectedError> {
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CancelRodexWritePacket {})
            }
        }
    }

    /// `CZ_REQ_ADD_ITEM_RODEX` (`0x0A04`): attach `count` of the stack at
    /// inventory `index` to the open write form. The server answers
    /// `0x0A05` (before 2020-09-16) or `0x0B3F`.
    pub fn add_rodex_item(&mut self, index: u16, count: u16) -> Result<(), NotConnectedError> {
        let packet = AddRodexItemPacket {
            index: InventoryIndex(index),
            count,
        };
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(packet)
            }
        }
    }

    /// `CZ_REQ_REMOVE_RODEX_ITEM` (`0x0A06`): take `count` of the attached
    /// stack at inventory `index` off the form. The server answers `0x0A07`.
    pub fn remove_rodex_item(&mut self, index: u16, count: u16) -> Result<(), NotConnectedError> {
        let packet = RemoveRodexItemPacket {
            index: InventoryIndex(index),
            count,
        };
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(packet)
            }
        }
    }

    /// Ask who `name` is (`ZC_CHECKNAME` `0x0A51`). Hercules only accepts a
    /// send to the last name checked. `0x0B97` from PACKETVER_MAIN_NUM
    /// 20201104, `0x0A13` before.
    pub fn check_rodex_receiver(&mut self, name: &str) -> Result<(), NotConnectedError> {
        let name = name.trim().to_owned();
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20190530 => self.send_map_server_packet(CheckRodexReceiverPacket { name }),
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(CheckRodexReceiverPacket2 { name, own_character: 0 })
            }
        }
    }

    /// `CZ_REQ_SEND_RODEX2` (`0x0A6E`) to a receiver confirmed by
    /// [`Self::check_rodex_receiver`].
    pub fn send_rodex_mail(
        &mut self,
        receiver_name: &str,
        sender_name: &str,
        receiver_character_id: u32,
        zeny: i64,
        title: &str,
        body: &str,
    ) -> Result<(), NotConnectedError> {
        let packet = SendRodexPacket::compose(
            receiver_name.trim(),
            sender_name,
            receiver_character_id,
            zeny,
            title.trim(),
            body,
        );
        match self.map_server_packet_version()? {
            SupportedPacketVersion::_20220406 | SupportedPacketVersion::_20190530 | SupportedPacketVersion::_20250716 => {
                self.send_map_server_packet(packet)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::packet_handlers::process_one;
    use crate::{NetworkEvent, SupportedPacketVersion};

    const VERSIONS: [SupportedPacketVersion; 3] = [
        SupportedPacketVersion::_20190530,
        SupportedPacketVersion::_20220406,
        SupportedPacketVersion::_20250716,
    ];

    fn add_ack(header: u16, result: u8, raw_index: u16) -> Vec<u8> {
        let mut bytes = header.to_le_bytes().to_vec();
        bytes.push(result);
        bytes.extend(raw_index.to_le_bytes());
        bytes.extend(2u16.to_le_bytes());
        bytes.extend(501u32.to_le_bytes());
        // type, identified, damaged (+ refine on 0x0A05), cards, options.
        let middle = if header == 0x0A05 { 4 } else { 3 };
        bytes.extend(std::iter::repeat_n(0u8, middle + 16 + 25));
        bytes.extend(14u16.to_le_bytes());
        bytes.push(0);
        bytes.extend(0u32.to_le_bytes());
        if header == 0x0B3F {
            bytes.extend([0, 0]);
        }
        bytes
    }

    #[test]
    fn attachment_acks_become_events_on_every_profile() {
        for version in VERSIONS {
            for (header, size) in [(0x0A05u16, 63), (0x0B3F, 64)] {
                let bytes = add_ack(header, 0, 5);
                assert_eq!(bytes.len(), size);
                let events = process_one(version, &bytes);
                assert!(
                    matches!(events.as_slice(), [NetworkEvent::RodexItemAttached {
                        result: 0,
                        index: Some(3),
                        count: 2,
                        item_id: 501,
                        weight: 14
                    }]),
                    "{header:04x} on {version:?}"
                );
                let events = process_one(version, &add_ack(header, 4, 0));
                assert!(matches!(events.as_slice(), [NetworkEvent::RodexItemAttached {
                    result: 4,
                    index: None,
                    ..
                }]));
            }

            let mut removed = 0x0A07u16.to_le_bytes().to_vec();
            removed.extend([1, 5, 0, 2, 0, 0, 0]);
            let events = process_one(version, &removed);
            assert!(matches!(events.as_slice(), [NetworkEvent::RodexItemDetached {
                removed: true,
                index: Some(3),
                count: 2,
                weight: 0
            }]));
        }
    }
}
