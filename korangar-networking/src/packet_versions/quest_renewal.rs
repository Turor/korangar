//! PACKETVER >= 20181017 quest packets (`0x0AFF`, `0x0B0C`, `0x0AFE`).
//!
//! The list and the add packet keep `mob_id` for the monster-name lookup.
//! The hunt update has no mob id; `huntIdent2` is the objective index.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    HuntingQuestUpdatePacket4, ObjectiveDetails1, Quest, QuestAddedPacket4, QuestDetails, QuestHuntSub, QuestListInfo4, QuestListPacket4,
    QuestMissionInfo,
};

use crate::event::{HuntIndexUpdate, NetworkEvent, NetworkEventList};

pub fn register_quest_renewal_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: QuestListPacket4| NetworkEvent::QuestList {
        quests: packet.quests.into_iter().map(quest_from_list).collect(),
    })?;
    packet_handler.register(|packet: QuestAddedPacket4| {
        let QuestAddedPacket4 {
            quest_id,
            active,
            start_time,
            expire_time,
            objective_count,
            objective_details,
        } = packet;
        NetworkEvent::QuestNotification {
            quest_id,
            active: active != 0,
            start_time,
            expire_time,
            objectives: objectives_from_added(objective_count, objective_details),
        }
    })?;
    packet_handler.register(|packet: HuntingQuestUpdatePacket4| NetworkEvent::HuntingQuestIndexUpdate {
        updates: packet
            .objective_details
            .into_iter()
            .map(|objective| HuntIndexUpdate {
                quest_id: objective.quest_id,
                objective_index: objective.hunt_ident2,
                total_count: objective.total_count,
                current_count: objective.current_count,
            })
            .collect(),
    })?;
    Ok(())
}

fn quest_from_list(quest: QuestListInfo4) -> Quest {
    let objective_details = quest.objective_details.into_iter().map(details_from_mission).collect();
    Quest {
        quest_id: quest.quest_id,
        active: quest.active,
        remaining_time: quest.remaining_time,
        expire_time: quest.expire_time,
        objective_count: quest.objective_count,
        objective_details,
    }
}

fn details_from_mission(objective: QuestMissionInfo) -> QuestDetails {
    QuestDetails {
        hunt_identification: objective.hunt_ident,
        objective_type: objective.mob_type,
        mob_id: objective.mob_id,
        minimum_level: objective.minimum_level,
        maximum_level: objective.maximum_level,
        kill_count: objective.kill_count,
        total_count: objective.total_count,
        mob_name: objective.mob_name,
    }
}

fn objectives_from_added(objective_count: u16, details: [QuestHuntSub; 3]) -> Vec<ObjectiveDetails1> {
    details
        .into_iter()
        .take(objective_count as usize)
        .map(|objective| ObjectiveDetails1 {
            hunt_identification: objective.hunt_ident,
            objective_type: objective.mob_type,
            mob_id: objective.mob_id,
            minimum_level: objective.minimum_level,
            maximum_level: objective.maximum_level,
            mob_count: objective.hunt_count,
            mob_name: objective.mob_name,
        })
        .collect()
}
