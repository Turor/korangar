//! Outgoing Rodex compose actions. Replies are registered in
//! `packet_versions::rodex`.

use ragnarok_packets::handler::PacketCallback;
use ragnarok_packets::{
    CancelRodexWritePacket, CheckRodexReceiverPacket, CheckRodexReceiverPacket2, OpenRodexWritePacket, SendRodexPacket,
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
