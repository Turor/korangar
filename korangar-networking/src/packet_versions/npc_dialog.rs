//! NPC dialog extras: numeric and string input, cutin, progress bar, sound, and
//! BGM.
//!
//! Layouts follow Hercules `PACKETVER` 20250716 RE and the roBrowser packet
//! table. `0x0142` / `0x01D4` open an input. The client answers with `0x0143`
//! or `0x01D5`. `0x02F0` starts a bar. `0x02F1` tells the server it finished or
//! that the player walked. Hercules does not parse a client `0x02F2`. An early
//! `0x02F1` sets the script state to `END`. `0x07FE` is the fixed BGM packet.
//! Packetver 20250716 sends `0x0B8C` instead.

#[cfg(test)]
use ragnarok_bytes::ByteReader;
use ragnarok_bytes::ByteWriter;
use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    DisplayImagePacket, EntityId, ImageLocation, InputEditDialogPacket, InputEditDialogStringPacket, NpcProgressCancelPacket,
    NpcProgressCompletePacket, NpcProgressPacket, NpcSoundPacket, OpenEditDialogPacket, OpenEditDialogStringPacket, Packet, PacketExt,
    PlayNpcBgmExPacket, PlayNpcBgmPacket,
};

use crate::event::{NetworkEvent, NetworkEventList};

pub fn register_npc_dialog_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    register_npc_dialog_packets_except_bgm(packet_handler)?;
    packet_handler.register(|packet: PlayNpcBgmPacket| NetworkEvent::NpcBgm {
        track: packet.track,
        play_type: 0,
    })?;
    Ok(())
}

/// Same dialog packets as [`register_npc_dialog_packets`], but BGM is `0x0B8C`.
pub fn register_npc_dialog_packets_20250716<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    register_npc_dialog_packets_except_bgm(packet_handler)?;
    packet_handler.register(|packet: PlayNpcBgmExPacket| NetworkEvent::NpcBgm {
        track: packet.track,
        play_type: packet.play_type,
    })?;
    Ok(())
}

fn register_npc_dialog_packets_except_bgm<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: OpenEditDialogPacket| NetworkEvent::NpcNumberInput { npc_id: packet.npc_id })?;
    packet_handler.register(|packet: OpenEditDialogStringPacket| NetworkEvent::NpcTextInput { npc_id: packet.npc_id })?;
    packet_handler.register(|packet: DisplayImagePacket| NetworkEvent::NpcCutin {
        image_name: packet.image_name,
        location: packet.location,
    })?;
    packet_handler.register(|packet: NpcProgressPacket| NetworkEvent::NpcProgress {
        color: packet.color,
        seconds: packet.seconds,
    })?;
    packet_handler.register(|_packet: NpcProgressCancelPacket| NetworkEvent::NpcProgressCancel)?;
    packet_handler.register(|packet: NpcSoundPacket| NetworkEvent::NpcSound {
        file_name: packet.file_name,
        action: packet.action,
        term: packet.term,
        npc_id: packet.npc_id,
    })?;
    Ok(())
}

/// What the client sends after the player confirms or cancels an NPC input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcInputReply {
    Number { npc_id: EntityId, value: i32 },
    Text { npc_id: EntityId, text: String },
}

/// Empty and unparsable numbers become `0`, matching roBrowser `parseInt(...) |
/// 0`. Cancel sends `0` for a number and an empty string for text so the script
/// does not stall.
pub fn decide_npc_input(npc_id: EntityId, number: bool, text: &str, cancel: bool) -> NpcInputReply {
    if number {
        let value = match cancel {
            true => 0,
            false => parse_npc_number(text),
        };
        NpcInputReply::Number { npc_id, value }
    } else {
        let text = match cancel {
            true => String::new(),
            false => text.to_owned(),
        };
        NpcInputReply::Text { npc_id, text }
    }
}

pub fn parse_npc_number(text: &str) -> i32 {
    text.trim().parse().unwrap_or(0)
}

