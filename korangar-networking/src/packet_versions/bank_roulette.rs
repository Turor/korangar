//! Bank and roulette packets shared by the 20190530, 20220406, and 20250716
//! map profiles.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    BankCheckPacket, BankCloseAckPacket, BankDepositAckPacket, BankOpenAckPacket, BankWithdrawAckPacket, RouletteClaimAckPacket,
    RouletteCloseAckPacket, RouletteGenerateAckPacket, RouletteInfoPacket, RouletteOpenAckPacket,
};

use crate::event::NetworkEventList;
use crate::{NetworkEvent, RoulettePrize};

pub fn register_bank_and_roulette<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: BankOpenAckPacket| NetworkEvent::BankOpened { unknown: packet.unknown })?;
    packet_handler.register(|packet: BankCloseAckPacket| NetworkEvent::BankClosed { unknown: packet.unknown })?;
    packet_handler.register(|packet: BankCheckPacket| NetworkEvent::BankBalance {
        money: packet.money,
        reason: packet.reason,
    })?;
    packet_handler.register(|packet: BankDepositAckPacket| NetworkEvent::BankDeposit {
        reason: packet.reason,
        money: packet.money,
        zeny: packet.zeny,
    })?;
    packet_handler.register(|packet: BankWithdrawAckPacket| NetworkEvent::BankWithdraw {
        reason: packet.reason,
        money: packet.money,
        zeny: packet.zeny,
    })?;

    packet_handler.register(|packet: RouletteOpenAckPacket| NetworkEvent::RouletteOpened {
        result: packet.result,
        serial: packet.serial,
        step: packet.step,
        index: packet.index,
        bonus_item_id: packet.bonus_item_id,
        gold: packet.gold,
        silver: packet.silver,
        bronze: packet.bronze,
    })?;
    packet_handler.register(|packet: RouletteInfoPacket| NetworkEvent::RoulettePrizes {
        serial: packet.serial,
        prizes: packet
            .prizes
            .into_iter()
            .map(|prize| RoulettePrize {
                row: prize.row,
                position: prize.position,
                item_id: prize.item_id,
                count: prize.count,
            })
            .collect(),
    })?;
    packet_handler.register(|packet: RouletteCloseAckPacket| NetworkEvent::RouletteClosed { result: packet.result })?;
    packet_handler.register(|packet: RouletteGenerateAckPacket| NetworkEvent::RouletteGenerated {
        result: packet.result,
        step: packet.step,
        index: packet.index,
        bonus_item_id: packet.bonus_item_id,
        gold: packet.gold,
        silver: packet.silver,
        bronze: packet.bronze,
    })?;
    packet_handler.register(|packet: RouletteClaimAckPacket| NetworkEvent::RouletteClaimed {
        result: packet.result,
        item_id: packet.item_id,
    })?;
    Ok(())
}
