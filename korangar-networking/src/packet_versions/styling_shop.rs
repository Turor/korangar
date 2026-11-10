//! Styling shop and sprite-look packets for PACKETVER 20250716.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{JobId, STYLIST_UI_TYPE, SpriteChangePacket, SpriteChangeType, StyleChangeResultPacket, UiOpenPacket};

use crate::NetworkEvent;
use crate::event::NetworkEventList;

pub fn sprite_change_event(packet: SpriteChangePacket) -> Option<NetworkEvent> {
    match packet.sprite_type {
        SpriteChangeType::Base => Some(NetworkEvent::ChangeJob {
            account_id: packet.account_id,
            job_id: JobId(packet.value as u16),
        }),
        SpriteChangeType::Hair => Some(NetworkEvent::ChangeHair {
            account_id: packet.account_id,
            hair_id: packet.value,
        }),
        SpriteChangeType::HairCollor => Some(NetworkEvent::ChangeHairColor {
            account_id: packet.account_id,
            color: packet.value,
        }),
        SpriteChangeType::ClothesColor => Some(NetworkEvent::ChangeClothColor {
            account_id: packet.account_id,
            color: packet.value,
        }),
        SpriteChangeType::HeadTop => Some(NetworkEvent::ChangeHeadTop {
            account_id: packet.account_id,
            view_id: packet.value,
        }),
        SpriteChangeType::HeadMiddle => Some(NetworkEvent::ChangeHeadMid {
            account_id: packet.account_id,
            view_id: packet.value,
        }),
        SpriteChangeType::HeadBottom => Some(NetworkEvent::ChangeHeadBottom {
            account_id: packet.account_id,
            view_id: packet.value,
        }),
        _ => None,
    }
}

/// Style result, and on profiles that do not already handle `ZC_UI_OPEN`, the
/// stylist open (`ui_type` 1).
pub fn register_style_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
    with_ui_open: bool,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: StyleChangeResultPacket| NetworkEvent::StylingShopResult { success: packet.flag == 0 })?;
    if with_ui_open {
        packet_handler.register(|packet: UiOpenPacket| {
            if packet.ui_type == STYLIST_UI_TYPE {
                Some(NetworkEvent::OpenStylingShop)
            } else if packet.ui_type == ragnarok_packets::ATTENDANCE_UI_TYPE {
                let data = packet.data.max(0) as u32;
                Some(NetworkEvent::AttendanceOpened {
                    days_attended: data / 10,
                    claimed_today: !data.is_multiple_of(10),
                })
            } else {
                None
            }
        })?;
    }
    Ok(())
}
