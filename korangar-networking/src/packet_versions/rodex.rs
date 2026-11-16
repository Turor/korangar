//! Rodex / Mail (`0x09E7`–`0x09F6`, list `0x0AC2`, write-form item acks
//! `0x0A05` / `0x0B3F` / `0x0A07`).
//!
//! `0x0AC2` is the post-2017 mail list: one `is_end` byte, then rows of
//! open type, mail id, read flag, type, 24-byte sender, expiry, and a
//! length-prefixed title.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    NewMailStatusPacket, RodexDeleteAckPacket, RodexItemAckPacket, RodexItemAddedPacket, RodexItemAddedPacket2, RodexItemRemovedPacket,
    RodexMailEntry, RodexMailListPacket, RodexReadPacket, RodexReadPacket2025, RodexReceiverPacket, RodexWriteOpenedPacket,
    RodexWriteResultPacket, RodexZenyAckPacket, rodex_ack_index,
};

use crate::event::{NetworkEvent, NetworkEventList, RodexAttachment, RodexMailSummary};

pub fn register_rodex_packets<Callback>(packet_handler: &mut PacketHandler<NetworkEventList, Callback>) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: NewMailStatusPacket| NetworkEvent::NewMailStatus {
        available: packet.new_available != 0,
    })?;
    packet_handler.register(|packet: RodexMailListPacket| {
        let is_end = packet.is_end != 0;
        let next_mail_id = rodex_page_cursor(&packet.mails);
        let summary = format_rodex_mails(is_end, &packet.mails);
        let mails = packet
            .mails
            .iter()
            .map(|mail| RodexMailSummary {
                mail_id: mail.mail_id,
                sender: mail.sender_name.clone(),
                title: mail.title.clone(),
                is_read: mail.is_read != 0,
            })
            .collect();
        Some(NetworkEvent::RodexMailList {
            is_end,
            next_mail_id,
            summary,
            mails,
        })
    })?;
    packet_handler.register(|packet: RodexWriteOpenedPacket| NetworkEvent::RodexWriteOpened {
        receiver_name: packet.receiver_name,
        allowed: packet.result == 1,
    })?;
    packet_handler.register(|packet: RodexReceiverPacket| NetworkEvent::RodexReceiver {
        character_id: packet.character_id,
        class: packet.class,
        base_level: packet.base_level,
        name: packet.name,
    })?;
    packet_handler.register(|packet: RodexItemAddedPacket| NetworkEvent::RodexItemAttached {
        result: packet.result,
        index: rodex_ack_index(packet.raw_index),
        count: packet.count,
        item_id: packet.item_id,
        weight: packet.weight,
    })?;
    packet_handler.register(|packet: RodexItemAddedPacket2| NetworkEvent::RodexItemAttached {
        result: packet.result,
        index: rodex_ack_index(packet.raw_index),
        count: packet.count,
        item_id: packet.item_id,
        weight: packet.weight,
    })?;
    packet_handler.register(|packet: RodexItemRemovedPacket| NetworkEvent::RodexItemDetached {
        removed: packet.result == 1,
        index: rodex_ack_index(packet.raw_index),
        count: packet.count,
        weight: packet.weight,
    })?;
    packet_handler.register(|packet: RodexWriteResultPacket| NetworkEvent::RodexWriteResult {
        result: packet.result,
        summary: format_rodex_write_result(packet.result),
    })?;
    packet_handler.register(|packet: RodexDeleteAckPacket| NetworkEvent::RodexDelete {
        open_type: packet.open_type,
        mail_id: packet.mail_id,
    })?;
    packet_handler.register(|packet: RodexZenyAckPacket| NetworkEvent::RodexZeny {
        mail_id: packet.mail_id,
        result: packet.result,
        summary: format_rodex_zeny(packet.mail_id, packet.result),
    })?;
    packet_handler.register(|packet: RodexItemAckPacket| NetworkEvent::RodexItems {
        mail_id: packet.mail_id,
        result: packet.result,
        summary: format_rodex_items(packet.mail_id, packet.result),
    })?;
    packet_handler.register(|packet: RodexReadPacket| {
        let summary = format_rodex_read(packet.mail_id, packet.zeny, packet.item_count, &packet.body, &packet.items);
        Some(NetworkEvent::RodexRead {
            mail_id: packet.mail_id,
            zeny: packet.zeny,
            item_count: packet.item_count,
            summary,
            attachments: parse_rodex_attachments(&packet.items, usize::from(packet.item_count)),
        })
    })?;
    packet_handler.register(|packet: RodexReadPacket2025| {
        let summary = format_rodex_read(packet.mail_id, packet.zeny, packet.item_count, &packet.body, &packet.items);
        Some(NetworkEvent::RodexRead {
            mail_id: packet.mail_id,
            zeny: packet.zeny,
            item_count: packet.item_count,
            summary,
            attachments: parse_rodex_attachments(&packet.items, usize::from(packet.item_count)),
        })
    })?;
    Ok(())
}

