//! Character creation draft, slot pages, bans, and the timed-delete reducer.
//!
//! Packet bytes live in `ragnarok-packets`. This module only decides which
//! fields go on the wire and how delete acknowledgements change a slot.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use korangar_interface::element::StateElement;
use ragnarok_packets::{CharacterId, Sex};
use rust_state::RustState;

use crate::graphics::Texture;
use crate::world::{AnimationData, JOB_NOVICE, JOB_SUMMONER};

pub const HUMAN_HAIR_STYLES: u16 = 23;
pub const DORAM_HAIR_STYLES: u16 = 6;
pub const HUMAN_HAIR_COLOR_MAX: u16 = 8;
pub const DORAM_HAIR_COLOR_MAX: u16 = 7;
pub const CONFIRMATION_LEN: usize = 6;

/// One banned character from `0x020D`.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct CharacterBan {
    pub character_id: CharacterId,
    pub until: String,
}

/// Fields of `0x0A39` after the name has been checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreationPacketFields {
    pub slot: u8,
    pub name: String,
    pub hair_color: u16,
    pub hair_style: u16,
    pub start_job: u16,
    pub sex: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeletionPhase {
    None,
    Waiting { remaining_secs: i64 },
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReserveEffect {
    Scheduled { delete_at: i32, message: &'static str },
    Rejected { message: &'static str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcceptEffect {
    Removed { message: &'static str },
    Rejected { message: &'static str },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelEffect {
    Cleared { message: &'static str },
    Rejected { message: &'static str },
}

/// Draft shown by the creation window, plus select-screen page and delete
/// state.
#[derive(RustState, StateElement)]
pub struct CharacterScreen {
    pub slot: u16,
    pub hair_style: u16,
    pub hair_color: u16,
    /// `0` female, `1` male. Only these two values are written on the wire.
    pub sex: u8,
    pub sex_locked: bool,
    pub doram: bool,
    pub page: u32,
    pub page_count: u32,
    pub page_label: String,
    pub status: String,
    pub delete_confirmation: String,
    pub bans: Vec<CharacterBan>,
    /// A `0x0B72` list arrived on this character server connection.
    #[hidden_element]
    list_received: bool,
    #[hidden_element]
    pending_delete: Option<CharacterId>,
    #[hidden_element]
    pub preview: Option<Arc<AnimationData>>,
    #[hidden_element]
    pub head_textures: Option<Vec<Arc<Texture>>>,
    #[hidden_element]
    pub hair_thumbs: Vec<Option<Arc<Texture>>>,
    #[hidden_element]
    pub color_thumbs: Vec<Option<Arc<Texture>>>,
    #[hidden_element]
    requested_preview: String,
    #[hidden_element]
    pub pending_entity: u32,
    #[hidden_element]
    pub preview_generation: u32,
    #[hidden_element]
    pub head_sprite: String,
    #[hidden_element]
    pub applied_head: String,
    #[hidden_element]
    loaded_thumbs: String,
}

impl Default for CharacterScreen {
    fn default() -> Self {
        Self {
            slot: 0,
            hair_style: 1,
            hair_color: 0,
            sex: 1,
            sex_locked: true,
            doram: false,
            page: 0,
            page_count: 0,
            page_label: String::new(),
            status: String::new(),
            delete_confirmation: String::new(),
            bans: Vec::new(),
            list_received: false,
            pending_delete: None,
            preview: None,
            head_textures: None,
            hair_thumbs: Vec::new(),
            color_thumbs: Vec::new(),
            requested_preview: String::new(),
            pending_entity: 0,
            preview_generation: 1,
            head_sprite: String::new(),
            applied_head: String::new(),
            loaded_thumbs: String::new(),
        }
    }
}

impl CharacterScreen {
    pub fn begin_creation(&mut self, slot: usize, account_sex: Sex) {
        let (sex, locked) = sex_from_account(account_sex);
        self.slot = slot as u16;
        self.hair_style = 1;
        self.hair_color = 0;
        self.sex = sex;
        self.sex_locked = locked;
        self.doram = false;
        self.status.clear();
        self.clear_preview();
    }

    pub fn reset_for_server(&mut self) {
        self.page = 0;
        self.page_count = 0;
        self.bans.clear();
        self.list_received = false;
        self.status.clear();
        self.pending_delete = None;
        self.delete_confirmation.clear();
        self.refresh_page_label();
        self.clear_preview();
    }

    /// Hercules `char_send_HC_ACK_CHARINFO_PER_PAGE` follows a list of exactly
    /// three characters with an empty `0x0B72` tail. That tail must not wipe
    /// the list. An account with no characters gets one empty packet, which is
    /// still applied because no list arrived before it.
    pub fn is_list_tail(&self, incoming_is_empty: bool) -> bool {
        incoming_is_empty && self.list_received
    }

    pub fn on_character_list(&mut self) {
        // `0x020D` arrives before `0x0B72`. Bans stay until the next character server.
        self.list_received = true;
        self.page = 0;
        self.refresh_page_label();
    }

    pub fn set_page_count(&mut self, count: u32) {
        self.page_count = count;
        let pages = count.max(1);
        if self.page >= pages {
            self.page = pages - 1;
        }
        self.refresh_page_label();
    }

    pub fn set_bans(&mut self, bans: Vec<CharacterBan>) {
        self.bans = bans.into_iter().filter(|ban| ban.character_id.0 != 0).collect();
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = message.into();
    }

    pub fn set_hair_style(&mut self, style: u16) {
        self.hair_style = style.clamp(1, self.style_count());
    }

    pub fn set_hair_color(&mut self, color: u16) {
        self.hair_color = color.min(self.color_max());
    }

    pub fn set_doram(&mut self, doram: bool) {
        self.doram = doram;
        self.hair_style = self.hair_style.clamp(1, self.style_count());
        self.hair_color = self.hair_color.min(self.color_max());
    }

    pub fn set_sex(&mut self, sex: u8) {
        if self.sex_locked {
            return;
        }
        if sex <= 1 {
            self.sex = sex;
        }
    }

    pub fn style_count(&self) -> u16 {
        match self.doram {
            true => DORAM_HAIR_STYLES,
            false => HUMAN_HAIR_STYLES,
        }
    }

    pub fn color_max(&self) -> u16 {
        match self.doram {
            true => DORAM_HAIR_COLOR_MAX,
            false => HUMAN_HAIR_COLOR_MAX,
        }
    }

    pub fn color_count(&self) -> u16 {
        self.color_max() + 1
    }

    pub fn packet_fields(&self, name: &str) -> CreationPacketFields {
        CreationPacketFields {
            slot: self.slot as u8,
            name: name.to_owned(),
            hair_color: self.hair_color.min(self.color_max()),
            hair_style: self.hair_style.clamp(1, self.style_count()),
            start_job: match self.doram {
                true => JOB_SUMMONER,
                false => JOB_NOVICE,
            },
            sex: match self.sex {
                0 => 0,
                _ => 1,
            },
        }
    }

    pub fn can_page_back(&self) -> bool {
        self.page_count > 1 && self.page > 0
    }

    pub fn can_page_forward(&self) -> bool {
        self.page_count > 1 && self.page + 1 < self.page_count
    }

    pub fn step_page(&mut self, delta: i32) {
        if delta < 0 && self.can_page_back() {
            self.page -= 1;
        } else if delta > 0 && self.can_page_forward() {
            self.page += 1;
        }
        self.refresh_page_label();
    }

    pub fn ban_label(&self, character_id: CharacterId) -> Option<&str> {
        self.bans
            .iter()
            .find(|ban| ban.character_id == character_id)
            .map(|ban| ban.until.as_str())
    }

    pub fn begin_delete_confirmation(&mut self, character_id: CharacterId) {
        self.pending_delete = Some(character_id);
        self.delete_confirmation.clear();
    }

    pub fn pending_delete_id(&self) -> Option<CharacterId> {
        self.pending_delete
    }

    pub fn preview_key(&self) -> String {
        format!("{}:{}:{}:{}", self.doram as u8, self.sex, self.hair_style, self.hair_color)
    }

    pub fn thumb_key(&self) -> String {
        format!("{}:{}", self.doram as u8, self.sex)
    }

    pub fn take_preview_request(&mut self) -> Option<String> {
        let key = self.preview_key();
        if self.requested_preview == key {
            return None;
        }
        self.requested_preview = key.clone();
        self.preview = None;
        self.head_textures = None;
        self.applied_head.clear();
        self.preview_generation = self.preview_generation.wrapping_add(1);
        Some(key)
    }

    pub fn note_pending_entity(&mut self, entity_id: u32, head_sprite: String) {
        self.pending_entity = entity_id;
        self.head_sprite = head_sprite;
    }

    pub fn install_preview(&mut self, entity_id: u32, animation: Arc<AnimationData>) -> bool {
        if entity_id != self.pending_entity || entity_id == 0 {
            return false;
        }
        self.preview = Some(animation);
        self.head_textures = None;
        self.applied_head.clear();
        true
    }

    pub fn thumbs_need_load(&self) -> bool {
        self.loaded_thumbs != self.thumb_key()
    }

    pub fn store_thumbs(&mut self, hair: Vec<Option<Arc<Texture>>>, color: Vec<Option<Arc<Texture>>>) {
        self.hair_thumbs = hair;
        self.color_thumbs = color;
        self.loaded_thumbs = self.thumb_key();
    }

    pub fn head_recolor_key(&self) -> String {
        format!("{}:{}:{}", self.head_sprite, self.doram as u8, self.hair_color)
    }

    pub fn needs_head_recolor(&self) -> bool {
        self.preview.is_some() && !self.head_sprite.is_empty() && self.applied_head != self.head_recolor_key()
    }

    pub fn store_head_textures(&mut self, textures: Option<Vec<Arc<Texture>>>) {
        self.head_textures = textures;
        self.applied_head = self.head_recolor_key();
    }

    pub fn clear_preview(&mut self) {
        self.preview = None;
        self.head_textures = None;
        self.hair_thumbs.clear();
        self.color_thumbs.clear();
        self.requested_preview.clear();
        self.pending_entity = 0;
        self.head_sprite.clear();
        self.applied_head.clear();
        self.loaded_thumbs.clear();
    }

    fn refresh_page_label(&mut self) {
        self.page_label = match self.page_count > 1 {
            true => format!("Page {} / {}", self.page + 1, self.page_count),
            false => String::new(),
        };
    }
}

pub fn sex_from_account(account_sex: Sex) -> (u8, bool) {
    match account_sex {
        Sex::Female => (0, true),
        Sex::Male => (1, true),
        Sex::Both | Sex::Server => (1, false),
    }
}

/// Exactly six ASCII bytes. Short input is not padded, because the server
/// compares the six bytes with `strcmp` after lowercasing.
pub fn confirmation_bytes(text: &str) -> Option<[u8; CONFIRMATION_LEN]> {
    let bytes = text.as_bytes();
    if bytes.len() != CONFIRMATION_LEN || !bytes.iter().all(|byte| byte.is_ascii() && *byte != 0) {
        return None;
    }
    let mut confirmation = [0; CONFIRMATION_LEN];
    confirmation.copy_from_slice(bytes);
    Some(confirmation)
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// `deletion_reverse_date` is an absolute unix timestamp. `0` means not queued.
pub fn deletion_phase(delete_at: i32, now_unix: i64) -> DeletionPhase {
    if delete_at <= 0 {
        return DeletionPhase::None;
    }
    let remaining = delete_at as i64 - now_unix;
    match remaining > 0 {
        true => DeletionPhase::Waiting { remaining_secs: remaining },
        false => DeletionPhase::Ready,
    }
}

pub fn format_countdown(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}

pub fn reserve_effect(result: u32, remaining_seconds: u32, now_unix: i64) -> ReserveEffect {
    match result {
        1 => {
            let delete_at = now_unix.saturating_add(remaining_seconds as i64);
            let delete_at = i32::try_from(delete_at).unwrap_or(i32::MAX);
            ReserveEffect::Scheduled {
                delete_at,
                message: "Deletion reserved. Confirm it when the countdown ends.",
            }
        }
        3 => ReserveEffect::Rejected {
            message: "Could not reserve deletion because of a database error.",
        },
        4 => ReserveEffect::Rejected {
            message: "Leave the guild before deleting this character.",
        },
        5 => ReserveEffect::Rejected {
            message: "Leave the party before deleting this character.",
        },
        _ => ReserveEffect::Rejected {
            message: "The server refused to reserve this deletion.",
        },
    }
}

pub fn accept_effect(result: u32) -> AcceptEffect {
    match result {
        1 => AcceptEffect::Removed {
            message: "Character deleted.",
        },
        2 => AcceptEffect::Rejected {
            message: "Deletion is disabled on this server.",
        },
        3 => AcceptEffect::Rejected {
            message: "Could not delete the character because of a database error.",
        },
        4 => AcceptEffect::Rejected {
            message: "The deletion countdown has not finished.",
        },
        5 => AcceptEffect::Rejected {
            message: "Confirmation does not match. This server expects delete.",
        },
        _ => AcceptEffect::Rejected {
            message: "The server refused to delete this character.",
        },
    }
}

pub fn cancel_effect(result: u32) -> CancelEffect {
    match result {
        1 => CancelEffect::Cleared {
            message: "Deletion canceled.",
        },
        _ => CancelEffect::Rejected {
            message: "Could not cancel the deletion.",
        },
    }
}

/// Slots shown for one page. `page_count` of 0 or 1 shows every slot.
pub fn visible_slot_range(slot_count: usize, page: u32, page_count: u32) -> (usize, usize) {
    if slot_count == 0 {
        return (0, 0);
    }
    let pages = page_count.max(1);
    if pages == 1 {
        return (0, slot_count);
    }
    let per_page = slot_count.div_ceil(pages as usize).max(1);
    let page = (page as usize).min(pages as usize - 1);
    let start = page * per_page;
    let end = (start + per_page).min(slot_count);
    (start, end)
}

/// Creation previews use `0xE1000000`. The styling shop already owns
/// `0xF0000000`.
const PREVIEW_ENTITY_BASE: u32 = 0xE100_0000;

pub fn preview_entity_id(generation: u32) -> u32 {
    PREVIEW_ENTITY_BASE | (generation & 0x00FF_FFFF)
}

pub fn is_preview_entity(entity_id: u32) -> bool {
    entity_id & 0xFF00_0000 == PREVIEW_ENTITY_BASE
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_tail_after_three_characters_keeps_the_list() {
        let mut screen = super::CharacterScreen::default();
        // No characters yet: the single empty packet is the real list.
        assert!(!screen.is_list_tail(true));
        screen.on_character_list();
        // Three characters, then the empty tail.
        assert!(screen.is_list_tail(true));
        assert!(!screen.is_list_tail(false));
        // A new character server connection starts over.
        screen.reset_for_server();
        assert!(!screen.is_list_tail(true));
    }

    use super::*;

    #[test]
    fn creation_fields_follow_hercules_limits() {
        let mut screen = CharacterScreen::default();
        screen.begin_creation(4, Sex::Both);
        screen.set_sex(0);
        screen.set_hair_style(23);
        screen.set_hair_color(8);
        let fields = screen.packet_fields("Nova");
        assert_eq!(fields.slot, 4);
        assert_eq!(fields.sex, 0);
        assert_eq!(fields.hair_style, 23);
        assert_eq!(fields.hair_color, 8);
        assert_eq!(fields.start_job, JOB_NOVICE);

        screen.set_doram(true);
        let fields = screen.packet_fields("Nova");
        assert_eq!(fields.start_job, JOB_SUMMONER);
        assert_eq!(fields.hair_style, DORAM_HAIR_STYLES);
        assert_eq!(fields.hair_color, DORAM_HAIR_COLOR_MAX);
    }

    #[test]
    fn account_sex_locks_male_and_female() {
        assert_eq!(sex_from_account(Sex::Male), (1, true));
        assert_eq!(sex_from_account(Sex::Female), (0, true));
        assert_eq!(sex_from_account(Sex::Server), (1, false));
        let mut screen = CharacterScreen::default();
        screen.begin_creation(0, Sex::Male);
        screen.set_sex(0);
        assert_eq!(screen.sex, 1);
    }

    #[test]
    fn confirmation_is_exactly_six_ascii_bytes() {
        assert_eq!(confirmation_bytes("delete").unwrap(), *b"delete");
        assert_eq!(confirmation_bytes("990101").unwrap(), *b"990101");
        assert!(confirmation_bytes("del").is_none());
        assert!(confirmation_bytes("delete!").is_none());
        assert!(confirmation_bytes("dele\0e").is_none());
        assert!(confirmation_bytes("삭제삭제").is_none());
    }

    #[test]
    fn reserve_accept_and_cancel_follow_hercules_results() {
        let effect = reserve_effect(1, 86_400, 1_700_000_000);
        assert_eq!(effect, ReserveEffect::Scheduled {
            delete_at: 1_700_086_400,
            message: "Deletion reserved. Confirm it when the countdown ends.",
        });
        assert!(matches!(reserve_effect(4, 0, 0), ReserveEffect::Rejected { .. }));
        assert!(matches!(accept_effect(1), AcceptEffect::Removed { .. }));
        assert!(matches!(accept_effect(4), AcceptEffect::Rejected { .. }));
        match accept_effect(5) {
            AcceptEffect::Rejected { message } => assert!(message.contains("delete")),
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(cancel_effect(1), CancelEffect::Cleared { .. }));
        assert!(matches!(cancel_effect(2), CancelEffect::Rejected { .. }));
    }

    #[test]
    fn countdown_uses_the_absolute_timestamp() {
        assert_eq!(deletion_phase(0, 100), DeletionPhase::None);
        assert_eq!(deletion_phase(150, 100), DeletionPhase::Waiting { remaining_secs: 50 });
        assert_eq!(deletion_phase(100, 100), DeletionPhase::Ready);
        assert_eq!(format_countdown(3661), "01:01:01");
    }

    #[test]
    fn pages_show_every_slot_until_the_server_sends_more_than_one() {
        assert_eq!(visible_slot_range(9, 0, 0), (0, 9));
        assert_eq!(visible_slot_range(9, 0, 1), (0, 9));
        assert_eq!(visible_slot_range(9, 0, 2), (0, 5));
        assert_eq!(visible_slot_range(9, 1, 2), (5, 9));
        let mut screen = CharacterScreen::default();
        screen.set_page_count(2);
        assert!(screen.can_page_forward());
        screen.step_page(1);
        assert_eq!(screen.page, 1);
        assert_eq!(screen.page_label, "Page 2 / 2");
        screen.step_page(1);
        assert_eq!(screen.page, 1);
    }

    #[test]
    fn character_list_keeps_bans_until_the_next_server() {
        let mut screen = CharacterScreen::default();
        screen.set_bans(vec![CharacterBan {
            character_id: CharacterId(42),
            until: "2099-01-01 00:00:00".to_owned(),
        }]);
        screen.on_character_list();
        assert_eq!(screen.ban_label(CharacterId(42)), Some("2099-01-01 00:00:00"));
        screen.reset_for_server();
        assert_eq!(screen.ban_label(CharacterId(42)), None);
    }
}
