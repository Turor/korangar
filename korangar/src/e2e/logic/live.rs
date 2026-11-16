//! Hercules TEST login through the same networking calls the client match arms
//! use.
//!
//! No swapchain. Passwords stay out of errors.

pub(super) mod dressing_room;
pub(super) mod grid_navigation;
pub(super) mod inventory_grid;
pub(super) mod inventory_view;
pub(super) mod item_flags;
pub(super) mod item_link;
pub(super) mod item_menu;
pub(super) mod item_move;
pub(super) mod item_tooltip;
pub(super) mod job_names;
pub(super) mod mail;
pub(super) mod storage_tools;

use std::fs::OpenOptions;
use std::io::Write;
use std::net::Ipv4Addr;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ragnarok_bytes::ByteWriter;
use ragnarok_packets::handler::PacketCallback;
use ragnarok_packets::{EntityId, JobId, Packet, PacketExt, Sex};

use super::{LogicSession, Outbound, Phase, safe_option_index};
use crate::e2e::actions::{begin_npc_dialog, send_walk};
use crate::e2e::credentials::{Credentials, LiveConnect, TEST_HOST};

/// One `0x00B8` choice taken from the serialized packet (npc id at byte 2,
/// choice at byte 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentChoice {
    pub npc_id: u32,
    pub option: i8,
}

#[derive(Clone, Default)]
pub(super) struct SentHeaders {
    pub(super) headers: Arc<Mutex<Vec<u16>>>,
    choices: Arc<Mutex<Vec<SentChoice>>>,
}

impl SentHeaders {
    pub(super) fn choices(&self) -> Vec<SentChoice> {
        self.choices.lock().map(|guard| guard.clone()).unwrap_or_default()
    }
}

