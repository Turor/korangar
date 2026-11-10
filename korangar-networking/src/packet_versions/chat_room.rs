//! Incoming chat room packets (`0x00D6`..`0x00E1`).

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    ChatRoomChangedPacket, ChatRoomDestroyedPacket, ChatRoomEnteredPacket, ChatRoomEntryPacket, ChatRoomMemberJoinedPacket,
    ChatRoomMemberLeftPacket, ChatRoomRefusedPacket, ChatRoomRoleChangedPacket, CreateChatRoomResultPacket,
};

use crate::event::{ChatRoomMember, NetworkEvent, NetworkEventList};

pub fn register_chat_room_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: CreateChatRoomResultPacket| NetworkEvent::ChatRoomCreateResult { result: packet.result })?;
    packet_handler.register(|packet: ChatRoomEntryPacket| NetworkEvent::ChatRoomBoard {
        owner_id: packet.owner_id,
        chat_id: packet.chat_id,
        limit: packet.limit,
        users: packet.users,
        public: packet.public != 0,
        title: packet.title,
    })?;
    packet_handler.register(|packet: ChatRoomChangedPacket| NetworkEvent::ChatRoomBoard {
        owner_id: packet.owner_id,
        chat_id: packet.chat_id,
        limit: packet.limit,
        users: packet.users,
        public: packet.public != 0,
        title: packet.title,
    })?;
    packet_handler.register(|packet: ChatRoomDestroyedPacket| NetworkEvent::ChatRoomDestroyed { chat_id: packet.chat_id })?;
    packet_handler.register(|packet: ChatRoomRefusedPacket| NetworkEvent::ChatRoomRefused { result: packet.result })?;
    packet_handler.register(|packet: ChatRoomEnteredPacket| NetworkEvent::ChatRoomEntered {
        chat_id: packet.chat_id,
        members: packet
            .members
            .into_iter()
            .map(|member| ChatRoomMember {
                name: member.name,
                owner: member.role == 0,
            })
            .collect(),
    })?;
    packet_handler.register(|packet: ChatRoomMemberJoinedPacket| NetworkEvent::ChatRoomMemberJoined {
        users: packet.users,
        name: packet.name,
    })?;
    packet_handler.register(|packet: ChatRoomMemberLeftPacket| NetworkEvent::ChatRoomMemberLeft {
        users: packet.users,
        name: packet.name,
        kicked: packet.kicked != 0,
    })?;
    packet_handler.register(|packet: ChatRoomRoleChangedPacket| NetworkEvent::ChatRoomRoleChanged {
        name: packet.name,
        owner: packet.role == 0,
    })?;
    Ok(())
}