/// Spinner step. An empty box starts at 0. The value saturates at `i32` bounds.
pub fn step_number(current: &str, delta: i32) -> String {
    parse_npc_number(current).saturating_add(delta).to_string()
}

pub fn npc_input_reply_bytes(reply: &NpcInputReply) -> Vec<u8> {
    match reply {
        NpcInputReply::Number { npc_id, value } => wire(&InputEditDialogPacket::new(*npc_id, *value)),
        NpcInputReply::Text { npc_id, text } => wire(&InputEditDialogStringPacket::new(*npc_id, text.clone())),
    }
}

/// Enter or Space while a dialog is open and the input box is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogHotkey {
    Next,
    Close,
}

pub fn dialog_hotkey(
    awaiting_next: bool,
    awaiting_close: bool,
    awaiting_choice: bool,
    input_open: bool,
    characters: &[char],
) -> Option<DialogHotkey> {
    if input_open || awaiting_choice {
        return None;
    }

    let pressed = characters
        .iter()
        .any(|character| *character == '\n' || *character == '\r' || *character == ' ');
    if !pressed {
        return None;
    }

    if awaiting_next {
        Some(DialogHotkey::Next)
    } else if awaiting_close {
        Some(DialogHotkey::Close)
    } else {
        None
    }
}

/// `true` when `now_ms` is at or past the end. A zero duration finishes
/// immediately.
pub fn progress_complete(started_ms: u32, duration_ms: u32, now_ms: u32) -> bool {
    if duration_ms == 0 {
        return true;
    }
    now_ms.wrapping_sub(started_ms) >= duration_ms
}

pub fn progress_fraction(started_ms: u32, duration_ms: u32, now_ms: u32) -> f32 {
    if duration_ms == 0 {
        return 1.0;
    }
    let elapsed = now_ms.wrapping_sub(started_ms) as f32;
    (elapsed / duration_ms as f32).clamp(0.0, 1.0)
}

/// Which progress packets to send. Walking and a finished bar both send only
/// `0x02F1`. The client does not send `0x02F2`. `ServerCancelled` is the same
/// early complete after the server already sent `0x02F2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressOutcome {
    StillRunning,
    Finished,
    Moved,
    ServerCancelled,
}

pub fn progress_outcome(active: bool, complete: bool, moved: bool, server_cancel: bool) -> ProgressOutcome {
    if !active {
        return ProgressOutcome::StillRunning;
    }
    if server_cancel {
        return ProgressOutcome::ServerCancelled;
    }
    if complete {
        return ProgressOutcome::Finished;
    }
    if moved {
        return ProgressOutcome::Moved;
    }
    ProgressOutcome::StillRunning
}

pub fn packets_for_progress(outcome: ProgressOutcome) -> Vec<Vec<u8>> {
    let complete = wire(&NpcProgressCompletePacket::new());
    match outcome {
        ProgressOutcome::StillRunning => Vec::new(),
        ProgressOutcome::Finished | ProgressOutcome::ServerCancelled | ProgressOutcome::Moved => vec![complete],
    }
}

/// `PLAY_BGM_STOP` (2) restores the map track. Loop and once play `track`.
pub fn apply_npc_bgm(play_type: u8, track: &str) -> Option<String> {
    if play_type == 2 {
        return None;
    }
    bgm_track_path(track)
}

/// First cutin file that exists. `exists` receives the path under the GRF root.
pub fn existing_cutin_texture(name: &str, mut exists: impl FnMut(&str) -> bool) -> Option<String> {
    cutin_texture_candidates(name)
        .into_iter()
        .find(|candidate| exists(&format!("data\\texture\\{candidate}")))
}

/// Texture paths relative to `data\texture\`. A name with an extension is used
/// as-is. Otherwise `.bmp` is tried before `.png`.
pub fn cutin_texture_candidates(name: &str) -> Vec<String> {
    let name = name.trim_matches('\0').trim();
    if name.is_empty() {
        return Vec::new();
    }

    let base = format!("유저인터페이스\\illust\\{name}");
    if name.contains('.') {
        vec![base]
    } else {
        vec![format!("{base}.bmp"), format!("{base}.png")]
    }
}

