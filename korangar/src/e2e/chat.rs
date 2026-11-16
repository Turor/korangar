//! Chat unread markers and partner whisper tabs for the logic-mode harness.
//!
//! Typed lines go through [`InputSystem`] and the chat box handler. Tab
//! changes call the same `select_tab` the tab strip uses. Live sessions send
//! `CZ_WHISPER` (`0x0096`) only through the TEST connection the harness opened.

use korangar_networking::MessageColor;
use ragnarok_bytes::{ByteReader, ByteWriter};
use ragnarok_packets::{PacketExt, WhisperMessagePacket};

use super::logic::LogicSession;
use crate::e2e::LiveConnect;
use crate::interface::windows::{ChatWindowState, WindowClass};
use crate::state::ChatMessage;
use crate::state::chat::{OutgoingChat, format_unread_badge, parse_outgoing_chat};

/// One tab the chat strip would draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatTabView {
    pub label: String,
    pub unread: u32,
    pub badge: Option<String>,
    pub closable: bool,
    pub selected: bool,
}

/// A line the active tab shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleLine {
    pub text: String,
    pub error: bool,
}

/// Chat window as the logic session currently has it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatView {
    pub window_open: bool,
    pub active: String,
    pub armed: bool,
    pub whisper_target: String,
    pub tabs: Vec<ChatTabView>,
    pub visible: Vec<VisibleLine>,
}

/// `CZ_WHISPER` after a real encode and decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhisperIntent {
    pub header: u16,
    pub target: String,
    pub text: String,
}

/// Chat log plus [`LogicSession`]. Offline has no socket.
pub struct ChatSession {
    logic: LogicSession,
    window: ChatWindowState,
    messages: Vec<ChatMessage>,
    player_name: String,
}

impl ChatSession {
    pub fn offline(player_name: &str) -> Self {
        let mut session = Self::from_logic(LogicSession::offline(), player_name.to_owned());
        session.logic.show_window(WindowClass::Chat);
        session
    }

    pub fn start_live() -> LiveConnect<Self> {
        match LogicSession::start_live() {
            LiveConnect::Skip(reason) => LiveConnect::Skip(reason),
            LiveConnect::Invalid(error) => LiveConnect::Invalid(error),
            LiveConnect::Ready(logic) => LiveConnect::Ready(Self::from_logic(logic, String::new())),
        }
    }

    fn from_logic(logic: LogicSession, player_name: String) -> Self {
        Self {
            logic,
            window: ChatWindowState::default(),
            messages: Vec::new(),
            player_name,
        }
    }

    pub fn username(&self) -> &str {
        self.logic.username()
    }

    pub fn character_name(&self) -> &str {
        self.logic.character_names().first().map(String::as_str).unwrap_or("")
    }

    /// Log in, enter the map, and open Chat. The player name comes from TEST.
    pub fn enter_world(&mut self) -> Result<(), String> {
        self.logic.login_to_character_select()?;
        self.logic.enter_world()?;
        if let Some(name) = self.logic.character_names().first().filter(|name| !name.is_empty()) {
            self.player_name.clone_from(name);
        }
        self.logic.show_window(WindowClass::Chat);
        Ok(())
    }

    pub fn window_open(&self) -> bool {
        self.logic.window_names().iter().any(|name| name == "Chat")
    }

    pub fn sent_headers(&self) -> Vec<u16> {
        self.logic.sent_headers()
    }

    pub fn view(&self) -> ChatView {
        let tabs = self
            .window
            .tab_slots(&self.messages, &self.player_name)
            .into_iter()
            .map(|slot| ChatTabView {
                badge: format_unread_badge(slot.unread),
                label: slot.label,
                unread: slot.unread,
                closable: slot.closable,
                selected: slot.selected,
            })
            .collect();
        let visible = self
            .window
            .visible_lines(&self.messages)
            .into_iter()
            .map(|(text, color)| VisibleLine {
                text,
                error: matches!(color, MessageColor::Error),
            })
            .collect();
        ChatView {
            window_open: self.window_open(),
            active: self.window.active_tab().label(),
            armed: self.window.whisper_target_armed(),
            whisper_target: self.window.whisper_target_text().to_owned(),
            tabs,
            visible,
        }
    }

    pub fn feed_public(&mut self, text: &str) {
        self.push_line(text, MessageColor::Public);
    }

