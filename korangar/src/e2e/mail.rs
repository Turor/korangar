//! Rodex mail with attachments, headless.
//!
//! [`offline_mail_exchange`] drives the write form the way the client does:
//! autopilot lines become the same `InputEvent`s the window buttons and the
//! inventory drop queue, the form state runs on Hercules-shaped acks decoded
//! by `ragnarok-packets`, and the requests are encoded to wire bytes. No
//! server. The live round trip is [`super::LogicSession::mail_round_trip`].

use korangar_networking::parse_rodex_attachments;
use ragnarok_bytes::{ByteReader, ByteWriter};
use ragnarok_packets::{
    AddRodexItemPacket, InventoryIndex, PacketExt, RemoveRodexItemPacket, RodexItemAddedPacket2, RodexItemRemovedPacket,
    RodexReceiverPacket, SendRodexPacket, rodex_ack_index,
};

use crate::autopilot::{Command, parse_command};
use crate::input::InputEvent;
use crate::state::mail_attach::{AttachCandidate, AttachStep};
use crate::state::mail_compose::MailComposeState;

/// What the offline exchange observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineMailReport {
    /// Debug names of the events the autopilot lines produced.
    pub scripted: Vec<String>,
    /// A 5-stack drop asked for an amount instead of sending.
    pub asked_amount: bool,
    /// `0x0A04` for 2 of inventory index 3.
    pub add_request: Vec<u8>,
    /// (index, count) on the form after the `0x0B3F` ack.
    pub attached: Vec<(u16, u16)>,
    pub summary: String,
    pub fee_text: String,
    /// `0x0A06` taking the stack back off.
    pub remove_request: Vec<u8>,
    pub attached_after_remove: usize,
    /// Receiver the send used (the name `0x0A51` confirmed).
    pub send_receiver: String,
    pub send_header: u16,
    pub send_zeny: i64,
    /// The window closes on `0x09ED` result 0.
    pub closed_on_success: bool,
    /// Item rows of a 2025-07-16 read (75-byte Turoran rows).
    pub read_attachments: Vec<(u32, u16)>,
}

fn encode<P: PacketExt>(packet: &P) -> Result<Vec<u8>, String> {
    let mut writer = ByteWriter::new();
    packet
        .packet_to_bytes(&mut writer)
        .map_err(|_| "packet did not encode".to_owned())?;
    Ok(writer.into_inner())
}

fn decode<P: PacketExt>(bytes: &[u8]) -> Result<P, String> {
    let mut reader = ByteReader::without_metadata(bytes);
    P::packet_from_bytes(&mut reader).map_err(|_| "packet did not decode".to_owned())
}

/// `ZC_ACK_ADD_ITEM_RODEX` (`0x0B3F`) as Hercules lays it out for a success.
fn add_ack(raw_index: u16, count: u16, item_id: u32, weight: u16) -> Vec<u8> {
    let mut bytes = 0x0B3Fu16.to_le_bytes().to_vec();
    bytes.push(0);
    bytes.extend(raw_index.to_le_bytes());
    bytes.extend(count.to_le_bytes());
    bytes.extend(item_id.to_le_bytes());
    bytes.extend([0, 1, 0]);
    bytes.extend([0u8; 16 + 25]);
    bytes.extend(weight.to_le_bytes());
    bytes.push(0);
    bytes.extend(0u32.to_le_bytes());
    bytes.extend([0, 0]);
    bytes
}

fn script(line: &str) -> Result<InputEvent, String> {
    match parse_command(line)? {
        Command::Event(event) | Command::Toggle(event) => Ok(event),
        _ => Err(format!("{line} is not an event")),
    }
}

