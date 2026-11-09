//! Rodex / Mail (`0x09E7`–`0x09F6`, list `0x0AC2`).
//!
//! `0x0AC2` is the post-2017 mail list: one `is_end` byte, then rows of
//! open type, mail id, read flag, type, 24-byte sender, expiry, and a
//! length-prefixed title.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    NewMailStatusPacket, RodexDeleteAckPacket, RodexMailEntry, RodexMailListPacket, RodexReadPacket, RodexReadPacket2025,
    RodexWriteResultPacket, RodexZenyAckPacket,
};

use crate::event::{NetworkEvent, NetworkEventList};

pub fn register_rodex_packets<Callback>(packet_handler: &mut PacketHandler<NetworkEventList, Callback>) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: NewMailStatusPacket| NetworkEvent::NewMailStatus {
        available: packet.new_available != 0,
    })?;
    packet_handler.register(|packet: RodexMailListPacket| {
        let is_end = packet.is_end != 0;
        let summary = format_rodex_mails(is_end, &packet.mails);
        Some(NetworkEvent::RodexMailList { is_end, summary })
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
    packet_handler.register(|packet: RodexReadPacket| {
        let summary = format_rodex_read(packet.mail_id, packet.zeny, packet.item_count, &packet.body);
        Some(NetworkEvent::RodexRead {
            mail_id: packet.mail_id,
            zeny: packet.zeny,
            item_count: packet.item_count,
            summary,
        })
    })?;
    packet_handler.register(|packet: RodexReadPacket2025| {
        let summary = format_rodex_read(packet.mail_id, packet.zeny, packet.item_count, &packet.body);
        Some(NetworkEvent::RodexRead {
            mail_id: packet.mail_id,
            zeny: packet.zeny,
            item_count: packet.item_count,
            summary,
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

pub fn format_rodex_read(mail_id: i64, zeny: i64, item_count: u8, body: &str) -> String {
    let body = body.trim_end_matches('\0').trim();
    let body = if body.is_empty() { "(no message)" } else { body };
    let items = match item_count {
        0 => "no items".to_owned(),
        1 => "1 item".to_owned(),
        count => format!("{count} items"),
    };
    format!("Mail #{mail_id}: {body} (zeny {zeny}, {items})")
}

pub fn format_rodex_zeny(mail_id: i64, result: u8) -> String {
    match result {
        0 => format!("Took zeny from mail #{mail_id}."),
        _ => format!("Could not take zeny from mail #{mail_id} (result {result})."),
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

    use super::{format_rodex_mails, format_rodex_read, format_rodex_write_result, format_rodex_zeny, parse_rodex_mail_id};

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
        assert_eq!(format_rodex_read(7, 50, 2, "Hello\0"), "Mail #7: Hello (zeny 50, 2 items)");
        assert_eq!(format_rodex_read(1, 0, 0, ""), "Mail #1: (no message) (zeny 0, no items)");
        assert_eq!(format_rodex_zeny(7, 0), "Took zeny from mail #7.");
        assert_eq!(parse_rodex_mail_id(" 15 "), Some(15));
        assert_eq!(parse_rodex_mail_id(""), None);
        assert_eq!(parse_rodex_mail_id("nope"), None);
    }
}
