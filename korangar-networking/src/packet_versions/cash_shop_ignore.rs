//! Incoming cash-shop frames are parsed and dropped. The client does not send
//! cash-shop requests. Variable-length opcodes with no struct (`0x0287` point
//! item list, `0x08CA` scheduler list, `0x0288` buy) stay in the framing tables
//! and are skipped by length on the framed map profiles.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    CashItemDeletePacket, CashPointUpdatePacket, CashShopBuyResultPacket, CashShopItemListPacket, CashShopOpenLegacyPacket,
    CashShopOpenPacket, CashShopOpenV2Packet, CashTimeCounterPacket,
};

use crate::event::NetworkEventList;

pub fn register_ignored_cash_shop<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    // `register_noop` consumes the frame and yields no `NetworkEvent`.
    packet_handler.register_noop::<CashShopOpenLegacyPacket>()?;
    packet_handler.register_noop::<CashShopOpenPacket>()?;
    packet_handler.register_noop::<CashShopOpenV2Packet>()?;
    packet_handler.register_noop::<CashShopItemListPacket>()?;
    packet_handler.register_noop::<CashShopBuyResultPacket>()?;
    packet_handler.register_noop::<CashPointUpdatePacket>()?;
    packet_handler.register_noop::<CashTimeCounterPacket>()?;
    packet_handler.register_noop::<CashItemDeletePacket>()?;
    Ok(())
}