pub fn format_rodex_mails(is_end: bool, mails: &[RodexMailEntry]) -> String {
    if mails.is_empty() {
        return if is_end {
            "Inbox is empty.".to_owned()
        } else {
            "No mails on this page.".to_owned()
        };
    }

    let mut lines = Vec::with_capacity(mails.len() + 1);
    for mail in mails {
        let read = if mail.is_read == 0 { "unread" } else { "read" };
        let mut tags = Vec::new();
        if mail.mail_type & 0x02 != 0 {
            tags.push("zeny");
        }
        if mail.mail_type & 0x04 != 0 {
            tags.push("item");
        }
        if mail.mail_type & 0x08 != 0 {
            tags.push("npc");
        }
        let tag = if tags.is_empty() {
            String::new()
        } else {
            format!(" ({})", tags.join(", "))
        };
        lines.push(format!(
            "#{id} from {sender}: {title} [{read}]{tag}",
            id = mail.mail_id,
            sender = mail.sender_name,
            title = mail.title,
            read = read,
        ));
    }
    if !is_end {
        lines.push("More mail on the next page.".to_owned());
    }
    lines.join("\n")
}

pub fn format_rodex_write_result(result: u8) -> String {
    match result {
        0 => "Mail sent.".to_owned(),
        _ => format!("Mail was not sent (result {result})."),
    }
}

/// Last mail id on a list page. Passing it to `0x0AC1` asks for the next page.
pub fn rodex_page_cursor(mails: &[RodexMailEntry]) -> i64 {
    mails.last().map(|mail| mail.mail_id).unwrap_or(0)
}

/// `ZC_ACK_READ_RODEX_SUB` row sizes this client can split. Every layout
/// starts with the `u16` amount and the `u32` item id.
///
/// Turoran Hercules sends three forge options after the five random options:
/// 74 bytes on 2019-05-30 (refine before the cards) and 75 on 2025-07-16
/// (refine and grade at the end). Stock Hercules has no forge block: 59 and
/// 60 bytes.
const ATTACHMENT_ROW_SIZES: [usize; 4] = [74, 75, 59, 60];

