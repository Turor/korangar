//! No-swapchain session: the autopilot command language, `InputSystem`, and
//! window state.
//!
//! Live login, map entry, and NPC menus are in [`live`].

mod live;
mod social;

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use ragnarok_packets::ClientTick;
use winit::keyboard::KeyCode;

use self::live::SentHeaders;
pub use self::live::grid_navigation::LiveGridNavReport;
pub use self::live::inventory_grid::LiveInventoryGridReport;
pub use self::live::inventory_view::LiveInventoryViewReport;
pub use self::live::item_menu::LiveItemMenuReport;
pub use self::live::item_move::LiveItemMoveReport;
pub use self::live::item_tooltip::LiveItemTooltipReport;
pub use self::live::job_names::LivePartyJobReport;
pub use self::live::mail::MailRoundTrip;
pub use self::live::storage_tools::LiveStoragePlanReport;
pub use self::live::{SelectedOption, SentChoice};
use crate::autopilot::{ClickKind, Command, parse_command};
use crate::e2e::actions::{begin_npc_dialog, plan_tab_switch, send_dialog_choice, send_public_chat, send_walk};
use crate::input::{InputEvent, InputSystem};
use crate::interface::windows::{WindowClass, WindowGroup};

#[derive(Clone, Copy)]
pub(super) struct NpcSpot {
    pub(super) id: u32,
    pub(super) x: u16,
    pub(super) y: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    CharacterSelect,
    EnteringMap,
    InWorld,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outbound {
    Login,
    SelectServer,
    CreateCharacter { slot: usize },
    SelectCharacter { slot: usize },
    ConnectMap,
    MapLoaded,
    StartDialog { npc_id: u32 },
    NextDialog { npc_id: u32 },
    CloseDialog { npc_id: u32 },
    ChooseDialogOption { npc_id: u32, option: i8 },
    SwitchWindowTab { from: String, to: String },
    SendMessage { text: String },
    PlayerMove { x: u16, y: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogView {
    pub npc_id: u32,
    pub text: String,
    pub options: Vec<String>,
    pub selected: Option<usize>,
    pub awaiting_next: bool,
    pub awaiting_close: bool,
    /// Set by Next. The following page replaces the text, matching
    /// `Dialog::add_text`.
    clear_next: bool,
}

impl DialogView {
    fn new(npc_id: u32) -> Self {
        Self {
            npc_id,
            text: String::new(),
            options: Vec::new(),
            selected: None,
            awaiting_next: false,
            awaiting_close: false,
            clear_next: false,
        }
    }

    pub fn visible_text(&self) -> String {
        let mut parts = Vec::new();
        if !self.text.is_empty() {
            parts.push(self.text.clone());
        }
        parts.extend(self.options.iter().cloned());
        parts.join("\n")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsView {
    pub open_member: Option<String>,
    /// Group of the open member. Empty when no member of a tabbed group is
    /// open.
    pub group_name: Option<&'static str>,
    /// Tabs of that group, not the static catalog of every group.
    pub tab_labels: Vec<&'static str>,
}

pub struct LogicSession {
    input: InputSystem,
    tick: u32,
    phase: Phase,
    windows: Vec<WindowClass>,
    dialog: Option<DialogView>,
    characters: Vec<String>,
    slots: Vec<u8>,
    outbound: Vec<Outbound>,
    sent: SentHeaders,
    net: Option<korangar_networking::NetworkingSystem<SentHeaders>>,
    pub(super) events: Option<korangar_networking::NetworkEventBuffer>,
    pub(super) login_data: Option<korangar_networking::LoginServerLoginData>,
    pub(super) servers: Vec<ragnarok_packets::CharacterServerInformation>,
    pub(super) password: String,
    pub(super) username: String,
    pub(super) npcs: Vec<NpcSpot>,
    pub(super) map_name: String,
    pub(super) player_x: Option<u16>,
    pub(super) player_y: Option<u16>,
    /// Last dialog page text, kept for the failure report. Never a password.
    pub(super) dialog_note: String,
    pub(super) map_loaded: bool,
    pub(super) list_received: bool,
    pub(super) failure: Option<String>,
    pub(super) login_busy: bool,
    pub(super) creating: bool,
    pub(super) selected_sent: bool,
    /// Option index chosen through `ChooseDialogOption`. A later page does not
    /// clear it.
    last_selected: Option<usize>,
    trace: Vec<String>,
    lock: Option<std::fs::File>,
    /// HUD status icons fed from live `StatusChange` packets.
    pub(super) status_strip: crate::e2e::StatusStrip,
    /// Player looks from the character list and live appear packets.
    pub(super) headgear: crate::e2e::HeadgearScene,
    /// Slot caps seen on the map server (`0x0B18` arrives before map entry
    /// ends).
    pub(super) item_capacities: Vec<korangar_networking::ItemCapacity>,
    /// Item reform and laphine windows driven by injected packets and events.
    pub(super) workshop: crate::e2e::item_workshop::ItemWorkshopSession,
    /// Weather, zone badge, and gat cells for the map the session is on.
    pub(crate) map_world: crate::e2e::weather::MapWorld,
    /// Companion packets seen on the live map connection.
    pub(super) companion_note: crate::e2e::companions::CompanionNote,
}

impl LogicSession {
    pub fn offline() -> Self {
        Self::empty(String::new(), String::new())
    }

    fn empty(username: String, password: String) -> Self {
        Self {
            input: InputSystem::new(Arc::new(AtomicU64::new(0))),
            tick: 0,
            phase: Phase::Idle,
            windows: Vec::new(),
            dialog: None,
            characters: Vec::new(),
            slots: Vec::new(),
            outbound: Vec::new(),
            sent: SentHeaders::default(),
            net: None,
            events: None,
            login_data: None,
            servers: Vec::new(),
            password,
            username,
            npcs: Vec::new(),
            map_name: String::new(),
            player_x: None,
            player_y: None,
            dialog_note: String::new(),
            map_loaded: false,
            list_received: false,
            failure: None,
            login_busy: false,
            creating: false,
            selected_sent: false,
            last_selected: None,
            trace: Vec::new(),
            lock: None,
            status_strip: crate::e2e::StatusStrip::default(),
            headgear: crate::e2e::HeadgearScene::default(),
            item_capacities: Vec::new(),
            workshop: crate::e2e::item_workshop::ItemWorkshopSession::default(),
            map_world: crate::e2e::weather::MapWorld::default(),
            companion_note: crate::e2e::companions::CompanionNote::default(),
        }
    }

    pub fn map_name(&self) -> &str {
        &self.map_name
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn character_names(&self) -> &[String] {
        &self.characters
    }

    pub fn window_names(&self) -> Vec<String> {
        self.windows.iter().map(|class| format!("{class:?}")).collect()
    }

    pub fn dialog(&self) -> Option<&DialogView> {
        self.dialog.as_ref()
    }

    pub fn last_selected(&self) -> Option<usize> {
        self.last_selected
    }

    pub fn status_strip(&self) -> &crate::e2e::StatusStrip {
        &self.status_strip
    }

    pub fn headgear(&self) -> &crate::e2e::HeadgearScene {
        &self.headgear
    }

    /// Map name and cell from the last `ChangeMap` / walk.
    pub fn player_map_cell(&self) -> Option<(String, u16, u16)> {
        let map = crate::state::navigation::normalize_map_name(&self.map_name);
        match (map.is_empty(), self.player_x, self.player_y) {
            (false, Some(x), Some(y)) => Some((map, x, y)),
            _ => None,
        }
    }

    pub fn companion_note(&self) -> &crate::e2e::companions::CompanionNote {
        &self.companion_note
    }

    pub fn outbound(&self) -> &[Outbound] {
        &self.outbound
    }

    pub fn sent_headers(&self) -> Vec<u16> {
        self.sent.headers.lock().map(|guard| guard.clone()).unwrap_or_default()
    }

    pub fn sent_header_count(&self) -> usize {
        self.sent.headers.lock().map(|guard| guard.len()).unwrap_or(0)
    }

    /// One injected right-click, with Alt when `alt` is set. The pair is
    /// `(right button landed, Alt is down)` after the same update the frame
    /// uses.
    fn sample_right_click(&mut self, alt: bool) -> (bool, bool) {
        self.input.expire_synthesized();
        self.input.advance_synthetic_drag();
        // Releasing a button that was never down marks a press. Flush that
        // frame before the click, and stay outside the double-click window.
        self.tick = self.tick.wrapping_add(1_000);
        let _ = self.finish_frame();
        if alt {
            self.input.synthesize_key(KeyCode::AltLeft, false, false, false);
        }
        self.input
            .synthesize_click(20.0, 20.0, korangar_interface::layout::MouseButton::Right);
        self.tick = self.tick.wrapping_add(1_000);
        let (report, _) = self.finish_frame();
        let right = report.mouse_click == Some(korangar_interface::layout::MouseButton::Right);
        let alt_down = self.input.key_down(KeyCode::AltLeft);
        (right, alt_down)
    }

    pub fn sent_choices(&self) -> Vec<self::live::SentChoice> {
        self.sent.choices()
    }

    pub fn settings_view(&self) -> SettingsView {
        let group = WindowGroup::Settings;
        let Some(class) = group.open_members(|class| self.windows.contains(&class)).into_iter().last() else {
            return SettingsView {
                open_member: None,
                group_name: None,
                tab_labels: Vec::new(),
            };
        };
        SettingsView {
            open_member: Some(format!("{class:?}")),
            group_name: Some(group.name()),
            tab_labels: group.tabs().iter().map(|tab| tab.label).collect(),
        }
    }

    /// Present a dialog the way a server `OpenDialog` plus choice buttons
    /// would.
    pub fn preview_dialog(&mut self, npc_id: u32, text: &str, options: &[&str]) {
        self.show_page(npc_id, text);
        self.show_choices(npc_id, options.iter().map(|option| (*option).to_owned()).collect());
    }

    pub fn apply_line(&mut self, line: &str) -> Result<(), String> {
        let command = parse_command(line)?;
        self.apply_command(command)
    }

    /// Select `index` (0-based). The packet option is `index + 1`, matching the
    /// dialog buttons.
    pub fn choose_option(&mut self, index: usize) -> Result<(), String> {
        let dialog = self.dialog.as_ref().ok_or_else(|| "no dialog is open".to_owned())?;
        if index >= dialog.options.len() {
            return Err(format!("dialog option {index} is out of range"));
        }
        let option = i8::try_from(index + 1).map_err(|_| "dialog option does not fit the packet".to_owned())?;
        let npc_id = dialog.npc_id;
        if let Some(dialog) = self.dialog.as_mut() {
            dialog.selected = Some(index);
        }
        self.last_selected = Some(index);
        let event = InputEvent::ChooseDialogOption {
            npc_id: ragnarok_packets::EntityId(npc_id),
            option,
        };
        self.note_event(&event);
        self.outbound.push(Outbound::ChooseDialogOption { npc_id, option });
        if let Some(net) = self.net.as_mut() {
            send_dialog_choice(net, ragnarok_packets::EntityId(npc_id), option).map_err(|_| "map server is not connected".to_owned())?;
        }
        Ok(())
    }

    pub fn choose_option_by_text(&mut self, label: &str) -> Result<usize, String> {
        let index = self
            .dialog
            .as_ref()
            .ok_or_else(|| "no dialog is open".to_owned())?
            .options
            .iter()
            .position(|option| option.eq_ignore_ascii_case(label) || option.to_ascii_lowercase().contains(&label.to_ascii_lowercase()))
            .ok_or_else(|| format!("dialog has no option matching {label}"))?;
        self.choose_option(index)?;
        Ok(index)
    }

    fn apply_command(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Chat(text) => {
                let event = InputEvent::SendMessage { text: text.clone() };
                self.note_event(&event);
                self.outbound.push(Outbound::SendMessage { text: text.clone() });
                if let Some(net) = self.net.as_mut() {
                    let name = self
                        .characters
                        .first()
                        .cloned()
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| self.username.clone());
                    if !name.is_empty() && !text.is_empty() {
                        send_public_chat(net, &name, &text).map_err(|_| "map server is not connected".to_owned())?;
                    }
                }
                Ok(())
            }
            Command::Tab(class) => self.switch_tab(class),
            Command::Window { class, open, event } => {
                self.note_event(&event);
                self.apply_workshop_event(&event);
                if self.windows.contains(&class) != open {
                    self.apply_window_event(event);
                }
                Ok(())
            }
            Command::Toggle(event) | Command::Event(event) => {
                self.note_event(&event);
                self.apply_workshop_event(&event);
                self.apply_window_event(event);
                Ok(())
            }
            Command::Wait(ms) => {
                let capped = ms.min(2_000);
                std::thread::sleep(std::time::Duration::from_millis(capped));
                Ok(())
            }
            Command::Move { x, y } => {
                let event = InputEvent::PlayerMove {
                    destination: ragnarok_packets::TilePosition { x, y },
                };
                self.note_event(&event);
                self.outbound.push(Outbound::PlayerMove { x, y });
                if let Some(net) = self.net.as_mut() {
                    send_walk(net, ragnarok_packets::TilePosition { x, y }).map_err(|_| "map server is not connected".to_owned())?;
                }
                Ok(())
            }
            Command::Talk { x, y } => {
                self.note(format!("TalkAt {x} {y}"));
                let near = self
                    .npcs
                    .iter()
                    .find(|npc| npc.x.abs_diff(x) <= 1 && npc.y.abs_diff(y) <= 1)
                    .map(|npc| npc.id);
                if let Some(npc_id) = near {
                    self.input.expire_synthesized();
                    self.input.advance_synthetic_drag();
                    self.input.synthesize_click(8.0, 8.0, korangar_interface::layout::MouseButton::Left);
                    let _ = self.finish_frame();
                    if let Some(net) = self.net.as_mut() {
                        let _ = begin_npc_dialog(net, ragnarok_packets::EntityId(npc_id));
                        self.outbound.push(Outbound::StartDialog { npc_id });
                    }
                }
                Ok(())
            }
            Command::Click { x, y, kind } => {
                let button = match kind {
                    ClickKind::Left => korangar_interface::layout::MouseButton::Left,
                    ClickKind::Right => korangar_interface::layout::MouseButton::Right,
                    ClickKind::Double => korangar_interface::layout::MouseButton::DoubleLeft,
                };
                self.input.expire_synthesized();
                self.input.advance_synthetic_drag();
                self.input.synthesize_click(x, y, button);
                let _ = self.finish_frame();
                Ok(())
            }
            Command::Drag { x0, y0, x1, y1 } => {
                self.input.expire_synthesized();
                self.input.advance_synthetic_drag();
                self.input.begin_synthetic_drag(x0, y0, x1, y1);
                let _ = self.finish_frame();
                self.input.expire_synthesized();
                self.input.advance_synthetic_drag();
                let _ = self.finish_frame();
                Ok(())
            }
            Command::Key { code, alt, ctrl, shift } => {
                self.input.expire_synthesized();
                self.input.advance_synthetic_drag();
                self.input.synthesize_key(code, alt, ctrl, shift);
                let (_report, events) = self.finish_frame();
                for event in events {
                    self.note_event(&event);
                    self.apply_window_event(event);
                }
                Ok(())
            }
            Command::Dump(path) => {
                self.note(format!("dump {}", path.as_str()));
                Ok(())
            }
            Command::Quit => {
                self.note("quit".to_owned());
                Ok(())
            }
        }
    }

    fn switch_tab(&mut self, class: WindowClass) -> Result<(), String> {
        let Some(group) = WindowGroup::of(class) else {
            return Err(format!("{class:?} is not in a tabbed group"));
        };
        let from = group
            .open_members(|member| self.windows.contains(&member))
            .into_iter()
            .find(|member| *member != class);
        let Some(from) = from else {
            return Ok(());
        };
        let event = InputEvent::SwitchWindowTab { from, to: class };
        self.note_event(&event);
        self.outbound.push(Outbound::SwitchWindowTab {
            from: format!("{from:?}"),
            to: format!("{class:?}"),
        });
        let to_is_open = self.windows.contains(&class);
        let Some(plan) = plan_tab_switch(from, class, to_is_open) else {
            return Ok(());
        };
        if plan.close_from {
            self.close_window(from);
        }
        if let Some(open) = plan.open_event {
            self.apply_window_event(open);
        }
        Ok(())
    }

    fn apply_window_event(&mut self, event: InputEvent) {
        let Some(class) = window_of(&event) else {
            return;
        };
        if self.windows.contains(&class) {
            self.close_window(class);
        } else {
            self.open_window(class);
        }
    }

    pub(super) fn open_window(&mut self, class: WindowClass) {
        if !self.windows.contains(&class) {
            self.windows.push(class);
        }
    }

    pub(super) fn close_window(&mut self, class: WindowClass) {
        self.windows.retain(|open| *open != class);
    }

    pub(super) fn show_page(&mut self, npc_id: u32, text: &str) {
        if !text.is_empty() {
            self.dialog_note = text.chars().take(120).collect();
        }
        let dialog = self.dialog.get_or_insert_with(|| DialogView::new(npc_id));
        dialog.npc_id = npc_id;
        if dialog.clear_next {
            dialog.text.clear();
            dialog.clear_next = false;
            dialog.awaiting_next = false;
            dialog.awaiting_close = false;
        }
        if !dialog.text.is_empty() && !text.is_empty() {
            dialog.text.push('\n');
        }
        dialog.text.push_str(text);
        dialog.options.clear();
        dialog.selected = None;
        dialog.awaiting_next = false;
        dialog.awaiting_close = false;
        self.open_window(WindowClass::Dialog);
    }

    pub(super) fn show_choices(&mut self, npc_id: u32, choices: Vec<String>) {
        let dialog = self.dialog.get_or_insert_with(|| DialogView::new(npc_id));
        dialog.npc_id = npc_id;
        dialog.options = choices;
        dialog.selected = None;
        dialog.awaiting_next = false;
        self.open_window(WindowClass::Dialog);
    }

    pub(super) fn mark_next(&mut self, npc_id: u32) {
        let dialog = self.dialog.get_or_insert_with(|| DialogView::new(npc_id));
        dialog.npc_id = npc_id;
        dialog.awaiting_next = true;
        dialog.awaiting_close = false;
        dialog.clear_next = true;
    }

    pub(super) fn mark_close(&mut self, npc_id: u32) {
        let dialog = self.dialog.get_or_insert_with(|| DialogView::new(npc_id));
        dialog.npc_id = npc_id;
        dialog.awaiting_next = false;
        dialog.awaiting_close = true;
    }

    fn finish_frame(&mut self) -> (crate::input::InputReport, Vec<InputEvent>) {
        let report = self.input.update_delta(ClientTick(self.tick));
        self.tick = self.tick.wrapping_add(16);
        let mut events = Vec::new();
        let _ = self.input.handle_keyboard_input(
            &mut events,
            false,
            #[cfg(feature = "debug")]
            false,
            #[cfg(feature = "debug")]
            false,
        );
        (report, events)
    }

    fn note(&mut self, line: String) {
        self.trace.push(line);
    }

    fn note_event(&mut self, event: &InputEvent) {
        self.trace.push(format!("{event:?}"));
    }

    pub(super) fn fail(&mut self, message: impl Into<String>) {
        if self.failure.is_some() {
            return;
        }
        let message = message.into();
        let message = if self.password.is_empty() {
            message
        } else {
            message.replace(&self.password, "[redacted]")
        };
        self.failure = Some(message);
    }

    pub(super) fn hold_lock(&mut self, file: std::fs::File) {
        self.lock = Some(file);
    }

    pub(crate) fn buffer_text(&mut self, text: &str) {
        for character in text.chars() {
            self.input.buffer_character(character);
        }
    }

    pub(crate) fn take_frame_characters(&mut self) -> Vec<char> {
        self.finish_frame().0.characters
    }

    pub(crate) fn show_window(&mut self, class: WindowClass) {
        self.open_window(class);
    }

    /// `CZ_WHISPER` through the same call the client match arm uses. Offline
    /// sessions have no socket and only record the intent at the caller.
    pub(crate) fn dispatch_whisper(&mut self, target: &str, text: &str) -> Result<(), String> {
        if let Some(net) = self.net.as_mut() {
            net.send_whisper(target, text)
                .map_err(|_| "map server is not connected".to_owned())?;
        }
        Ok(())
    }
}

impl Drop for LogicSession {
    fn drop(&mut self) {
        if let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) {
            if self.phase == Phase::InWorld {
                let _ = net.log_out();
            }
            net.disconnect_from_map_server();
            net.disconnect_from_character_server();
            net.disconnect_from_login_server();
            net.get_events(events);
        }
        self.password.clear();
        // The advisory lock closes with the file.
        self.lock.take();
    }
}

fn window_of(event: &InputEvent) -> Option<WindowClass> {
    Some(match event {
        InputEvent::ToggleAudioSettingsWindow => WindowClass::AudioSettings,
        InputEvent::ToggleGameSettingsWindow => WindowClass::GameSettings,
        InputEvent::ToggleInterfaceSettingsWindow => WindowClass::InterfaceSettings,
        InputEvent::ToggleGraphicsSettingsWindow => WindowClass::GraphicsSettings,
        InputEvent::ToggleKeybindsWindow => WindowClass::Keybinds,
        InputEvent::ToggleInventoryWindow => WindowClass::Inventory,
        InputEvent::ToggleEquipmentWindow => WindowClass::Equipment,
        InputEvent::ToggleStatsWindow => WindowClass::Stats,
        InputEvent::ToggleMenuWindow => WindowClass::Menu,
        InputEvent::ToggleWorldMap => WindowClass::WorldMap,
        _ => return None,
    })
}

pub(super) fn safe_option_index(options: &[String]) -> usize {
    const HINTS: &[&str] = &[
        "cancel",
        "quit",
        "exit",
        "close",
        "leave",
        "goodbye",
        "bye",
        "nothing",
        "no thanks",
        "취소",
        "닫기",
        "그만",
    ];
    options
        .iter()
        .position(|text| {
            let lower = text.to_ascii_lowercase();
            HINTS.iter().any(|hint| lower.contains(hint))
        })
        .unwrap_or(options.len().saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{ChooseDialogOptionPacket, Packet};

    use super::{LogicSession, Outbound, safe_option_index};
    use crate::interface::windows::WindowGroup;

    #[test]
    fn settings_script_opens_the_tabbed_group() {
        let mut session = LogicSession::offline();
        let closed = session.settings_view();
        assert!(closed.open_member.is_none());
        assert!(closed.group_name.is_none());
        assert!(closed.tab_labels.is_empty());
        session.apply_line("open GameSettings").unwrap();
        let view = session.settings_view();
        assert_eq!(view.open_member.as_deref(), Some("GameSettings"));
        assert_eq!(view.group_name, Some("Settings"));
        assert_eq!(view.tab_labels, ["Interface", "Graphics", "Audio", "Game", "Keys"]);
        session.apply_line("tab AudioSettings").unwrap();
        let view = session.settings_view();
        assert_eq!(view.open_member.as_deref(), Some("AudioSettings"));
        assert_eq!(view.group_name, Some("Settings"));
        assert!(session.outbound().iter().any(|item| matches!(
            item,
            Outbound::SwitchWindowTab { from, to } if from == "GameSettings" && to == "AudioSettings"
        )));
        assert!(!session.window_names().iter().any(|name| name == "GameSettings"));
        for name in ["Settings", "Character", "Social", "Journal", "Pet Stable"] {
            assert!(WindowGroup::ALL.iter().any(|group| group.name() == name), "{name}");
        }
        session.apply_line("move 10 20").unwrap();
        assert!(matches!(session.outbound().last(), Some(Outbound::PlayerMove { x: 10, y: 20 })));
        session.apply_line("chat hello").unwrap();
        assert!(matches!(session.outbound().last(), Some(Outbound::SendMessage { text }) if text == "hello"));
    }

    #[test]
    fn dialog_option_uses_the_button_index() {
        let mut session = LogicSession::offline();
        session.preview_dialog(42, "Where to?", &["Prontera", "Cancel"]);
        let index = session.choose_option_by_text("Cancel").unwrap();
        assert_eq!(index, 1);
        assert_eq!(safe_option_index(&session.dialog().unwrap().options), 1);
        assert_eq!(session.dialog().unwrap().selected, Some(1));
        assert!(session.dialog().unwrap().visible_text().contains("Cancel"));
        assert!(matches!(
            session.outbound().last(),
            Some(Outbound::ChooseDialogOption { npc_id: 42, option: 2 })
        ));
        assert_eq!(ChooseDialogOptionPacket::HEADER.0, 0x00B8);
        session.mark_next(42);
        session.show_page(42, "Next page");
        assert_eq!(session.dialog().unwrap().text, "Next page");

        let sent = super::live::SentHeaders::default();
        let packet = ChooseDialogOptionPacket::new(ragnarok_packets::EntityId(42), 2);
        ragnarok_packets::handler::PacketCallback::outgoing_packet(&sent, &packet);
        let choices = sent.choices();
        assert_eq!(choices, vec![super::SentChoice { npc_id: 42, option: 2 }]);
        assert!(sent.headers.lock().unwrap().contains(&0x00B8));
    }
}
