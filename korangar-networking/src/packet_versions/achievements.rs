//! Achievement list/update/reward (`0x0A23`, `0x0A24`, `0x0A26`) and the
//! attendance window (`0x0AE3`).

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    AchievementListPacket, AchievementRewardAckPacket, AchievementUpdatePacket, AttendanceDataPacket, AttendanceRewardAckPacket,
};

use crate::event::{NetworkEvent, NetworkEventList};

pub fn register_achievement_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: AchievementListPacket| NetworkEvent::AchievementList {
        total_score: packet.total_score,
        level: packet.level,
        experience: packet.acheivement_experience,
        experience_to_next_level: packet.acheivement_experience_to_next_level,
        entries: packet.acheivement_data,
    })?;
    packet_handler.register(|packet: AchievementUpdatePacket| NetworkEvent::AchievementUpdate {
        total_score: packet.total_score,
        level: packet.level,
        experience: packet.acheivement_experience,
        experience_to_next_level: packet.acheivement_experience_to_next_level,
        entry: packet.acheivement_data,
    })?;
    packet_handler.register(|packet: AchievementRewardAckPacket| NetworkEvent::AchievementReward {
        failed: packet.failed != 0,
        achievement_id: packet.achievement_id,
    })?;
    packet_handler.register(|packet: AttendanceDataPacket| NetworkEvent::Attendance {
        start_time: packet.start_time,
        end_time: packet.end_time,
        count: packet.attendance_count,
        rewards: packet.rewards,
    })?;
    packet_handler.register(|packet: AttendanceRewardAckPacket| NetworkEvent::AttendanceClaim { result: packet.result })?;
    Ok(())
}
