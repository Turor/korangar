use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::StateElement;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{format_warpra_destinations, format_warpra_result, push_warpra_chunk};
use ragnarok_packets::{WARPRA_ACTION_BUY, WARPRA_ACTION_LIST, WARPRA_ACTION_WARP, WarpraDestinationEntry};
use rust_state::{RustState, State};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Warpra catalog and the destination the player wants to warp to or unlock.
#[derive(RustState, StateElement)]
pub struct WarpraWindowState {
    status: String,
    list_text: String,
    kind_text: String,
    location_text: String,
    kind_ghost: String,
    location_ghost: String,
    zeny: i32,
    price: i32,
    destinations: Vec<WarpraDestinationEntry>,
    pending: Vec<WarpraDestinationEntry>,
}

impl Default for WarpraWindowState {
    fn default() -> Self {
        Self {
            status: "Open Warpra to load towns, fields, dungeons, and special maps.".to_owned(),
            list_text: String::new(),
            kind_text: "0".to_owned(),
            location_text: String::new(),
            kind_ghost: "kind".to_owned(),
            location_ghost: "location id".to_owned(),
            zeny: 0,
            price: 0,
            destinations: Vec::new(),
            pending: Vec::new(),
        }
    }
}

impl WarpraWindowState {
    /// Drop an in-flight catalog so the next list cannot prepend those rows.
    pub fn begin_list_request(&mut self) {
        self.pending.clear();
    }

    /// Drop the catalog when the map or character changes.
    pub fn clear_catalog(&mut self) {
        self.pending.clear();
        self.destinations.clear();
        self.list_text.clear();
        self.zeny = 0;
        self.price = 0;
        self.status = "Open Warpra to load towns, fields, dungeons, and special maps.".to_owned();
    }

    pub fn apply_list(&mut self, zeny: i32, price: i32, flags: u8, total: u16, entries: Vec<WarpraDestinationEntry>) {
        self.zeny = zeny;
        self.price = price;
        match push_warpra_chunk(&mut self.pending, flags, total, entries) {
            None => {
                self.status = format!("Loading destinations ({}/{}). Zeny {}.", self.pending.len(), total, self.zeny);
            }
            Some(destinations) => {
                self.destinations = destinations;
                self.list_text = format_warpra_destinations(&self.destinations);
                self.status = format!(
                    "{} destinations. Unlock price {}. Zeny {}.",
                    self.destinations.len(),
                    self.price,
                    self.zeny
                );
            }
        }
    }

    pub fn show_result(&mut self, result: i16, kind: i16, location_id: i16) {
        let name = self
            .destinations
            .iter()
            .find(|entry| i16::from(entry.kind) == kind && entry.id == location_id)
            .map(|entry| entry.name.clone());
        if result == 0 {
            if let Some(entry) = self
                .destinations
                .iter_mut()
                .find(|entry| i16::from(entry.kind) == kind && entry.id == location_id)
            {
                entry.unlocked = 1;
            }
            self.list_text = format_warpra_destinations(&self.destinations);
        }
        self.status = format_warpra_result(result, kind, location_id, name.as_deref(), Some(self.price));
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn list_action() -> i16 {
        WARPRA_ACTION_LIST
    }

    pub fn warp_request(&self) -> Result<(i16, i16), String> {
        self.selected()
    }

    pub fn buy_request(&self) -> Result<(i16, i16), String> {
        self.selected()
    }

    pub fn warp_action() -> i16 {
        WARPRA_ACTION_WARP
    }

    pub fn buy_action() -> i16 {
        WARPRA_ACTION_BUY
    }

    fn selected(&self) -> Result<(i16, i16), String> {
        let kind = self
            .kind_text
            .trim()
            .parse::<i16>()
            .map_err(|_| "Enter a kind: 0 town, 1 field, 2 dungeon, 3 special.".to_owned())?;
        let location_id = self
            .location_text
            .trim()
            .parse::<i16>()
            .map_err(|_| "Enter a location id.".to_owned())?;
        Ok((kind, location_id))
    }
}

struct KindBox;
struct LocationBox;

pub struct WarpraWindow;

impl WarpraWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for WarpraWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Warpra)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().warpra_window();
        let kind_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};
        let location_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Warpra",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.list_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Kind and location id" },
                text_box! {
                    ghost_text: state_path.kind_ghost(),
                    state: state_path.kind_text(),
                    input_handler: DefaultHandler::<_, _, 4>::new(state_path.kind_text(), kind_action),
                    focus_id: KindBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: state_path.location_ghost(),
                    state: state_path.location_text(),
                    input_handler: DefaultHandler::<_, _, 8>::new(state_path.location_text(), location_action),
                    focus_id: LocationBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Refresh",
                    event: InputEvent::RequestWarpraList,
                },
                button! {
                    text: "Warp",
                    event: InputEvent::RequestWarpraWarp,
                },
                button! {
                    text: "Unlock",
                    event: InputEvent::RequestWarpraBuy,
                },
            ),
        }
    }
}
