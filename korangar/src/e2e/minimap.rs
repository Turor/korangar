//! Minimap harness for logic-mode and live scenarios.
//!
//! Open, Escape, zoom, and hide go through the same helpers the client calls.
//! The harness does not open a swapchain and does not drop `InterfaceSettings`
//! (that type writes `client/interface_settings.ron` on drop).

use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use korangar_interface::layout::area::Area;
use ragnarok_packets::ClientTick;
use winit::keyboard::KeyCode;

use crate::input::{InputEvent, InputSystem};
use crate::interface::windows::minimap::{self, MINIMAP_CLOSABLE, MINIMAP_SIZE, MvpMinimapState};
use crate::settings::{InterfaceSettings, KeybindMap, edges_from_transitions};
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

/// Offline stand-in for the always-on minimap. `open` follows
/// [`minimap::minimap_open_after_map_entry`]. Hide flips `show_interface`
/// and leaves the window open.
pub struct MinimapHarness {
    input: InputSystem,
    tick: u32,
    board: MvpMinimapState,
    navigation: Navigation,
    /// Not dropped: `InterfaceSettings::drop` would rewrite the settings file.
    settings: ManuallyDrop<InterfaceSettings>,
    open: bool,
    show_interface: bool,
}

impl MinimapHarness {
    pub fn new() -> Self {
        Self {
            input: InputSystem::new(Arc::new(AtomicU64::new(0))),
            tick: 0,
            board: MvpMinimapState::default(),
            navigation: Navigation::default(),
            settings: ManuallyDrop::new(InterfaceSettings::default()),
            open: false,
            show_interface: true,
        }
    }

    pub fn enter_map(&mut self, map_name: &str, x: u16, y: u16) {
        self.navigation.on_map_change(map_name, x, y);
        let title = display_name_from_table(&hashbrown::HashMap::new(), map_name);
        self.board.set_display_name(title);
        self.board.set_map_id(map_name);
        self.board.set_player_cell(x, y);
        self.board.set_map_size(300, 300);
        // A map entry in logic mode is a player in the world, the same bit
        // `ensure_mvp_minimap_open` reads before it opens the window.
        self.open = minimap::minimap_open_after_map_entry(true, self.open);
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
        if saw && minimap::escape_closes_minimap(MINIMAP_CLOSABLE) {
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

    /// Injected left click on the + button. Any other click leaves zoom alone.
    /// The write is `write_minimap_zoom`, the same helper the click handler
    /// uses.
    pub fn click_zoom_in(&mut self) -> bool {
        let canvas = minimap_canvas();
        let (_minus, plus) = minimap::zoom_button_areas(canvas);
        let x = plus.left + plus.width / 2.0;
        let y = plus.top + plus.height / 2.0;
        self.input.expire_synthesized();
        self.input.synthesize_click(x, y, korangar_interface::layout::MouseButton::Left);
        let (report, _) = self.finish_frame();
        let clicked = matches!(
            report.mouse_click,
            Some(korangar_interface::layout::MouseButton::Left | korangar_interface::layout::MouseButton::DoubleLeft)
        );
        let delta = minimap::zoom_delta_at(canvas, self.settings.minimap_zoom, x, y);
        if clicked && delta == 1 {
            minimap::write_minimap_zoom(&mut self.settings, delta);
        }
        clicked && delta == 1
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
            self.show_interface = minimap::toggle_show_interface(self.show_interface);
        }
        toggled
    }

    pub fn view(&self) -> MinimapView {
        let (map_width, map_height) = self.board.map_size();
        let (player_x, player_y) = self.board.player_cell();
        let window = minimap::map_window(self.settings.minimap_zoom, map_width, map_height, player_x, player_y);
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
            hidden: !self.show_interface,
            title: self.board.title().to_owned(),
            caption: self.board.caption().to_owned(),
            map_key,
            player_x,
            player_y,
            zoom: self.settings.minimap_zoom,
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

fn minimap_canvas() -> Area {
    Area {
        left: 0.0,
        top: 0.0,
        width: MINIMAP_SIZE,
        height: MINIMAP_SIZE,
    }
}

/// Live view from the session's map and cell. The caption is
/// `MvpMinimapState::caption` after the same setters ChangeMap uses.
/// `open` is [`minimap::minimap_open_after_map_entry`]. No GRF.
pub(crate) fn view_for_session(in_world: bool, map_name: &str, player_x: Option<u16>, player_y: Option<u16>) -> MinimapView {
    let mut board = MvpMinimapState::default();
    let map_key = display_name_from_table(&hashbrown::HashMap::new(), map_name);
    if !map_name.trim().is_empty() {
        board.set_display_name(map_key.clone());
        board.set_map_id(map_name);
    }
    if let (Some(x), Some(y)) = (player_x, player_y) {
        board.set_player_cell(x, y);
    }
    let (board_x, board_y) = board.player_cell();
    MinimapView {
        open: minimap::minimap_open_after_map_entry(in_world, false),
        closable: MINIMAP_CLOSABLE,
        hidden: false,
        caption: board.caption().to_owned(),
        title: board.title().to_owned(),
        map_key,
        player_x: board_x,
        player_y: board_y,
        zoom: 0,
        marks: Vec::new(),
        texture_missing: board.texture_missing(),
    }
}