    pub fn feed_party(&mut self, text: &str) {
        self.push_line(text, MessageColor::Party);
    }

    pub fn feed_system(&mut self, text: &str) {
        self.push_line(text, MessageColor::Server);
    }

    /// The player's own public echo (`Name : text`). Never unread.
    pub fn feed_own_public(&mut self, text: &str) {
        self.push_line(&format!("{} : {text}", self.player_name), MessageColor::Public);
    }

    pub fn feed_whisper(&mut self, sender: &str, text: &str) {
        self.window.record_incoming_whisper(&mut self.messages, sender, text);
    }

    /// Context-menu Whisper: open or reuse the partner tab and arm it.
    pub fn focus_partner(&mut self, name: &str) {
        self.window.focus_partner(name.to_owned(), self.messages.len());
    }

    /// Click a tab by its label. Uses the tab strip's select path.
    pub fn inject_tab(&mut self, label: &str) -> Result<(), String> {
        let slots = self.window.tab_slots(&self.messages, &self.player_name);
        let Some(slot) = slots
            .iter()
            .find(|slot| slot.label.eq_ignore_ascii_case(label))
            .map(|slot| slot.slot)
        else {
            return Err(format!("no chat tab {label}"));
        };
        if self.window.select_slot(slot, self.messages.len()) {
            Ok(())
        } else {
            Err(format!("chat tab slot {slot} is empty"))
        }
    }

    /// Characters from one input frame, applied to the chat box. Enter is `\r`.
    ///
    /// A whisper encodes `0x0096` and, on a live session, sends it.
    pub fn inject_text_enter(&mut self, text: &str) -> Result<Option<WhisperIntent>, String> {
        self.logic.buffer_text(text);
        self.logic.buffer_text("\r");
        let characters = self.logic.take_frame_characters();
        if characters.is_empty() {
            return Err("injected text did not reach the input frame".to_owned());
        }
        let mut line: Option<String> = None;
        for character in characters {
            if let Some(submitted) = self.window.apply_chat_character(character) {
                line = Some(submitted);
            }
        }
        let Some(line) = line else {
            return Err("enter did not submit the chat box".to_owned());
        };
        let outgoing = parse_outgoing_chat(
            &line,
            self.window.whisper_target_text(),
            self.window.whisper_target_armed(),
            self.window.sends_guild_chat(),
        );
        match outgoing {
            OutgoingChat::Whisper { target, text } => {
                let (target, text) = self.window.begin_whisper(target, text, self.messages.len());
                let intent = whisper_intent(&target, &text)?;
                self.logic.dispatch_whisper(&target, &text)?;
                Ok(Some(intent))
            }
            OutgoingChat::Public(_) | OutgoingChat::Guild(_) => Ok(None),
        }
    }

    /// `ZC_ACK_WHISPER`. Non-zero results use the error color on the partner
    /// tab.
    pub fn acknowledge(&mut self, result: u8) {
        self.window.record_whisper_ack(&mut self.messages, result);
    }

    /// Close the partner chip. The chat window stays open.
    pub fn close_partner(&mut self, name: &str) -> Result<(), String> {
        if self.window.close_named_partner(name, self.messages.len()) {
            Ok(())
        } else {
            Err(format!("no partner tab {name}"))
        }
    }

    fn push_line(&mut self, text: &str, color: MessageColor) {
        self.messages.push(ChatMessage::new(text.to_owned(), color));
    }
}

fn whisper_intent(target: &str, text: &str) -> Result<WhisperIntent, String> {
    let packet = WhisperMessagePacket::new(target.to_owned(), text.to_owned());
    let mut writer = ByteWriter::new();
    packet
        .packet_to_bytes(&mut writer)
        .map_err(|_| "whisper packet did not encode".to_owned())?;
    let bytes = writer.into_inner();
    if bytes.len() < 2 {
        return Err("whisper packet is too short".to_owned());
    }
    let header = u16::from_le_bytes([bytes[0], bytes[1]]);
    let mut reader = ByteReader::without_metadata(&bytes);
    let parsed = WhisperMessagePacket::packet_from_bytes(&mut reader).map_err(|_| "whisper packet did not decode".to_owned())?;
    if !reader.is_empty() {
        return Err("whisper packet left trailing bytes".to_owned());
    }
    Ok(WhisperIntent {
        header,
        target: parsed.target,
        text: parsed.message,
    })
}