/// Run the write form through attach, amount prompt, detach, re-attach and
/// send against encoded acks.
pub fn offline_mail_exchange() -> Result<OfflineMailReport, String> {
    let lines = [
        "event ComposeMailTo(\"bob\")",
        "event MailAttachItem(3, 0)",
        "event ConfirmMailAttachAmount",
        "event MailDetachItem(3)",
        "event SendMail",
    ];
    let scripted = lines
        .iter()
        .map(|line| script(line).map(|event| format!("{event:?}")))
        .collect::<Result<Vec<_>, _>>()?;

    let mut form = MailComposeState::default();
    form.begin("bob", "");
    form.title = "Potions".to_owned();
    form.opened(true);

    let potions = AttachCandidate {
        index: 3,
        item_id: 501,
        name: "Red Potion".to_owned(),
        texture: None,
        amount: 5,
        equipped: false,
    };
    let asked_amount = form.attachments.offer(form.write_open(), potions.clone()) == AttachStep::AskAmount;
    form.attachments.amount_text = "2".to_owned();
    let (index, count) = form.attachments.confirm_amount()?;
    let add_request = encode(&AddRodexItemPacket {
        index: InventoryIndex(index),
        count,
    })?;

    let ack = decode::<RodexItemAddedPacket2>(&add_ack(index + 2, count, 501, 14))?;
    form.attachments
        .attached(ack.result, rodex_ack_index(ack.raw_index), ack.count, ack.item_id, ack.weight)
        .ok_or("the add ack was not expected")?;
    form.zeny_text = "1000".to_owned();
    form.attachments_changed();
    let attached = form.attachments.items.iter().map(|item| (item.index, item.count)).collect();
    let summary = form.attachments.summary.clone();
    let fee_text = form.attachments.fee_text.clone();

    let (index, count) = form.attachments.detach(3)?;
    let remove_request = encode(&RemoveRodexItemPacket {
        index: InventoryIndex(index),
        count,
    })?;
    let mut removed = 0x0A07u16.to_le_bytes().to_vec();
    removed.push(1);
    removed.extend((index + 2).to_le_bytes());
    removed.extend(count.to_le_bytes());
    removed.extend(0u16.to_le_bytes());
    let removed = decode::<RodexItemRemovedPacket>(&removed)?;
    form.attachments
        .detached(
            removed.result == 1,
            rodex_ack_index(removed.raw_index),
            removed.count,
            removed.weight,
        )
        .ok_or("the remove ack was not expected")?;
    let attached_after_remove = form.attachments.count();

    // Attach a single potion again and send.
    let mut single = potions;
    single.amount = 1;
    if form.attachments.offer(form.write_open(), single) != (AttachStep::Send { index: 3, count: 1 }) {
        return Err("a single item should attach without a prompt".to_owned());
    }
    let ack = decode::<RodexItemAddedPacket2>(&add_ack(5, 1, 501, 7))?;
    form.attachments
        .attached(ack.result, rodex_ack_index(ack.raw_index), ack.count, ack.item_id, ack.weight);
    form.attachments_changed();

    let name = form.start_send()?;
    let mut receiver = 0x0A51u16.to_le_bytes().to_vec();
    receiver.extend(150_001u32.to_le_bytes());
    receiver.extend(4001u16.to_le_bytes());
    receiver.extend(99u16.to_le_bytes());
    let mut confirmed = b"Bob".to_vec();
    confirmed.resize(24, 0);
    receiver.extend(confirmed);
    let receiver = decode::<RodexReceiverPacket>(&receiver)?;
    if !name.eq_ignore_ascii_case(&receiver.name) {
        return Err("the check answered for another name".to_owned());
    }
    let request = form
        .receiver_checked(receiver.character_id, &receiver.name)
        .ok_or("no check was pending")??;
    let send = encode(&SendRodexPacket::compose(
        &request.receiver,
        "Alice",
        request.character_id,
        request.zeny,
        &request.title,
        &request.body,
    ))?;
    let send_header = u16::from_le_bytes([send[0], send[1]]);
    let send_zeny = i64::from_le_bytes(send[52..60].try_into().map_err(|_| "short send packet")?);
    let closed_on_success = form.send_result(0) == Some(true) && !form.write_open() && form.attachments.count() == 0;

    // A read on 2025-07-16: two 75-byte rows.
    let mut rows = vec![0u8; 150];
    rows[0] = 1;
    rows[2..6].copy_from_slice(&501u32.to_le_bytes());
    rows[75] = 1;
    rows[77..81].copy_from_slice(&1201u32.to_le_bytes());
    let read_attachments = parse_rodex_attachments(&rows, 2)
        .into_iter()
        .map(|row| (row.item_id, row.amount))
        .collect();

    Ok(OfflineMailReport {
        scripted,
        asked_amount,
        add_request,
        attached,
        summary,
        fee_text,
        remove_request,
        attached_after_remove,
        send_receiver: request.receiver,
        send_header,
        send_zeny,
        closed_on_success,
        read_attachments,
    })
}