impl PacketCallback for SentHeaders {
    fn outgoing_packet<P>(&self, packet: &P)
    where
        P: Packet,
    {
        let mut writer = ByteWriter::new();
        if packet.packet_to_bytes(&mut writer).is_err() {
            return;
        }
        let bytes = writer.into_inner();
        if bytes.len() < 2 {
            return;
        }
        let header = u16::from_le_bytes([bytes[0], bytes[1]]);
        if let Ok(mut headers) = self.headers.lock() {
            headers.push(header);
        }
        if header == 0x00B8 && bytes.len() >= 7 {
            let npc_id = u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
            let option = bytes[6] as i8;
            if let Ok(mut choices) = self.choices.lock() {
                choices.push(SentChoice { npc_id, option });
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedOption {
    pub npc_id: u32,
    pub index: usize,
    pub text: String,
    pub wire_option: i8,
}

impl LogicSession {
    pub fn start_live() -> LiveConnect<LogicSession> {
        match Credentials::resolve() {
            LiveConnect::Skip(reason) => LiveConnect::Skip(reason),
            LiveConnect::Invalid(error) => LiveConnect::Invalid(error),
            LiveConnect::Ready(credentials) => match Self::connect(credentials) {
                Ok(session) => LiveConnect::Ready(session),
                Err(error) => LiveConnect::Invalid(error),
            },
        }
    }

    fn connect(credentials: Credentials) -> Result<Self, String> {
        let lock = account_lock()?;
        let mut session = Self::empty(credentials.username.clone(), credentials.password().to_owned());
        session.hold_lock(lock);
        let (net, events) = korangar_networking::NetworkingSystem::spawn_with_callback(session.sent.clone());
        session.net = Some(net);
        session.events = Some(events);
        Ok(session)
    }

    pub fn login_to_character_select(&mut self) -> Result<(), String> {
        self.dispatch_login()?;
        match self.pump_until_character_select(Duration::from_secs(25)) {
            Err(error) if self.login_busy => {
                self.login_busy = false;
                self.failure = None;
                self.reset_links();
                std::thread::sleep(Duration::from_secs(3));
                self.dispatch_login()?;
                self.pump_until_character_select(Duration::from_secs(25))
                    .map_err(|retry| format!("{error}; retry: {retry}"))
            }
            other => other,
        }
    }

    pub fn open_settings_group(&mut self) -> Result<super::SettingsView, String> {
        self.login_to_character_select()?;
        self.enter_world()?;
        self.apply_line("open GameSettings")?;
        self.apply_line("tab AudioSettings")?;
        let view = self.settings_view();
        if view.open_member.as_deref() != Some("AudioSettings") {
            return Err(format!("settings tab is {:?}", view.open_member));
        }
        Ok(view)
    }

    /// Log in, enter the map, and keep reading packets for `watch` so the
    /// status strip sees the statuses the server sends on map entry. The
    /// session tick for [`crate::e2e::StatusStrip::view`] is returned too.
    pub fn enter_world_and_watch_statuses(&mut self, watch: Duration) -> Result<u32, String> {
        self.login_to_character_select()?;
        self.enter_world()?;
        self.pump_for(watch)?;
        let tick = session_tick();
        self.status_strip.expire(tick);
        Ok(tick)
    }

    pub fn enter_world_and_choose_npc_option(&mut self) -> Result<SelectedOption, String> {
        self.login_to_character_select()?;
        self.enter_world()?;
        self.select_npc_option()
    }

    pub fn enter_world_for_minimap(&mut self) -> Result<(), String> {
        self.login_to_character_select()?;
        self.enter_world()
    }

    pub fn minimap_view(&self) -> crate::e2e::minimap::MinimapView {
        super::super::minimap::view_for_session(self.phase == Phase::InWorld, &self.map_name, self.player_x, self.player_y)
    }

    pub fn character_cell(&self) -> Option<(u16, u16)> {
        Some((self.player_x?, self.player_y?))
    }

    fn dispatch_login(&mut self) -> Result<(), String> {
        let address = format!("{TEST_HOST}:{}", crate::e2e::TEST_PORT)
            .parse()
            .map_err(|_| "TEST login address is not a socket".to_owned())?;
        let username = self.username.clone();
        let password = self.password.clone();
        let Some(net) = self.net.as_mut() else {
            return Err("logic mode has no network".to_owned());
        };
        net.connect_to_login_server(
            korangar_networking::SupportedPacketVersion::_20250716,
            address,
            username,
            password,
        );
        self.outbound.push(Outbound::Login);
        self.note("LogIn".to_owned());
        Ok(())
    }

    fn pump_until_character_select(&mut self, budget: Duration) -> Result<(), String> {
        let deadline = Instant::now() + budget;
        let mut saw: Option<Instant> = None;
        while Instant::now() < deadline {
            self.poll_network()?;
            self.take_failure()?;
            if self.phase == Phase::CharacterSelect {
                if saw.is_none() {
                    saw = Some(Instant::now());
                }
                if saw.is_some_and(|start| start.elapsed() >= Duration::from_millis(400)) {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(self.timeout("character select"))
    }

    pub(crate) fn enter_world(&mut self) -> Result<(), String> {
        if self.characters.is_empty() {
            self.dispatch_create()?;
            self.pump_while(Duration::from_secs(20), |session| session.creating)?;
        }
        self.take_failure()?;
        if !self.selected_sent {
            let slot = self.slots.first().copied().unwrap_or(0) as usize;
            self.dispatch_select(slot)?;
        }
        self.pump_while(Duration::from_secs(25), |session| session.phase != Phase::InWorld)?;
        self.take_failure()?;
        if self.phase != Phase::InWorld {
            return Err(self.timeout("map"));
        }
        Ok(())
    }

    /// Logs in, enters the map, then waits so property and weather packets
    /// arrive.
    pub fn enter_world_for_weather(&mut self) -> Result<(), String> {
        self.login_to_character_select()?;
        self.enter_world()?;
        self.pump_for(Duration::from_secs(2))?;
        Ok(())
    }

    fn dispatch_create(&mut self) -> Result<(), String> {
        let name = crate::autopilot::default_character_name(&self.username);
        let Some(net) = self.net.as_mut() else {
            return Err("logic mode has no network".to_owned());
        };
        net.create_character(0, name, 3, 2, JobId(0), Sex::Male)
            .map_err(|_| "character server is not connected".to_owned())?;
        self.creating = true;
        self.outbound.push(Outbound::CreateCharacter { slot: 0 });
        Ok(())
    }

    fn dispatch_select(&mut self, slot: usize) -> Result<(), String> {
        let Some(net) = self.net.as_mut() else {
            return Err("logic mode has no network".to_owned());
        };
        net.select_character(slot)
            .map_err(|_| "character server is not connected".to_owned())?;
        self.selected_sent = true;
        self.outbound.push(Outbound::SelectCharacter { slot });
        self.phase = Phase::EnteringMap;
        Ok(())
    }

    fn select_npc_option(&mut self) -> Result<SelectedOption, String> {
        self.pump_for(Duration::from_secs(6))?;
        let mut spots = self.npcs.clone();
        if let (Some(x), Some(y)) = (self.player_x, self.player_y) {
            spots.sort_by_key(|npc| chebyshev(x, y, npc.x, npc.y));
        }
        if spots.is_empty() {
            return Err(self.npc_failure("no NPC entered view on TEST"));
        }
        for npc in spots.into_iter().take(12) {
            self.close_dialog();
            self.approach(npc.x, npc.y)?;
            self.talk(npc.id)?;
            if self.advance_until_choices(npc.id, Duration::from_secs(6))? {
                let options = self.dialog.as_ref().map(|dialog| dialog.options.clone()).unwrap_or_default();
                if options.is_empty() || options.iter().all(|option| option.trim().is_empty()) {
                    continue;
                }
                let index = safe_option_index(&options);
                let text = options[index].clone();
                self.choose_option(index)?;
                self.pump_for(Duration::from_millis(600))?;
                self.take_failure()?;
                let wire_option = i8::try_from(index + 1).unwrap_or(1);
                return Ok(SelectedOption {
                    npc_id: npc.id,
                    index,
                    text,
                    wire_option,
                });
            }
        }
        Err(self.npc_failure("no NPC offered a dialog menu"))
    }

    fn approach(&mut self, x: u16, y: u16) -> Result<(), String> {
        let (Some(px), Some(py)) = (self.player_x, self.player_y) else {
            return Ok(());
        };
        if chebyshev(px, py, x, y) <= 3 {
            return Ok(());
        }
        let tx = if x > 0 { x - 1 } else { x.saturating_add(1) };
        let Some(net) = self.net.as_mut() else {
            return Err("logic mode has no network".to_owned());
        };
        send_walk(net, ragnarok_packets::TilePosition { x: tx, y }).map_err(|_| "map server is not connected".to_owned())?;
        self.note(format!("PlayerMove {tx} {y}"));
        let wait = Duration::from_millis(400 + u64::from(chebyshev(px, py, tx, y)) * 200).min(Duration::from_secs(8));
        self.pump_for(wait)
    }

    fn npc_failure(&self, step: &str) -> String {
        format!(
            "{step} (map={}, player={:?}, npcs={}, last_dialog={:?})",
            if self.map_name.is_empty() { "?" } else { &self.map_name },
            (self.player_x, self.player_y),
            self.npcs.len(),
            self.dialog_note
        )
    }

    fn talk(&mut self, npc_id: u32) -> Result<(), String> {
        self.input.expire_synthesized();
        self.input.advance_synthetic_drag();
        self.input.synthesize_click(8.0, 8.0, korangar_interface::layout::MouseButton::Left);
        let _ = self.finish_frame();
        let Some(net) = self.net.as_mut() else {
            return Err("logic mode has no network".to_owned());
        };
        begin_npc_dialog(net, EntityId(npc_id)).map_err(|_| "map server is not connected".to_owned())?;
        self.outbound.push(Outbound::StartDialog { npc_id });
        self.note(format!("TalkToNpc {npc_id}"));
        Ok(())
    }

    fn close_dialog(&mut self) {
        let Some(npc_id) = self.dialog.as_ref().map(|dialog| dialog.npc_id) else {
            return;
        };
        if let Some(net) = self.net.as_mut() {
            let _ = net.close_dialog(EntityId(npc_id));
        }
        self.outbound.push(Outbound::CloseDialog { npc_id });
        self.dialog = None;
    }

    fn advance_until_choices(&mut self, npc_id: u32, budget: Duration) -> Result<bool, String> {
        let started = Instant::now();
        let deadline = started + budget;
        let mut nexts = 0_u8;
        while Instant::now() < deadline {
            self.poll_network()?;
            self.take_failure()?;
            let (has_options, awaiting_next) = self
                .dialog
                .as_ref()
                .map(|dialog| (!dialog.options.is_empty(), dialog.awaiting_next && dialog.npc_id == npc_id))
                .unwrap_or((false, false));
            if has_options {
                return Ok(true);
            }
            // No page arrived. This sprite is not in a script conversation.
            if self.dialog.is_none() && started.elapsed() >= Duration::from_millis(2500) {
                return Ok(false);
            }
            if awaiting_next && nexts < 6 {
                nexts += 1;
                if let Some(dialog) = self.dialog.as_mut() {
                    dialog.awaiting_next = false;
                }
                self.send_next(npc_id)?;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(false)
    }

    fn send_next(&mut self, npc_id: u32) -> Result<(), String> {
        let Some(net) = self.net.as_mut() else {
            return Err("logic mode has no network".to_owned());
        };
        net.next_dialog(EntityId(npc_id))
            .map_err(|_| "map server is not connected".to_owned())?;
        self.outbound.push(Outbound::NextDialog { npc_id });
        Ok(())
    }

    fn poll_network(&mut self) -> Result<(), String> {
        let drained = {
            let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) else {
                return Ok(());
            };
            net.get_events(events);
            events.drain().collect::<Vec<_>>()
        };
        for event in drained {
            self.on_event(event)?;
        }
        Ok(())
    }

    fn pump_for(&mut self, budget: Duration) -> Result<(), String> {
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            self.poll_network()?;
            self.take_failure()?;
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }

    fn pump_while(&mut self, budget: Duration, mut pending: impl FnMut(&LogicSession) -> bool) -> Result<(), String> {
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            self.poll_network()?;
            self.take_failure()?;
            if !pending(self) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(self.timeout("session step"))
    }

    pub(super) fn on_event(&mut self, event: korangar_networking::NetworkEvent) -> Result<(), String> {
        use korangar_networking::NetworkEvent;
        self.headgear.observe(&event);
        if let NetworkEvent::ItemCapacity(capacity) = &event {
            self.item_capacities.push(*capacity);
        }
        match event {
            NetworkEvent::LoginServerConnected {
                character_servers,
                login_data,
            } => {
                let Some(server) = character_servers.first().cloned() else {
                    self.fail("login returned no character server");
                    return Ok(());
                };
                if ipv4(server.server_ip) != TEST_HOST {
                    self.fail("character server is not Hercules TEST");
                    return Ok(());
                }
                self.login_data = Some(login_data);
                self.status_strip.set_local_entity(login_data.account_id.0);
                self.servers = character_servers;
                self.outbound.push(Outbound::SelectServer);
                let Some(net) = self.net.as_mut() else {
                    return Ok(());
                };
                net.disconnect_from_login_server();
                net.connect_to_character_server(korangar_networking::SupportedPacketVersion::_20250716, &login_data, server);
            }
            NetworkEvent::LoginServerConnectionFailed { reason, message } => {
                if matches!(
                    reason,
                    korangar_networking::UnifiedLoginFailedReason::AlreadyLoggedIn
                        | korangar_networking::UnifiedLoginFailedReason::AlreadyOnline
                ) {
                    self.login_busy = true;
                }
                self.fail(format!("login failed ({reason:?}): {message}"));
            }
            NetworkEvent::CharacterServerConnected { .. } => {
                if let Some(net) = self.net.as_mut() {
                    let _ = net.request_character_list();
                }
            }
            NetworkEvent::CharacterServerConnectionFailed { message, .. } => {
                self.fail(format!("character server failed: {message}"));
            }
            NetworkEvent::CharacterList { characters } => {
                if characters.is_empty() && self.list_received {
                    return Ok(());
                }
                self.list_received = true;
                self.characters = characters.iter().map(|character| clean_name(&character.name)).collect();
                self.slots = characters.iter().map(|character| character.character_number).collect();
                if self.phase != Phase::EnteringMap && self.phase != Phase::InWorld {
                    self.phase = Phase::CharacterSelect;
                }
            }
            NetworkEvent::CharacterCreated { character_information } => {
                self.creating = false;
                let slot = usize::from(character_information.character_number);
                self.characters = vec![clean_name(&character_information.name)];
                self.slots = vec![character_information.character_number];
                self.phase = Phase::CharacterSelect;
                if !self.selected_sent {
                    self.dispatch_select(slot)?;
                }
            }
            NetworkEvent::CharacterCreationFailed { reason, message } => {
                self.creating = false;
                self.fail(format!("character creation failed ({reason:?}): {message}"));
            }
            NetworkEvent::CharacterSelected { login_data } => {
                if login_data.server_ip.to_string() != TEST_HOST {
                    self.fail("map server is not Hercules TEST");
                    return Ok(());
                }
                let Some(saved) = self.login_data else {
                    self.fail("map connect without login data");
                    return Ok(());
                };
                self.outbound.push(Outbound::ConnectMap);
                let Some(net) = self.net.as_mut() else {
                    return Ok(());
                };
                net.disconnect_from_character_server();
                net.connect_to_map_server(korangar_networking::SupportedPacketVersion::_20250716, &saved, login_data);
                self.phase = Phase::EnteringMap;
            }
            NetworkEvent::CharacterSelectionFailed { message, .. } => {
                self.fail(format!("character select failed: {message}"));
            }
            NetworkEvent::ChangeMap { map_name, position } => {
                self.map_world.on_map_change(&map_name);
                if self.map_name != map_name {
                    self.npcs.clear();
                    self.map_name = map_name;
                }
                self.player_x = Some(position.x);
                self.player_y = Some(position.y);
            }
            NetworkEvent::PlayerMove { destination, .. } => {
                self.player_x = Some(destination.x);
                self.player_y = Some(destination.y);
            }
            NetworkEvent::UpdateClientTick { .. } => {
                if !self.map_loaded && self.phase == Phase::EnteringMap {
                    self.map_loaded = true;
                    if let Some(net) = self.net.as_mut() {
                        let _ = net.map_loaded();
                    }
                    self.outbound.push(Outbound::MapLoaded);
                    self.phase = Phase::InWorld;
                    self.windows
                        .retain(|class| *class != crate::interface::windows::WindowClass::CharacterSelection);
                }
            }
            NetworkEvent::AddEntity { entity_data } => {
                let job = entity_data.job_id.0;
                if is_npc(job) {
                    let id = entity_data.entity_id.0;
                    if !self.npcs.iter().any(|npc| npc.id == id) {
                        self.npcs.push(super::NpcSpot {
                            id,
                            x: entity_data.position.x,
                            y: entity_data.position.y,
                        });
                    }
                }
            }
            NetworkEvent::OpenDialog { text, npc_id } => self.show_page(npc_id.0, &text),
            NetworkEvent::AddNextButton { npc_id } => self.mark_next(npc_id.0),
            NetworkEvent::AddCloseButton { npc_id } => self.mark_close(npc_id.0),
            NetworkEvent::AddChoiceButtons { choices, npc_id } => self.show_choices(npc_id.0, choices),
            NetworkEvent::StatusChange {
                status_index,
                entity_id,
                state,
                total_duration_milliseconds,
                remaining_milliseconds,
                ..
            } => self.status_strip.on_status_change(
                entity_id.0,
                status_index,
                state,
                remaining_milliseconds,
                total_duration_milliseconds,
                session_tick(),
            ),
            NetworkEvent::StatusChangeSequence {
                status_index,
                entity_id,
                state,
            } => self
                .status_strip
                .on_status_change(entity_id.0, status_index, state, 0, 0, session_tick()),
            NetworkEvent::MapServerDisconnected { reason } => {
                if reason != korangar_networking::DisconnectReason::ClosedByClient {
                    self.fail(format!("map disconnected ({reason:?})"));
                }
            }
            NetworkEvent::LoginServerDisconnected { .. } | NetworkEvent::CharacterServerDisconnected { .. } => {}
            NetworkEvent::SpecialEffect { effect_id, .. } => {
                let _ = self.map_world.note_effect(crate::world::effects::effect_id_value(effect_id));
            }
            NetworkEvent::MapProperty { property_type, flags } => self.map_world.apply_property(property_type, flags),
            NetworkEvent::MapType { map_type } => self.map_world.apply_map_type(map_type),
            NetworkEvent::ChangeMapCell {
                position,
                cell_type,
                map_name,
            } => self.map_world.apply_cell(position, cell_type, &map_name),
            event => self.companion_note.observe(&event),
        }
        Ok(())
    }

    fn reset_links(&mut self) {
        if let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) {
            net.disconnect_from_map_server();
            net.disconnect_from_character_server();
            net.disconnect_from_login_server();
            net.get_events(events);
            let _ = events.drain();
        }
        self.phase = Phase::Idle;
        self.list_received = false;
        self.map_loaded = false;
        self.selected_sent = false;
        self.creating = false;
        self.login_data = None;
        self.servers.clear();
    }

    pub(super) fn take_failure(&mut self) -> Result<(), String> {
        if let Some(message) = self.failure.clone() {
            return Err(message);
        }
        Ok(())
    }

    fn timeout(&self, step: &str) -> String {
        format!(
            "timed out waiting for {step} (phase={:?}, windows={:?}, npcs={})",
            self.phase,
            self.window_names(),
            self.npcs.len()
        )
    }
}

/// Milliseconds since the first live status packet path ran; the logic
/// session's stand-in for the client tick.
fn session_tick() -> u32 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START.get_or_init(Instant::now);
    u32::try_from(start.elapsed().as_millis()).unwrap_or(u32::MAX)
}

fn ipv4(address: ragnarok_packets::ServerAddress) -> String {
    Ipv4Addr::from(address).to_string()
}

fn clean_name(name: &str) -> String {
    name.trim_matches(|character: char| character.is_whitespace() || character == '\0')
        .to_owned()
}

fn is_npc(job: u16) -> bool {
    (41..1000).contains(&job) && job != 45 && job != 139
}

fn chebyshev(x0: u16, y0: u16, x1: u16, y1: u16) -> u16 {
    x0.abs_diff(x1).max(y0.abs_diff(y1))
}

fn account_lock() -> Result<std::fs::File, String> {
    let path = std::env::temp_dir().join("korangar-e2e-account.lock");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|_| "cannot open the e2e account lock".to_owned())?;
    lock_exclusive(&file)?;
    let _ = file.write_all(format!("{}\n", std::process::id()).as_bytes());
    Ok(file)
}

#[cfg(unix)]
fn lock_exclusive(file: &std::fs::File) -> Result<(), String> {
    // SAFETY: `fd` is the lock file we just opened, and `LOCK_EX` blocks until the
    // previous live scenario releases it. The file stays open for the session.
    let rc = unsafe { flock(file.as_raw_fd(), LOCK_EX) };
    if rc != 0 {
        return Err("cannot lock the e2e account".to_owned());
    }
    Ok(())
}

#[cfg(not(unix))]
fn lock_exclusive(_file: &std::fs::File) -> Result<(), String> {
    // Live TEST runs on CT119. Windows has no `flock`; a no-op keeps the crate
    // building.
    Ok(())
}

#[cfg(unix)]
const LOCK_EX: i32 = 2;

#[cfg(unix)]
unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}