/// `None` removes the cutin (`255` or an unknown clear). `3` and `4` share the
/// centered anchor.
pub fn cutin_anchor(location: ImageLocation) -> Option<u8> {
    match location {
        ImageLocation::BottomLeft => Some(0),
        ImageLocation::BottomMiddle => Some(1),
        ImageLocation::BottomRight => Some(2),
        ImageLocation::MiddleFloating | ImageLocation::MiddleColorless => Some(3),
        ImageLocation::ClearAll => None,
    }
}

/// File name passed to the audio engine, which prefixes `data\wav\`.
/// `None` means there is nothing to play. Action 2 is not a path; the caller
/// skips playback.
pub fn sound_effect_path(name: &str) -> Option<String> {
    let name = name.trim_matches('\0').trim();
    if name.is_empty() {
        return None;
    }

    let mut path = name.to_owned();
    if !path.to_ascii_lowercase().ends_with(".wav") {
        path.push_str(".wav");
    }
    Some(path)
}

/// Filesystem track for background music playback.
/// An empty name restores the map track.
pub fn bgm_track_path(name: &str) -> Option<String> {
    let name = name.trim_matches('\0').trim();
    if name.is_empty() {
        return None;
    }

    let mut path = name.to_owned();
    let lower = path.to_ascii_lowercase();
    if !lower.ends_with(".mp3") && !lower.ends_with(".wav") {
        path.push_str(".mp3");
    }
    if !path.contains('\\') && !path.contains('/') {
        path = format!("bgm\\{path}");
    }
    Some(path)
}

fn wire(packet: &impl Packet) -> Vec<u8> {
    let mut writer = ByteWriter::new();
    packet.packet_to_bytes(&mut writer).expect("packet write");
    writer.into_inner()
}

