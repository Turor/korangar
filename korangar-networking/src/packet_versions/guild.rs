//! Guild create, invite, roster edits, skills, relations, chat, and emblems.
//!
//! `0x0167` is the create-guild result. Skill level-up stays on `0x0112`.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    ChangeGuildPacket, GuildChatReceivedPacket, GuildEmblemImagePacket, GuildInviteRequestPacket, GuildInviteResultPacket,
    GuildMakeResultPacket, GuildMemberAddedPacket, GuildMemberPositionChangedPacket, GuildPositionChangedPacket, GuildPositionInfoPacket,
    GuildRelationAddedPacket, GuildRelationRemovedPacket, GuildRelationsPacket, GuildSkillListPacket, LegacyChangeGuildPacket,
    LegacyGuildEmblemImagePacket,
};

use crate::event::{GuildMemberSnapshot, GuildSkillSnapshot, NetworkEvent, NetworkEventList};

fn positive_id(id: i32) -> u32 {
    if id < 0 { 0 } else { id as u32 }
}

pub fn register_guild_action_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: GuildMakeResultPacket| NetworkEvent::GuildMakeResult { result: packet.result })?;
    packet_handler.register(|packet: GuildInviteResultPacket| NetworkEvent::GuildInviteResult { result: packet.result })?;
    packet_handler.register(|packet: GuildInviteRequestPacket| NetworkEvent::GuildInviteRequest {
        guild_id: packet.guild_id,
        name: packet.guild_name,
    })?;
    packet_handler.register(|packet: GuildRelationsPacket| NetworkEvent::GuildRelations {
        relations: packet
            .relations
            .into_iter()
            .map(|row| (row.relation, positive_id(row.guild_id), row.name))
            .collect(),
    })?;
    packet_handler.register(|packet: GuildRelationRemovedPacket| NetworkEvent::GuildRelationRemoved {
        guild_id: packet.guild_id,
        opposition: packet.relation,
    })?;
    packet_handler.register(|packet: GuildRelationAddedPacket| NetworkEvent::GuildRelationAdded {
        opposition: packet.relation,
        guild_id: positive_id(packet.guild_id),
        name: packet.name,
    })?;
    packet_handler.register(|packet: GuildPositionInfoPacket| NetworkEvent::GuildPositionRights {
        positions: packet
            .positions
            .into_iter()
            .map(|row| (row.position_id, row.mode, row.ranking, row.pay_rate))
            .collect(),
    })?;
    packet_handler.register(|packet: GuildPositionChangedPacket| NetworkEvent::GuildPositionsUpdated {
        positions: packet
            .positions
            .into_iter()
            .map(|row| (row.position_id, row.mode, row.ranking, row.pay_rate, row.name))
            .collect(),
    })?;
    packet_handler.register(|packet: GuildMemberPositionChangedPacket| NetworkEvent::GuildMemberPositions {
        entries: packet
            .members
            .into_iter()
            .map(|row| (row.account_id, row.character_id, row.position_id))
            .collect(),
    })?;
    packet_handler.register(|packet: GuildSkillListPacket| NetworkEvent::GuildSkills {
        skill_points: i32::from(packet.skill_points),
        skills: packet
            .skills
            .into_iter()
            .map(|skill| GuildSkillSnapshot {
                skill_id: skill.skill_id.0,
                level: skill.level,
                sp: skill.sp,
                range: skill.range,
                name: skill.name,
                upgradable: skill.upgradable != 0,
            })
            .collect(),
    })?;
    packet_handler.register(|packet: GuildChatReceivedPacket| NetworkEvent::GuildChatMessage { text: packet.message })?;
    packet_handler.register(|packet: GuildEmblemImagePacket| NetworkEvent::GuildEmblemImage {
        result: packet.result,
        guild_id: positive_id(packet.guild_id),
        emblem_version: packet.emblem_id,
        bytes: packet.emblem_data,
    })?;
    packet_handler.register(|packet: LegacyGuildEmblemImagePacket| NetworkEvent::GuildEmblemImage {
        result: 2,
        guild_id: positive_id(packet.guild_id),
        emblem_version: packet.emblem_id,
        bytes: packet.emblem_data,
    })?;
    packet_handler.register(|packet: ChangeGuildPacket| NetworkEvent::GuildEmblemOnEntity {
        account_id: packet.account_id,
        guild_id: positive_id(packet.guild_id),
        emblem_version: packet.emblem_id,
    })?;
    packet_handler.register(|packet: LegacyChangeGuildPacket| NetworkEvent::GuildEmblemOnEntity {
        account_id: packet.account_id,
        guild_id: positive_id(packet.guild_id),
        emblem_version: u32::from(packet.emblem_id),
    })?;
    packet_handler.register(|packet: GuildMemberAddedPacket| NetworkEvent::GuildMemberAdded {
        member: GuildMemberSnapshot {
            account_id: packet.account_id,
            character_id: packet.character_id,
            name: packet.name,
            job: packet.job,
            level: packet.level,
            position_id: packet.position_id,
            online: packet.current_state != 0,
            contribution: packet.contribution_exp,
        },
    })?;
    Ok(())
}
