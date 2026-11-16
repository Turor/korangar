//! Minimap harness for logic-mode and live scenarios.
//!
//! The offline path uses the real input system, navigation marks, keybind map,
//! and zoom step. It does not open a swapchain.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use ragnarok_packets::ClientTick;
use winit::keyboard::KeyCode;

use crate::input::{InputEvent, InputSystem};
use crate::interface::windows::minimap::{self, MINIMAP_CLOSABLE, MINIMAP_SIZE, MvpMinimapState};
use crate::settings::{KeybindMap, edges_from_transitions};
use crate::state::navigation::{MinimapMarkUpdate, Navigation};
use crate::world::display_name_from_table;

/// What a scenario asserts. Coordinates are canvas pixels, not a screenshot.
#[derive(Clone, Debug, PartialEq)]
pub struct MinimapMarkView {
    pub slot: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MinimapView {
    pub open: bool,
    pub closable: bool,
    pub hidden: bool,
    pub title: String,
    pub caption: String,
    pub map_key: String,
    pub player_x: u16,
    pub player_y: u16,
    pub zoom: u8,
    pub marks: Vec<MinimapMarkView>,
    pub texture_missing: bool,
}

/// Offline stand-in for the always-on minimap. `open` stays true across Escape,
/// the menu bar, and hide. `hidden` is the interface-visibility flag.
pub struct MinimapHarness {
    input: InputSystem,
    tick: u32,
    board: MvpMinimapState,
    navigation: Navigation,
    zoom: u8,
    open: bool,
    hidden: bool,
}

impl MinimapHarness {
    pub fn new() -> Self {
        Self {
            input: InputSystem::new(Arc::new(AtomicU64::new(0))),
            tick: 0,
            board: MvpMinimapState::default(),
            navigation: Navigation::default(),
            zoom: 0,
            open: false,
            hidden: false,
        }
    }

    pub fn enter_map(&mut self, map_name: &str, x: u16, y: u16) {
        self.navigation.on_map_change(map_name, x, y);
        let title = display_name_from_table(&hashbrown::HashMap::new(), map_name);
        self.board.set_display_name(title);
        self.board.set_map_id(map_name);
        self.board.set_player_cell(x, y);
        self.board.set_map_size(300, 300);
        self.open = true;
    }

    pub fn feed_mark(&mut self, kind: u8, slot: u8, x: u32, y: u32, color: (u8, u8, u8)) {
        self.navigation.apply_mark(MinimapMarkUpdate {
            npc_id: 1,
            kind,
            x,
            y,
            slot,
            red: color.0,
            green: color.1,
            blue: color.2,
            now: self.tick,
        });
    }

    /// Injected Escape. A non-closable minimap stays open.
    pub fn press_escape(&mut self) -> bool {
        self.input.expire_synthesized();
        self.input.synthesize_key(KeyCode::Escape, false, false, false);
        let (_report, events) = self.finish_frame();
        let saw = events.iter().any(|event| matches!(event, InputEvent::Escape));
        if saw && MINIMAP_CLOSABLE {
            self.open = false;
        }
        saw
    }

    /// Apply every menu-bar press aimed at the minimap. A close press clears
    /// `open`.
    pub fn press_menu_bar(&mut self) {
        if minimap::menu_bar_closes_minimap(self.open) {
            self.open = false;
        }
    }

    /// Injected left click, then the same zoom step the +/- buttons use.
    pub fn click_zoom_in(&mut self) -> bool {
        self.input.expire_synthesized();
        self.input
            .synthesize_click(24.0, 24.0, korangar_interface::layout::MouseButton::Left);
        let (report, _) = self.finish_frame();
        let clicked = matches!(
            report.mouse_click,
            Some(korangar_interface::layout::MouseButton::Left | korangar_interface::layout::MouseButton::DoubleLeft)
        );
        if clicked {
            self.zoom = minimap::step_zoom(self.zoom, 1);
        }
        clicked
    }

    /// Ctrl+H through the default keybind map. The window stays open.
    pub fn press_hide_interface(&mut self) -> bool {
        self.input.expire_synthesized();
        self.input.synthesize_key(KeyCode::KeyH, false, true, false);
        let _report = self.input.update_delta(ClientTick(self.tick));
        self.tick = self.tick.wrapping_add(16);
        let edges = edges_from_transitions(
            self.input.key_down(KeyCode::AltLeft) || self.input.key_down(KeyCode::AltRight),
            self.input.key_down(KeyCode::ControlLeft) || self.input.key_down(KeyCode::ControlRight),
            self.input.key_down(KeyCode::ShiftLeft) || self.input.key_down(KeyCode::ShiftRight),
            false,
            |code| (self.input.key_pressed(code), self.input.key_released(code)),
        );
        let outcome = KeybindMap::default().poll(None, 0, 0, &edges);
        let toggled = outcome.events.iter().any(|event| matches!(event, InputEvent::ToggleShowInterface));
        if toggled {
            self.hidden = !self.hidden;
        }
        toggled
    }

    pub fn view(&self) -> MinimapView {
        let (map_width, map_height) = self.board.map_size();
        let (player_x, player_y) = self.board.player_cell();
        let window = minimap::map_window(self.zoom, map_width, map_height, player_x, player_y);
        let marks = minimap::plot_marks(self.navigation.marks(), window, map_width, map_height, MINIMAP_SIZE)
            .into_iter()
            .map(|mark| MinimapMarkView {
                slot: mark.slot,
                red: mark.red,
                green: mark.green,
                blue: mark.blue,
            })
            .collect();
        let map_key = display_name_from_table(&hashbrown::HashMap::new(), self.board.map_id());
        MinimapView {
            open: self.open,
            closable: MINIMAP_CLOSABLE,
            hidden: self.hidden,
            title: self.board.title().to_owned(),
            caption: self.board.caption().to_owned(),
            map_key,
            player_x,
            player_y,
            zoom: self.zoom,
            marks,
            texture_missing: self.board.texture_missing(),
        }
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
}

impl Default for MinimapHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Live view from the session's map and cell. No GRF and no display-name table.
pub(crate) fn view_for_session(in_world: bool, map_name: &str, player_x: Option<u16>, player_y: Option<u16>) -> MinimapView {
    let title = if map_name.is_empty() {
        "Minimap".to_owned()
    } else {
        display_name_from_table(&hashbrown::HashMap::new(), map_name)
    };
    let map_key = display_name_from_table(&hashbrown::HashMap::new(), map_name);
    let player_x = player_x.unwrap_or(0);
    let player_y = player_y.unwrap_or(0);
    MinimapView {
        open: in_world && !map_name.trim().is_empty(),
        closable: MINIMAP_CLOSABLE,
        hidden: false,
        caption: format!("{title} ({player_x}, {player_y})"),
        title,
        map_key,
        player_x,
        player_y,
        zoom: 0,
        marks: Vec::new(),
        texture_missing: false,
    }
}
