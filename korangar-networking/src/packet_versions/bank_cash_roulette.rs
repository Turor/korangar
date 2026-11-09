//! Bank, cash shop, and roulette packets shared by the 20190530, 20220406, and
//! 20250716 map profiles. Cash-shop open uses `0x0A2B` before 2020 and `0x0B6E`
//! after. Both are decoded into the same event.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    BankCheckPacket, BankCloseAckPacket, BankDepositAckPacket, BankOpenAckPacket, BankWithdrawAckPacket, CashShopBuyResultPacket,
    CashShopItemListPacket, CashShopOpenLegacyPacket, CashShopOpenPacket, CashShopOpenV2Packet, RouletteClaimAckPacket,
    RouletteCloseAckPacket, RouletteGenerateAckPacket, RouletteInfoPacket, RouletteOpenAckPacket,
};

use crate::event::NetworkEventList;
use crate::{CashShopOffer, NetworkEvent, RoulettePrize};

pub fn register_bank_cash_roulette<Callback>(
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

    packet_handler.register(|packet: CashShopOpenLegacyPacket| NetworkEvent::CashShopOpened {
        cash_points: packet.cash_points,
        kafra_points: packet.kafra_points,
        tab: 0,
    })?;
    let open = |cash_points, kafra_points, tab| NetworkEvent::CashShopOpened {
        cash_points,
        kafra_points,
        tab,
    };
    packet_handler.register(move |packet: CashShopOpenPacket| open(packet.cash_points, packet.kafra_points, packet.tab))?;
    packet_handler.register(move |packet: CashShopOpenV2Packet| open(packet.cash_points, packet.kafra_points, packet.tab))?;
    packet_handler.register(|packet: CashShopItemListPacket| NetworkEvent::CashShopItems {
        tab: packet.tab,
        items: packet
            .items
            .into_iter()
            .map(|item| CashShopOffer {
                item_id: item.item_id,
                price: item.price,
            })
            .collect(),
    })?;
    packet_handler.register(|packet: CashShopBuyResultPacket| NetworkEvent::CashShopBuyResult {
        item_id: packet.item_id,
        result: packet.result,
        cash_points: packet.cash_points,
        kafra_points: packet.kafra_points,
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