#[cfg(test)]
fn read_packet<P: Packet>(bytes: &[u8]) -> P {
    let mut reader = ByteReader::without_metadata(bytes);
    P::packet_from_bytes(&mut reader).expect("packet read")
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{
        DisplayImagePacket, EntityId, ImageLocation, InputEditDialogPacket, InputEditDialogStringPacket, NpcProgressCancelPacket,
        NpcProgressCompletePacket, NpcProgressPacket, NpcSoundPacket, OpenEditDialogPacket, OpenEditDialogStringPacket, PlayNpcBgmExPacket,
        PlayNpcBgmPacket,
    };

    use super::{
        DialogHotkey, NpcInputReply, ProgressOutcome, apply_npc_bgm, bgm_track_path, cutin_anchor, cutin_texture_candidates,
        decide_npc_input, dialog_hotkey, existing_cutin_texture, npc_input_reply_bytes, packets_for_progress, parse_npc_number,
        progress_complete, progress_fraction, progress_outcome, read_packet, sound_effect_path, step_number, wire,
    };

    #[test]
    fn open_edit_dialogs_are_six_bytes() {
        let number = [0x42, 0x01, 0x78, 0x56, 0x34, 0x12];
        let parsed: OpenEditDialogPacket = read_packet(&number);
        assert_eq!(parsed.npc_id, EntityId(0x1234_5678));
        assert_eq!(wire(&OpenEditDialogPacket::new(EntityId(0x1234_5678))), number);

        let text = [0xD4, 0x01, 0x02, 0x00, 0x00, 0x00];
        let parsed: OpenEditDialogStringPacket = read_packet(&text);
        assert_eq!(parsed.npc_id, EntityId(2));
        assert_eq!(wire(&OpenEditDialogStringPacket::new(EntityId(2))), text);
    }

    #[test]
    fn number_reply_is_npc_id_and_i32() {
        let positive = wire(&InputEditDialogPacket::new(EntityId(7), 5));
        assert_eq!(positive, [0x43, 0x01, 7, 0, 0, 0, 5, 0, 0, 0]);
        let negative = wire(&InputEditDialogPacket::new(EntityId(7), -1));
        assert_eq!(negative, [0x43, 0x01, 7, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]);
        let parsed: InputEditDialogPacket = read_packet(&negative);
        assert_eq!(parsed.value, -1);
    }

    #[test]
    fn string_reply_includes_length_npc_id_and_nul() {
        let bytes = wire(&InputEditDialogStringPacket::new(EntityId(1), "hi".to_owned()));
        assert_eq!(&bytes[0..2], &[0xD5, 0x01]);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), bytes.len() as u16);
        assert_eq!(&bytes[4..8], &[1, 0, 0, 0]);
        assert_eq!(&bytes[8..], b"hi\0");
        assert!(bytes.len() >= 9);

        let parsed: InputEditDialogStringPacket = read_packet(&bytes);
        assert_eq!(parsed.npc_id, EntityId(1));
        assert_eq!(parsed.text, "hi");

        let empty = wire(&InputEditDialogStringPacket::new(EntityId(4), String::new()));
        assert!(empty.len() >= 9);
        assert_eq!(*empty.last().unwrap(), 0);
    }

    #[test]
    fn input_state_machine_writes_reply_bytes() {
        let npc_id = EntityId(9);
        let number = decide_npc_input(npc_id, true, "12", false);
        assert_eq!(number, NpcInputReply::Number { npc_id, value: 12 });
        assert_eq!(npc_input_reply_bytes(&number), wire(&InputEditDialogPacket::new(npc_id, 12)));

        let cancelled_number = decide_npc_input(npc_id, true, "12", true);
        assert_eq!(
            npc_input_reply_bytes(&cancelled_number),
            wire(&InputEditDialogPacket::new(npc_id, 0))
        );

        let text = decide_npc_input(npc_id, false, "hi", false);
        assert_eq!(
            npc_input_reply_bytes(&text),
            wire(&InputEditDialogStringPacket::new(npc_id, "hi".to_owned()))
        );

        let cancelled_text = decide_npc_input(npc_id, false, "hi", true);
        assert_eq!(
            npc_input_reply_bytes(&cancelled_text),
            wire(&InputEditDialogStringPacket::new(npc_id, String::new()))
        );

        assert_eq!(parse_npc_number("  "), 0);
        assert_eq!(parse_npc_number("nope"), 0);
        assert_eq!(step_number("", 1), "1");
        assert_eq!(step_number("2", -5), "-3");
        assert_eq!(step_number(&i32::MAX.to_string(), 1), i32::MAX.to_string());
    }

    #[test]
    fn dialog_hotkey_prefers_next_then_close() {
        assert_eq!(dialog_hotkey(true, true, false, false, &['\r']), Some(DialogHotkey::Next));
        assert_eq!(dialog_hotkey(false, true, false, false, &[' ']), Some(DialogHotkey::Close));
        assert_eq!(dialog_hotkey(true, false, false, true, &['\n']), None);
        assert_eq!(dialog_hotkey(true, false, true, false, &['\n']), None);
        assert_eq!(dialog_hotkey(false, false, false, false, &['x']), None);
    }

    #[test]
    fn progress_packet_and_outcomes() {
        let bytes = wire(&NpcProgressPacket::new(0x00FF_0000, 3));
        assert_eq!(bytes, [0xF0, 0x02, 0x00, 0x00, 0xFF, 0x00, 3, 0, 0, 0]);
        let parsed: NpcProgressPacket = read_packet(&bytes);
        assert_eq!(parsed.color, 0x00FF_0000);
        assert_eq!((parsed.color >> 16) & 0xFF, 0xFF);

        assert_eq!(wire(&NpcProgressCompletePacket::new()), [0xF1, 0x02]);
        assert_eq!(wire(&NpcProgressCancelPacket::new()), [0xF2, 0x02]);

        assert!(!progress_complete(1000, 2000, 2999));
        assert!(progress_complete(1000, 2000, 3000));
        assert!(progress_complete(1000, 0, 1000));
        assert!((progress_fraction(1000, 2000, 2000) - 0.5).abs() < f32::EPSILON);

        assert_eq!(progress_outcome(true, false, false, false), ProgressOutcome::StillRunning);
        assert_eq!(packets_for_progress(ProgressOutcome::Finished), vec![vec![0xF1, 0x02]]);
        assert_eq!(packets_for_progress(progress_outcome(true, false, true, false)), vec![vec![
            0xF1, 0x02
        ]]);
        assert_eq!(packets_for_progress(progress_outcome(true, true, false, true)), vec![vec![
            0xF1, 0x02
        ]]);
        assert!(packets_for_progress(progress_outcome(false, true, true, true)).is_empty());
    }

    #[test]
    fn cutin_sound_and_bgm_layouts() {
        let cutin = wire(&DisplayImagePacket::new("kafra.bmp".to_owned(), ImageLocation::BottomLeft));
        assert_eq!(cutin.len(), 67);
        assert_eq!(&cutin[0..2], &[0xB3, 0x01]);
        assert_eq!(cutin[66], 0);
        let clear = wire(&DisplayImagePacket::new(String::new(), ImageLocation::ClearAll));
        assert_eq!(clear[66], 255);
        assert!(cutin_anchor(ImageLocation::ClearAll).is_none());
        assert_eq!(cutin_anchor(ImageLocation::MiddleColorless), Some(3));
        assert_eq!(cutin_texture_candidates("kafra"), vec![
            "유저인터페이스\\illust\\kafra.bmp".to_owned(),
            "유저인터페이스\\illust\\kafra.png".to_owned(),
        ]);
        assert!(cutin_texture_candidates("").is_empty());
        assert_eq!(cutin_texture_candidates("kafra.bmp"), vec![
            "유저인터페이스\\illust\\kafra.bmp".to_owned()
        ]);

        let sound = wire(&NpcSoundPacket::new("bell".to_owned(), 0, 0, EntityId(3)));
        assert_eq!(sound.len(), 35);
        assert_eq!(&sound[0..2], &[0xD3, 0x01]);
        assert_eq!(sound[2], b'b');
        assert_eq!(sound[26], 0);
        assert_eq!(sound_effect_path("bell"), Some("bell.wav".to_owned()));
        assert_eq!(sound_effect_path("bell.wav"), Some("bell.wav".to_owned()));
        assert_eq!(sound_effect_path("\0"), None);

        let bgm = wire(&PlayNpcBgmPacket::new("town".to_owned()));
        assert_eq!(bgm.len(), 26);
        assert_eq!(&bgm[0..2], &[0xFE, 0x07]);
        assert_eq!(bgm_track_path("town"), Some("bgm\\town.mp3".to_owned()));
        assert_eq!(bgm_track_path("bgm\\01.mp3"), Some("bgm\\01.mp3".to_owned()));
        assert_eq!(bgm_track_path(""), None);

        let modern = wire(&PlayNpcBgmExPacket::new(0, "town".to_owned()));
        assert_eq!(modern, [0x8C, 0x0B, 10, 0, 0, b't', b'o', b'w', b'n', 0]);
        let parsed: PlayNpcBgmExPacket = read_packet(&modern);
        assert_eq!(parsed.play_type, 0);
        assert_eq!(parsed.track, "town");
        let stop = wire(&PlayNpcBgmExPacket::new(2, String::new()));
        assert_eq!(stop, [0x8C, 0x0B, 6, 0, 2, 0]);
        let parsed: PlayNpcBgmExPacket = read_packet(&stop);
        assert_eq!(parsed.play_type, 2);
        assert!(parsed.track.is_empty());
        assert_eq!(apply_npc_bgm(0, "town"), Some("bgm\\town.mp3".to_owned()));
        assert_eq!(apply_npc_bgm(1, "town"), Some("bgm\\town.mp3".to_owned()));
        assert_eq!(apply_npc_bgm(2, "town"), None);

        assert_eq!(
            existing_cutin_texture("kafra", |path| path.ends_with("kafra.png")),
            Some("유저인터페이스\\illust\\kafra.png".to_owned())
        );
        assert!(existing_cutin_texture("kafra", |_| false).is_none());
    }
}