/// Attachment rows after the mail text.
///
/// Any other length stays a count so a wrong stride cannot invent item ids.
pub fn rodex_attachment_labels(item_count: u8, items: &[u8]) -> String {
    let count = usize::from(item_count);
    if count == 0 {
        return "no items".to_owned();
    }
    let parsed = parse_rodex_attachments(items, count);
    if parsed.is_empty() {
        return match count {
            1 => "1 item".to_owned(),
            count => format!("{count} items"),
        };
    }
    parsed
        .into_iter()
        .map(|attachment| format!("item {} x{}", attachment.item_id, attachment.amount))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Item id and amount of each attachment row, or nothing when the block is
/// not `count` rows of a known size.
pub fn parse_rodex_attachments(items: &[u8], count: usize) -> Vec<RodexAttachment> {
    if count == 0 || !items.len().is_multiple_of(count) {
        return Vec::new();
    }
    let stride = items.len() / count;
    if !ATTACHMENT_ROW_SIZES.contains(&stride) {
        return Vec::new();
    }
    items
        .chunks(stride)
        .filter_map(|chunk| {
            let amount = u16::from_le_bytes([chunk[0], chunk[1]]);
            let item_id = u32::from_le_bytes([chunk[2], chunk[3], chunk[4], chunk[5]]);
            (item_id != 0).then_some(RodexAttachment { item_id, amount })
        })
        .collect()
}

pub fn format_rodex_read(mail_id: i64, zeny: i64, item_count: u8, body: &str, items: &[u8]) -> String {
    let body = body.trim_end_matches('\0').trim();
    let body = if body.is_empty() { "(no message)" } else { body };
    let items = rodex_attachment_labels(item_count, items);
    format!("Mail #{mail_id}: {body} (zeny {zeny}, {items})")
}

pub fn format_rodex_zeny(mail_id: i64, result: u8) -> String {
    match result {
        0 => format!("Took zeny from mail #{mail_id}."),
        _ => format!("Could not take zeny from mail #{mail_id} (result {result})."),
    }
}

pub fn format_rodex_items(mail_id: i64, result: u8) -> String {
    match result {
        0 => format!("Took items from mail #{mail_id}."),
        _ => format!("Could not take items from mail #{mail_id} (result {result})."),
    }
}

pub fn parse_rodex_mail_id(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::RodexMailEntry;

    use super::{
        format_rodex_items, format_rodex_mails, format_rodex_read, format_rodex_write_result, format_rodex_zeny, parse_rodex_attachments,
        parse_rodex_mail_id, rodex_attachment_labels, rodex_page_cursor,
    };
    use crate::event::RodexAttachment;

    fn sample(title: &str, is_read: u8, mail_type: u8) -> RodexMailEntry {
        RodexMailEntry {
            open_type: 0,
            mail_id: 42,
            is_read,
            mail_type,
            sender_name: "Kafra".to_owned(),
            expire_time: 1,
            title_length: title.len() as i16,
            title: title.to_owned(),
        }
    }

    #[test]
    fn formats_unread_item_mail_and_empty_inbox() {
        let text = format_rodex_mails(true, &[sample("Welcome", 0, 0x04)]);
        assert!(text.contains("#42 from Kafra: Welcome [unread] (item)"));
        assert!(!text.contains("next page"));
        assert_eq!(format_rodex_mails(true, &[]), "Inbox is empty.");
        assert_eq!(format_rodex_mails(false, &[]), "No mails on this page.");
        let more = format_rodex_mails(false, &[sample("Page", 1, 0)]);
        assert!(more.contains("[read]"));
        assert!(more.ends_with("More mail on the next page."));
    }

    #[test]
    fn formats_results_and_parses_mail_ids() {
        assert_eq!(format_rodex_write_result(0), "Mail sent.");
        assert!(format_rodex_write_result(1).contains("result 1"));
        assert_eq!(format_rodex_read(7, 50, 2, "Hello\0", &[]), "Mail #7: Hello (zeny 50, 2 items)");
        assert_eq!(format_rodex_read(1, 0, 0, "", &[]), "Mail #1: (no message) (zeny 0, no items)");
        assert_eq!(format_rodex_zeny(7, 0), "Took zeny from mail #7.");
        assert_eq!(format_rodex_items(7, 0), "Took items from mail #7.");
        assert!(format_rodex_items(7, 1).contains("result 1"));
        assert_eq!(rodex_page_cursor(&[sample("A", 0, 0)]), 42);
        assert_eq!(rodex_page_cursor(&[]), 0);

        let mut row = vec![0u8; 59];
        row[0] = 3;
        row[2] = 0xF5;
        row[3] = 0x01;
        assert_eq!(rodex_attachment_labels(1, &row), "item 501 x3");
        // Turoran rows carry forge options: 75 bytes on 2025-07-16, 74 before.
        for size in [74, 75] {
            let mut rows = vec![0u8; size * 2];
            rows[0] = 1;
            rows[2..6].copy_from_slice(&1201u32.to_le_bytes());
            rows[size] = 20;
            rows[size + 2..size + 6].copy_from_slice(&7060u32.to_le_bytes());
            assert_eq!(rodex_attachment_labels(2, &rows), "item 1201 x1, item 7060 x20");
            assert_eq!(parse_rodex_attachments(&rows, 2), vec![
                RodexAttachment { item_id: 1201, amount: 1 },
                RodexAttachment { item_id: 7060, amount: 20 },
            ]);
        }
        assert!(parse_rodex_attachments(&[0u8; 70], 1).is_empty());
        assert_eq!(rodex_attachment_labels(1, &[1, 2, 3]), "1 item");
        assert_eq!(parse_rodex_mail_id(" 15 "), Some(15));
        assert_eq!(parse_rodex_mail_id(""), None);
        assert_eq!(parse_rodex_mail_id("nope"), None);
    }
}
