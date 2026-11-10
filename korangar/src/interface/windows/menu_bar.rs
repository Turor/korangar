//! Bottom menu bar.
//!
//! One [`MenuBarEntry`] in [`menu_bar_entries`] is one button. A later gap task
//! adds a window by appending a single entry (name, caption, hotkey, optional
//! `menu_icon` texture, window class, and the existing toggle or open/close
//! event). Do not add a Cash Shop entry. Server-driven windows (storage,
//! trade, shops, refine, enchant) stay off this list: the server opens them.
//! There is no separate party window. The roster is a tab on Friends, so
//! Party is not a second button.
//!
//! Related windows share one button ([`super::WindowGroup`]): Character,
//! Journal, Social, Settings and Pet Stable. The button opens the member that
//! was shown last and closes whichever member is open; the member windows
//! switch between each other with their tab strip.
//!
//! Buttons show the roBrowser `menu_icon/bt_*.bmp` art when the client has it
//! and fall back to the caption at the normal button font size. The tooltip
//! carries the window name and hotkey.
//!
//! The bar is [`WindowClass::MenuBar`]. It is not closable, so Escape still
//! closes the top closable window. The last button cycles the layout
//! ([`MenuBarLayout`]): as few rows as fit the screen, two rows, three rows,
//! or a vertical strip. The layout is saved in the interface settings. The
//! first open is centered just above chat and the hotbar. A drag is stored by
//! the same [`super::WindowCache`] anchor path as every other window, and the
//! window stays inside the viewport.

use std::sync::Arc;

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::alignment::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{Anchor, AnchorPoint, CustomWindow, Window};
use rust_state::{Path, PathExt, State};
use serde::{Deserialize, Serialize};

use super::window_groups::WindowGroup;
use super::WindowClass;
use super::placement::{HudFrame, ScreenAnchor, first_open_hud, frame_anchor_offset, hud_bottom_margin};
use crate::graphics::{Color, ScreenPosition, ScreenSize, Texture};
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::renderer::LayoutExt;
use crate::settings::InterfaceSettingsPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

/// In-game window chrome ([`crate::state::theme::InterfaceThemeType::InGame`]).
/// `with_derived_borderless` also inserts `gaps` between the title and the
/// content, so the buttons start below title + gaps + title gap.
const MENU_BAR_TITLE: f32 = 22.0;
const MENU_BAR_GAPS: f32 = 6.0;
const MENU_BAR_TITLE_GAP: f32 = 2.0;
const MENU_BAR_BORDER: f32 = 8.0;
/// Space above the buttons (title strip, the drag handle) and below them.
const MENU_BAR_CHROME_HEIGHT: f32 = MENU_BAR_TITLE + MENU_BAR_GAPS + MENU_BAR_TITLE_GAP + MENU_BAR_BORDER;
/// One cell. Wide enough for a six-letter caption at the normal button font
/// and tall enough for the 32×34 `menu_icon` art.
pub const MENU_BAR_CELL_WIDTH: f32 = 56.0;
pub const MENU_BAR_CELL_HEIGHT: f32 = 36.0;
const BUTTON_GAP: f32 = 4.0;
/// `menu_icon/bt_*.bmp` size.
const ICON_WIDTH: f32 = 32.0;
const ICON_HEIGHT: f32 = 34.0;
/// Empty margin around the preset frame.
const MENU_BAR_EDGE_MARGIN: f32 = 8.0;
/// A read that failed is retried this many times before the caption is used.
const ICON_ATTEMPTS: u8 = 3;

/// How the buttons are arranged. Saved in the interface settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MenuBarLayout {
    /// As few rows as fit the screen width.
    #[default]
    Auto,
    TwoRows,
    ThreeRows,
    /// Columns of buttons, as few columns as fit the screen height.
    Vertical,
}

impl MenuBarLayout {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::TwoRows,
            Self::TwoRows => Self::ThreeRows,
            Self::ThreeRows => Self::Vertical,
            Self::Vertical => Self::Auto,
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Self::Auto => "Layout: fit the screen (click for 2 rows)",
            Self::TwoRows => "Layout: 2 rows (click for 3 rows)",
            Self::ThreeRows => "Layout: 3 rows (click for vertical)",
            Self::Vertical => "Layout: vertical (click to fit the screen)",
        }
    }
}

/// What a menu-bar click should do.
#[derive(Clone, Debug)]
pub enum MenuBarPress {
    /// Run this input event. Toggle events ignore whether the window is open.
    Dispatch(Box<InputEvent>),
    /// Remove the window. Used when the open event has no matching close event.
    Close(WindowClass),
    /// Open the group member that was shown last.
    OpenGroup(WindowGroup),
    /// Close these open group members.
    CloseAll(Vec<WindowClass>),
}

#[derive(Debug)]
enum MenuBarKind {
    /// `event` opens the window when it is closed and closes it when it is
    /// open.
    Toggle(Box<InputEvent>),
    OpenClose {
        open: Box<InputEvent>,
        close: Box<InputEvent>,
    },
    /// `open` shows the window. A second click only removes it.
    OpenOrDismiss(Box<InputEvent>),
    /// One button for a tabbed window group.
    Group(WindowGroup),
}

/// One button.
pub struct MenuBarEntry {
    /// Caption drawn when there is no icon.
    pub caption: &'static str,
    /// Window name, plus the hotkey when the window has one.
    pub tooltip: String,
    /// `menu_icon\\bt_*.bmp`, relative to the interface texture directory.
    pub icon: Option<&'static str>,
    pub class: WindowClass,
    kind: MenuBarKind,
}

impl MenuBarEntry {
    fn new(
        label: &str,
        caption: &'static str,
        hotkey: Option<&str>,
        icon: Option<&'static str>,
        class: WindowClass,
        kind: MenuBarKind,
    ) -> Self {
        let tooltip = match hotkey {
            Some(hotkey) => format!("{label} ({hotkey})"),
            None => label.to_owned(),
        };
        Self {
            caption,
            tooltip,
            icon,
            class,
            kind,
        }
    }

    fn toggle(
        label: &str,
        caption: &'static str,
        hotkey: Option<&str>,
        icon: Option<&'static str>,
        class: WindowClass,
        event: InputEvent,
    ) -> Self {
        Self::new(label, caption, hotkey, icon, class, MenuBarKind::Toggle(Box::new(event)))
    }

    fn open_close(
        label: &str,
        caption: &'static str,
        icon: Option<&'static str>,
        class: WindowClass,
        open: InputEvent,
        close: InputEvent,
    ) -> Self {
        Self::new(label, caption, None, icon, class, MenuBarKind::OpenClose {
            open: Box::new(open),
            close: Box::new(close),
        })
    }

    fn group(caption: &'static str, icon: Option<&'static str>, group: WindowGroup) -> Self {
        let tabs: Vec<&str> = group.tabs().iter().map(|tab| tab.label).collect();
        let label = format!("{}: {}", group.name(), tabs.join(", "));
        Self::new(&label, caption, None, icon, group.tabs()[0].class, MenuBarKind::Group(group))
    }

    /// Whether this button's window, or any window of its group, is open.
    pub fn is_open(&self, is_open: impl Fn(WindowClass) -> bool) -> bool {
        match &self.kind {
            MenuBarKind::Group(group) => group.tabs().iter().any(|tab| is_open(tab.class)),
            _ => is_open(self.class),
        }
    }

    /// Whether this button reaches `class`, directly or through a tab.
    pub fn covers(&self, class: WindowClass) -> bool {
        match &self.kind {
            MenuBarKind::Group(group) => group.contains(class),
            _ => self.class == class,
        }
    }

    fn open_or_dismiss(label: &str, caption: &'static str, class: WindowClass, open: InputEvent) -> Self {
        Self::new(label, caption, None, None, class, MenuBarKind::OpenOrDismiss(Box::new(open)))
    }
}

/// Player windows the bar can open. Order is the button order and the bit
/// index in [`menu_bar_open_bits`].
pub fn menu_bar_entries() -> &'static [MenuBarEntry] {
    use std::sync::OnceLock;

    static ENTRIES: OnceLock<Vec<MenuBarEntry>> = OnceLock::new();
    ENTRIES.get_or_init(menu_bar_entry_list)
}

fn menu_bar_entry_list() -> Vec<MenuBarEntry> {
    use InputEvent::{
        CloseBank, CloseMail, CloseRoulette, OpenBank, OpenEden, OpenMail, OpenRoulette, OpenWarpra, ToggleCartWindow, ToggleChatWindow,
        ToggleEmotesWindow, ToggleHotbarWindow, ToggleInventoryWindow, ToggleItemFinderWindow, ToggleMenuWindow, ToggleMvpMinimap,
        ToggleNavigationWindow, ToggleSkillTreeWindow, ToggleStatusPanelWindow, ToggleWorldMap,
    };
    use WindowClass::{
        Bank, Cart, Chat, Eden, Emotes, Hotbar, Inventory, ItemFinder, Mail, Menu, MvpMinimap, Navigation, Roulette, SkillTree, StatusPanel,
        Warpra, WorldMap,
    };

    let icon = |name: &'static str| Some(name);

    vec![
        MenuBarEntry::group("Char", icon("menu_icon\\bt_status.bmp"), WindowGroup::Character),
        MenuBarEntry::toggle(
            "Inventory",
            "Items",
            Some("Alt+E"),
            icon("menu_icon\\bt_item.bmp"),
            Inventory,
            ToggleInventoryWindow,
        ),
        MenuBarEntry::toggle(
            "Skills",
            "Skills",
            Some("Alt+S"),
            icon("menu_icon\\bt_skill.bmp"),
            SkillTree,
            ToggleSkillTreeWindow,
        ),
        MenuBarEntry::group("Quests", icon("menu_icon\\bt_quest.bmp"), WindowGroup::Journal),
        MenuBarEntry::group("Social", icon("menu_icon\\bt_party.bmp"), WindowGroup::Social),
        MenuBarEntry::open_close("Mail", "Mail", icon("menu_icon\\bt_mail.bmp"), Mail, OpenMail, CloseMail),
        MenuBarEntry::open_close("Bank", "Bank", icon("menu_icon\\bt_bank.bmp"), Bank, OpenBank, CloseBank),
        MenuBarEntry::toggle("Minimap", "Map", None, None, MvpMinimap, ToggleMvpMinimap),
        MenuBarEntry::toggle(
            "World Map",
            "World",
            Some("Alt+0"),
            icon("menu_icon\\bt_map.bmp"),
            WorldMap,
            ToggleWorldMap,
        ),
        MenuBarEntry::toggle("Chat", "Chat", None, None, Chat, ToggleChatWindow),
        MenuBarEntry::group("Option", icon("menu_icon\\bt_option.bmp"), WindowGroup::Settings),
        MenuBarEntry::toggle("Emotes", "Emotes", Some("Alt+1-9"), None, Emotes, ToggleEmotesWindow),
        MenuBarEntry::toggle("Menu", "Menu", None, None, Menu, ToggleMenuWindow),
        MenuBarEntry::toggle("Cart", "Cart", None, None, Cart, ToggleCartWindow),
        MenuBarEntry::toggle("Hotbar", "Hotbar", None, None, Hotbar, ToggleHotbarWindow),
        MenuBarEntry::toggle(
            "Navigation",
            "Nav",
            None,
            icon("menu_icon\\bt_navigation.bmp"),
            Navigation,
            ToggleNavigationWindow,
        ),
        MenuBarEntry::toggle("Item Finder", "Find", Some("Alt+F"), None, ItemFinder, ToggleItemFinderWindow),
        MenuBarEntry::group("Pets", None, WindowGroup::PetStable),
        MenuBarEntry::open_or_dismiss("Eden", "Eden", Eden, OpenEden),
        MenuBarEntry::open_or_dismiss("Warpra", "Warp", Warpra, OpenWarpra),
        MenuBarEntry::open_close("Roulette", "Wheel", None, Roulette, OpenRoulette, CloseRoulette),
        MenuBarEntry::toggle("Basic info", "Info", None, None, StatusPanel, ToggleStatusPanelWindow),
    ]
}

/// Cells on the bar: every entry plus the layout button at the end.
pub fn menu_bar_cell_count() -> usize {
    menu_bar_entries().len() + 1
}

/// Bit `i` is set when entry `i`'s window is open. The menu bar reads this
/// from client state while it lays out.
pub fn menu_bar_open_bits(is_open: impl Fn(WindowClass) -> bool) -> u64 {
    let mut bits = 0;
    for (index, entry) in menu_bar_entries().iter().enumerate() {
        if index < 64 && entry.is_open(&is_open) {
            bits |= 1_u64 << index;
        }
    }
    bits
}

/// Click handler for [`InputEvent::MenuBarPress`].
pub fn press_menu_bar(index: usize, is_open: impl Fn(WindowClass) -> bool) -> Option<MenuBarPress> {
    let entry = menu_bar_entries().get(index)?;
    let open = is_open(entry.class);
    Some(match &entry.kind {
        MenuBarKind::Group(group) => {
            let open = group.open_members(&is_open);
            if open.is_empty() {
                MenuBarPress::OpenGroup(*group)
            } else {
                MenuBarPress::CloseAll(open)
            }
        }
        MenuBarKind::Toggle(event) => MenuBarPress::Dispatch(event.clone()),
        MenuBarKind::OpenClose { open: open_event, close } => MenuBarPress::Dispatch(if open { close.clone() } else { open_event.clone() }),
        MenuBarKind::OpenOrDismiss(open_event) => {
            if open {
                MenuBarPress::Close(entry.class)
            } else {
                MenuBarPress::Dispatch(open_event.clone())
            }
        }
    })
}

/// Window a menu-bar event opens or closes. `None` when the event is not one
/// the bar sends.
#[cfg(test)]
pub fn menu_bar_event_window(event: &InputEvent) -> Option<WindowClass> {
    Some(match event {
        InputEvent::ToggleStatsWindow => WindowClass::Stats,
        InputEvent::ToggleEquipmentWindow => WindowClass::Equipment,
        InputEvent::ToggleInventoryWindow => WindowClass::Inventory,
        InputEvent::ToggleSkillTreeWindow => WindowClass::SkillTree,
        InputEvent::ToggleQuestJournalWindow => WindowClass::QuestJournal,
        InputEvent::ToggleFriendListWindow => WindowClass::FriendList,
        InputEvent::ToggleGuildWindow => WindowClass::Guild,
        InputEvent::ToggleClanWindow => WindowClass::Clan,
        InputEvent::OpenMail | InputEvent::CloseMail => WindowClass::Mail,
        InputEvent::OpenBank | InputEvent::CloseBank => WindowClass::Bank,
        InputEvent::ToggleAchievementsWindow => WindowClass::Achievements,
        InputEvent::ToggleMvpMinimap => WindowClass::MvpMinimap,
        InputEvent::ToggleWorldMap => WindowClass::WorldMap,
        InputEvent::ToggleChatWindow => WindowClass::Chat,
        InputEvent::ToggleInterfaceSettingsWindow => WindowClass::InterfaceSettings,
        InputEvent::ToggleKeybindsWindow => WindowClass::Keybinds,
        InputEvent::ToggleEmotesWindow => WindowClass::Emotes,
        InputEvent::ToggleMenuWindow => WindowClass::Menu,
        InputEvent::ToggleCartWindow => WindowClass::Cart,
        InputEvent::ToggleHotbarWindow => WindowClass::Hotbar,
        InputEvent::ToggleNavigationWindow => WindowClass::Navigation,
        InputEvent::ToggleItemFinderWindow => WindowClass::ItemFinder,
        InputEvent::OpenStable => WindowClass::Stable,
        InputEvent::OpenEden => WindowClass::Eden,
        InputEvent::OpenWarpra => WindowClass::Warpra,
        InputEvent::OpenCompanions => WindowClass::Companions,
        InputEvent::ToggleSwitchEquipWindow => WindowClass::SwitchEquip,
        InputEvent::ToggleAudioSettingsWindow => WindowClass::AudioSettings,
        InputEvent::ToggleGraphicsSettingsWindow => WindowClass::GraphicsSettings,
        InputEvent::ToggleGameSettingsWindow => WindowClass::GameSettings,
        InputEvent::OpenRoulette | InputEvent::CloseRoulette => WindowClass::Roulette,
        InputEvent::ToggleStatusEffectsWindow => WindowClass::StatusEffects,
        InputEvent::ToggleStatusPanelWindow => WindowClass::StatusPanel,
        InputEvent::ToggleCharacterOverviewWindow => WindowClass::CharacterOverview,
        _ => return None,
    })
}

/// Textures for entries that have a `menu_icon` file, filled in by the client
/// while the bar is open. A missing icon keeps the caption.
#[derive(Default)]
pub struct MenuBarIcons {
    textures: Vec<Option<Arc<Texture>>>,
    attempts: Vec<u8>,
    settled: Vec<bool>,
}

impl MenuBarIcons {
    fn ensure_len(&mut self, count: usize) {
        if self.textures.len() != count {
            self.textures = vec![None; count];
            self.attempts = vec![0; count];
            self.settled = vec![false; count];
        }
    }

    /// Up to `budget` entries whose icon still needs a read, as
    /// `(index, file)`.
    pub fn pending(&mut self, budget: usize) -> Vec<(usize, &'static str)> {
        let entries = menu_bar_entries();
        self.ensure_len(entries.len());
        entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.icon.map(|file| (index, file)))
            .filter(|(index, _)| !self.settled[*index])
            .take(budget)
            .collect()
    }

    /// `Some` installs the texture. `None` with `missing` keeps the caption
    /// for good; `None` without it is a failed read that is retried a few
    /// times.
    pub fn resolve(&mut self, index: usize, texture: Option<Arc<Texture>>, missing: bool) {
        self.ensure_len(menu_bar_entries().len());
        let Some(settled) = self.settled.get_mut(index) else {
            return;
        };
        match texture {
            Some(texture) => {
                self.textures[index] = Some(texture);
                *settled = true;
            }
            None => {
                self.attempts[index] = self.attempts[index].saturating_add(1);
                if missing || self.attempts[index] >= ICON_ATTEMPTS {
                    *settled = true;
                }
            }
        }
    }

    fn texture(&self, index: usize) -> Option<Arc<Texture>> {
        self.textures.get(index).cloned().flatten()
    }
}

/// Columns and rows of the button grid for `count` cells.
pub fn menu_bar_grid(layout: MenuBarLayout, count: usize, screen_width: f32, screen_height: f32) -> (usize, usize) {
    let count = count.max(1);
    let (screen_width, screen_height) = finite_screen(screen_width, screen_height);
    let (strip_left, strip_right, bottom) = menu_bar_strip_for(layout, screen_width, screen_height);
    let usable_width = strip_right - strip_left - MENU_BAR_BORDER * 2.0 + BUTTON_GAP;
    let usable_height = bottom - MENU_BAR_EDGE_MARGIN - MENU_BAR_CHROME_HEIGHT + BUTTON_GAP;
    let max_columns = ((usable_width / (MENU_BAR_CELL_WIDTH + BUTTON_GAP)).floor() as usize).max(1);
    let max_rows = ((usable_height / (MENU_BAR_CELL_HEIGHT + BUTTON_GAP)).floor() as usize).max(1);

    let rows_from = |minimum_rows: usize| {
        let mut rows = minimum_rows.clamp(1, count);
        while count.div_ceil(rows) > max_columns && rows < count {
            rows += 1;
        }
        (count.div_ceil(rows), rows)
    };

    match layout {
        MenuBarLayout::Auto => rows_from(1),
        MenuBarLayout::TwoRows => rows_from(2),
        MenuBarLayout::ThreeRows => rows_from(3),
        MenuBarLayout::Vertical => {
            let mut columns = 1;
            while count.div_ceil(columns) > max_rows && columns < count {
                columns += 1;
            }
            (columns, count.div_ceil(columns))
        }
    }
}

/// Layout the layout button switches to: the next mode whose grid differs
/// from the current one on this screen, so a click always changes the bar.
pub fn next_distinct_layout(current: MenuBarLayout, screen_width: f32, screen_height: f32) -> MenuBarLayout {
    let count = menu_bar_cell_count();
    let shape = menu_bar_grid(current, count, screen_width, screen_height);
    let mut next = current.next();
    while next != current && menu_bar_grid(next, count, screen_width, screen_height) == shape {
        next = next.next();
    }
    next
}

/// Outer window size for a grid.
fn menu_bar_window_size(columns: usize, rows: usize) -> (f32, f32) {
    let columns = columns.max(1) as f32;
    let rows = rows.max(1) as f32;
    let width = MENU_BAR_BORDER * 2.0 + columns * MENU_BAR_CELL_WIDTH + (columns - 1.0) * BUTTON_GAP;
    let height = MENU_BAR_CHROME_HEIGHT + rows * MENU_BAR_CELL_HEIGHT + (rows - 1.0) * BUTTON_GAP;
    (width, height)
}

/// Grid position of cell `index`. Rows fill left to right; the vertical strip
/// fills top to bottom.
fn menu_bar_cell_position(layout: MenuBarLayout, columns: usize, rows: usize, index: usize) -> (usize, usize) {
    match layout {
        MenuBarLayout::Vertical => (index / rows.max(1), index % rows.max(1)),
        _ => (index % columns.max(1), index / columns.max(1)),
    }
}

fn menu_bar_cell(content_left: f32, content_top: f32, column: usize, row: usize) -> HudFrame {
    HudFrame {
        left: content_left + (MENU_BAR_CELL_WIDTH + BUTTON_GAP) * column as f32,
        top: content_top + (MENU_BAR_CELL_HEIGHT + BUTTON_GAP) * row as f32,
        width: MENU_BAR_CELL_WIDTH,
        height: MENU_BAR_CELL_HEIGHT,
    }
}

/// Icon rectangle centered in a cell, at the art's own size when it fits.
fn icon_area(cell: Area) -> Area {
    let scale = (cell.width / ICON_WIDTH).min(cell.height / ICON_HEIGHT).min(1.0);
    let width = ICON_WIDTH * scale;
    let height = ICON_HEIGHT * scale;
    Area {
        left: cell.left + (cell.width - width) / 2.0,
        top: cell.top + (cell.height - height) / 2.0,
        width,
        height,
    }
}

/// A zero size is the value before the first resize, so use the client's
/// initial 1280×720 window instead of collapsing the bar.
fn finite_screen(screen_width: f32, screen_height: f32) -> (f32, f32) {
    if screen_width.is_finite() && screen_height.is_finite() && screen_width >= 1.0 && screen_height >= 1.0 {
        (screen_width, screen_height)
    } else {
        (1280.0, 720.0)
    }
}

fn menu_bar_live_layout(state: &ClientState) -> (MenuBarLayout, f32, f32) {
    let size = *client_state().window_size().follow_safe(state);
    let layout = *client_state().interface_settings().menu_bar_layout().follow_safe(state);
    let (width, height) = finite_screen(size.width, size.height);
    (layout, width, height)
}

/// [`menu_bar_live_layout`] for an element, which sees the state wrapper.
fn menu_bar_state_layout(state: &State<ClientState>) -> (MenuBarLayout, f32, f32) {
    let size = *state.get(&client_state().window_size());
    let layout = *state.get(&client_state().interface_settings().menu_bar_layout());
    let (width, height) = finite_screen(size.width, size.height);
    (layout, width, height)
}

fn menu_bar_live_size(state: &ClientState) -> (f32, f32) {
    let (layout, width, height) = menu_bar_live_layout(state);
    let (columns, rows) = menu_bar_grid(layout, menu_bar_cell_count(), width, height);
    menu_bar_window_size(columns, rows)
}

/// The window sizer locks minimum and maximum to the grid, so the size
/// follows the layout and the screen instead of a stale saved size.
fn menu_bar_locked_width(state: &ClientState) -> f32 {
    menu_bar_live_size(state).0
}

fn menu_bar_locked_height(state: &ClientState) -> f32 {
    menu_bar_live_size(state).1
}

/// The menu bar stays opaque: it keeps the theme's window color but ignores
/// any transparency the theme sets, so the world never shows through it.
fn menu_bar_background(state: &ClientState) -> Color {
    let color = client_theme()
        .follow(state)
        .map(|theme| theme.window.background_color)
        .unwrap_or(Color::rgb_u8(0, 0, 0));
    opaque(color)
}

fn opaque(color: Color) -> Color {
    Color { alpha: 1.0, ..color }
}

/// Bottom strip the bar docks into: right of the hotbar (which sits right of
/// chat), down to the same bottom margin as chat. Returns `(left, right,
/// bottom)`.
pub fn menu_bar_dock_strip(screen_width: f32, screen_height: f32) -> (f32, f32, f32) {
    let (screen_width, screen_height) = finite_screen(screen_width, screen_height);
    let hud = first_open_hud(screen_width, screen_height);
    let left = (hud.hotbar.right().max(hud.chat.right()) + MENU_BAR_EDGE_MARGIN).min(screen_width - 1.0);
    let right = (screen_width - MENU_BAR_EDGE_MARGIN).max(left + 1.0);
    let bottom = screen_height - hud_bottom_margin(screen_width, screen_height);
    (left, right, bottom)
}

/// Strip right of chat along the whole bottom edge. Fixed row counts need
/// this width; they sit above the hotbar when they would cover it.
fn menu_bar_wide_strip(screen_width: f32, screen_height: f32) -> (f32, f32, f32) {
    let (screen_width, screen_height) = finite_screen(screen_width, screen_height);
    let hud = first_open_hud(screen_width, screen_height);
    let left = (hud.chat.right() + MENU_BAR_EDGE_MARGIN).min(screen_width - 1.0);
    let right = (screen_width - MENU_BAR_EDGE_MARGIN).max(left + 1.0);
    let bottom = screen_height - hud_bottom_margin(screen_width, screen_height);
    (left, right, bottom)
}

/// Auto and vertical fit the corner right of the hotbar; 2 and 3 rows keep
/// their row count with the wider strip.
fn menu_bar_strip_for(layout: MenuBarLayout, screen_width: f32, screen_height: f32) -> (f32, f32, f32) {
    match layout {
        MenuBarLayout::Auto | MenuBarLayout::Vertical => menu_bar_dock_strip(screen_width, screen_height),
        MenuBarLayout::TwoRows | MenuBarLayout::ThreeRows => menu_bar_wide_strip(screen_width, screen_height),
    }
}

/// Rectangle the bar uses while the user has not moved it, for `layout`.
///
/// Auto and vertical sit on the bottom edge in the corner right of the
/// hotbar. 2 and 3 rows sit on the bottom edge right of chat, or directly
/// above the hotbar when the edge spot would cover it. Every layout hugs the
/// right edge and stays clear of chat and the hotbar.
pub fn default_menu_bar_frame_for(layout: MenuBarLayout, screen_width: f32, screen_height: f32) -> HudFrame {
    let (screen_width, screen_height) = finite_screen(screen_width, screen_height);
    let (columns, rows) = menu_bar_grid(layout, menu_bar_cell_count(), screen_width, screen_height);
    let (width, height) = menu_bar_window_size(columns, rows);
    let width = width.min(screen_width);
    let height = height.min(screen_height);
    let (strip_left, strip_right, bottom) = menu_bar_strip_for(layout, screen_width, screen_height);
    let left = (strip_right - width).max(strip_left.min(screen_width - width)).max(0.0);
    let mut frame = HudFrame {
        left,
        top: (bottom - height).max(0.0),
        width,
        height,
    };
    let hotbar = first_open_hud(screen_width, screen_height).hotbar;
    if frames_intersect(frame, hotbar) {
        frame.top = (hotbar.top - MENU_BAR_EDGE_MARGIN - height).max(0.0);
    }
    frame
}

fn frames_intersect(a: HudFrame, b: HudFrame) -> bool {
    a.left < b.right() && a.right() > b.left && a.top < b.bottom() && a.bottom() > b.top
}

/// Anchor and size for `layout` while the bar is docked.
pub fn docked_menu_bar_window(layout: MenuBarLayout, screen_width: f32, screen_height: f32) -> (Anchor<ClientState>, ScreenSize) {
    let frame = default_menu_bar_frame_for(layout, screen_width, screen_height);
    let (left, top) = frame_anchor_offset(ScreenAnchor::BottomCenter, frame, screen_width, screen_height);
    let anchor = Anchor::at(AnchorPoint::BottomCenter, ScreenPosition { left, top });
    let size = ScreenSize {
        width: frame.width,
        height: frame.height,
    };
    (anchor, size)
}

/// [`default_menu_bar_frame_for`] with the default layout. The window cache
/// has no state access, so the preset uses this.
#[cfg(test)]
pub fn default_menu_bar_frame(screen_width: f32, screen_height: f32) -> HudFrame {
    default_menu_bar_frame_for(MenuBarLayout::default(), screen_width, screen_height)
}

/// Content box under the title, using the same insets as the in-game window
/// resolver.
#[cfg(test)]
fn menu_bar_content_origin(window: HudFrame) -> (f32, f32) {
    (
        window.left + MENU_BAR_BORDER,
        window.top + MENU_BAR_TITLE + MENU_BAR_GAPS + MENU_BAR_TITLE_GAP,
    )
}

/// Every cell of a window laid out with `layout`, in cell order.
#[cfg(test)]
pub fn menu_bar_button_frames(layout: MenuBarLayout, window: HudFrame, screen_width: f32, screen_height: f32) -> Vec<HudFrame> {
    let count = menu_bar_cell_count();
    let (columns, rows) = menu_bar_grid(layout, count, screen_width, screen_height);
    let (left, top) = menu_bar_content_origin(window);
    (0..count)
        .map(|index| {
            let (column, row) = menu_bar_cell_position(layout, columns, rows, index);
            menu_bar_cell(left, top, column, row)
        })
        .collect()
}

/// Anchor and size passed to the window cache when the bar has never been
/// moved.
pub fn default_menu_bar_window(screen_width: f32, screen_height: f32) -> (Anchor<ClientState>, ScreenSize) {
    docked_menu_bar_window(MenuBarLayout::default(), screen_width, screen_height)
}

#[derive(Default)]
pub struct MenuBarWindow;

impl CustomWindow<ClientState> for MenuBarWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::MenuBar)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Menu",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            background_color: ComputedSelector::new_default(menu_bar_background),
            minimum_width: ComputedSelector::new_default(menu_bar_locked_width),
            maximum_width: ComputedSelector::new_default(menu_bar_locked_width),
            minimum_height: ComputedSelector::new_default(menu_bar_locked_height),
            maximum_height: ComputedSelector::new_default(menu_bar_locked_height),
            elements: MenuBarButtons::new(),
        }
    }
}

struct MenuBarClick {
    index: u8,
}

impl ClickHandler<ClientState> for MenuBarClick {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(InputEvent::MenuBarPress { index: self.index });
    }
}

struct MenuBarLayoutClick;

impl ClickHandler<ClientState> for MenuBarLayoutClick {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(InputEvent::CycleMenuBarLayout);
    }
}

struct MenuBarButtons {
    clicks: Vec<MenuBarClick>,
    layout_click: MenuBarLayoutClick,
}

impl MenuBarButtons {
    fn new() -> Self {
        let clicks = (0..menu_bar_entries().len())
            .map(|index| MenuBarClick { index: index as u8 })
            .collect();
        Self {
            clicks,
            layout_click: MenuBarLayoutClick,
        }
    }
}

struct MenuBarLayoutInfo {
    area: Area,
}

impl Element<ClientState> for MenuBarButtons {
    type LayoutInfo = MenuBarLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let (bar_layout, screen_width, screen_height) = menu_bar_state_layout(state);
        let (columns, rows) = menu_bar_grid(bar_layout, menu_bar_cell_count(), screen_width, screen_height);
        let (_, height) = menu_bar_window_size(columns, rows);
        with_single_resolver(resolvers, |resolver| {
            let area = resolver.with_height((height - MENU_BAR_CHROME_HEIGHT).max(1.0));
            MenuBarLayoutInfo { area }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let entries = menu_bar_entries();
        let (bar_layout, screen_width, screen_height) = menu_bar_state_layout(state);
        let count = menu_bar_cell_count();
        let (columns, rows) = menu_bar_grid(bar_layout, count, screen_width, screen_height);

        let theme_path = client_theme();
        let icons_path = client_state().menu_bar_icons();
        let theme = state.get(&theme_path);
        let bits = *state.get(&client_state().menu_bar_open());
        let icons = state.get(&icons_path);
        let content = layout_info.area;
        let idle_background = theme.button.background_color;
        let idle_foreground = theme.button.foreground_color;
        let hover_background = theme.button.hovered_background_color;
        let active = theme.tokens.gold;
        let active_foreground = theme.tokens.bg;
        let corner = theme.button.corner_diameter;
        let shadow = theme.button.shadow_color;
        let shadow_padding = theme.button.shadow_padding;
        let font_size = theme.button.font_size;
        let highlight = theme.button.highlight_color;

        for index in 0..count {
            let (column, row) = menu_bar_cell_position(bar_layout, columns, rows, index);
            let cell = menu_bar_cell(content.left, content.top, column, row);
            let area = Area {
                left: cell.left,
                top: cell.top,
                width: cell.width,
                height: cell.height,
            };
            let hovered = area.check().run(layout);
            let entry = entries.get(index);
            let open = entry.is_some() && index < 64 && bits & (1_u64 << index) != 0;

            if hovered {
                match entry {
                    Some(entry) => {
                        layout.add_tooltip(&entry.tooltip, MenuBarTooltip.tooltip_id());
                        if let Some(click) = self.clicks.get(index) {
                            layout.register_click_handler(korangar_interface::layout::MouseButton::Left, click);
                        }
                    }
                    None => {
                        layout.add_tooltip(bar_layout.tooltip(), MenuBarTooltip.tooltip_id());
                        layout.register_click_handler(korangar_interface::layout::MouseButton::Left, &self.layout_click);
                    }
                }
            }

            // Neutral at rest. Gold only while hovered or while the window is
            // open.
            let (background, foreground) = match (open, hovered) {
                (true, true) => (hover_background, active),
                (true, false) => (active, active_foreground),
                (false, true) => (hover_background, active),
                (false, false) => (idle_background, idle_foreground),
            };
            layout.add_rectangle(area, corner, background, shadow, shadow_padding);

            let texture = entry.and_then(|_| icons.texture(index));
            match texture {
                Some(texture) => layout.add_texture(icon_area(area), texture, Color::WHITE, false),
                None => {
                    let caption = entry.map_or("Layout", |entry| entry.caption);
                    layout.add_text(
                        area,
                        caption,
                        font_size,
                        foreground,
                        highlight,
                        HorizontalAlignment::Center { offset: 0.0, border: 2.0 },
                        VerticalAlignment::Center { offset: 0.0 },
                        OverflowBehavior::Shrink,
                    );
                }
            }
        }
    }
}

struct MenuBarTooltip;

#[cfg(test)]
mod tests {
    use super::{
        MENU_BAR_CELL_HEIGHT, MENU_BAR_CELL_WIDTH, MenuBarIcons, MenuBarKind, MenuBarLayout, MenuBarPress, default_menu_bar_frame,
        default_menu_bar_frame_for, default_menu_bar_window, docked_menu_bar_window, menu_bar_button_frames, menu_bar_cell_count,
        menu_bar_entries, menu_bar_event_window, menu_bar_grid, menu_bar_open_bits, opaque, press_menu_bar,
    };
    use crate::graphics::{Color, ScreenPosition};
    use crate::input::InputEvent;
    use crate::interface::windows::WindowClass;
    use crate::interface::windows::placement::{HudFrame, first_open_hud};

    const LAYOUTS: [MenuBarLayout; 4] = [
        MenuBarLayout::Auto,
        MenuBarLayout::TwoRows,
        MenuBarLayout::ThreeRows,
        MenuBarLayout::Vertical,
    ];
    const SCREENS: [(f32, f32); 4] = [(1280.0, 720.0), (1279.0, 700.0), (1024.0, 576.0), (1024.0, 560.0)];

    fn rects_overlap(a: HudFrame, b: HudFrame) -> bool {
        a.left < b.right() && a.right() > b.left && a.top < b.bottom() && a.bottom() > b.top
    }

    /// Window a menu-bar event opens or closes.
    fn event_window(event: &InputEvent) -> Option<WindowClass> {
        menu_bar_event_window(event)
    }

    #[test]
    fn every_entry_toggles_its_window_and_reports_open() {
        let entries = menu_bar_entries();
        assert!(entries.len() > 8);
        assert!(entries.len() < 64);

        let mut classes = std::collections::HashSet::new();
        for (index, entry) in entries.iter().enumerate() {
            assert!(classes.insert(entry.class), "duplicate class {:?}", entry.class);
            assert!(!entry.caption.is_empty());
            assert!(
                entry.caption.chars().count() <= 6,
                "{} caption is too long for a cell",
                entry.caption
            );
            assert!(!entry.tooltip.is_empty());
            if let Some(icon) = entry.icon {
                assert!(icon.starts_with("menu_icon\\bt_") && icon.ends_with(".bmp"), "{icon}");
            }

            let closed = press_menu_bar(index, |_| false);
            let opened = press_menu_bar(index, |class| class == entry.class);
            match (&entry.kind, closed, opened) {
                (MenuBarKind::Toggle(event), Some(MenuBarPress::Dispatch(closed)), Some(MenuBarPress::Dispatch(opened))) => {
                    assert_eq!(std::mem::discriminant(event.as_ref()), std::mem::discriminant(closed.as_ref()));
                    assert_eq!(std::mem::discriminant(event.as_ref()), std::mem::discriminant(opened.as_ref()));
                    assert_eq!(event_window(closed.as_ref()), Some(entry.class), "{}", entry.tooltip);
                }
                (MenuBarKind::OpenClose { open, close }, Some(MenuBarPress::Dispatch(closed)), Some(MenuBarPress::Dispatch(opened))) => {
                    assert_eq!(
                        std::mem::discriminant(open.as_ref()),
                        std::mem::discriminant(closed.as_ref()),
                        "{} closed press must be the open event",
                        entry.tooltip
                    );
                    assert_eq!(
                        std::mem::discriminant(close.as_ref()),
                        std::mem::discriminant(opened.as_ref()),
                        "{} open press must be the close event",
                        entry.tooltip
                    );
                    assert_ne!(std::mem::discriminant(open.as_ref()), std::mem::discriminant(close.as_ref()));
                    assert_eq!(event_window(closed.as_ref()), Some(entry.class));
                    assert_eq!(event_window(opened.as_ref()), Some(entry.class));
                }
                (MenuBarKind::OpenOrDismiss(open), Some(MenuBarPress::Dispatch(closed)), Some(MenuBarPress::Close(class))) => {
                    assert_eq!(std::mem::discriminant(open.as_ref()), std::mem::discriminant(closed.as_ref()));
                    assert_eq!(class, entry.class);
                    assert_eq!(event_window(closed.as_ref()), Some(entry.class));
                }
                (MenuBarKind::Group(group), Some(MenuBarPress::OpenGroup(closed)), Some(MenuBarPress::CloseAll(opened))) => {
                    assert_eq!(*group, closed);
                    assert_eq!(opened, vec![entry.class]);
                    for tab in group.tabs() {
                        assert!(
                            !entries.iter().any(|other| !matches!(other.kind, MenuBarKind::Group(_)) && other.class == tab.class),
                            "{:?} has its own button and a tab",
                            tab.class
                        );
                    }
                }
                other => panic!("{} press mismatch: {other:?}", entry.tooltip),
            }
        }

        assert!(entries.iter().any(|entry| matches!(entry.kind, MenuBarKind::OpenClose { .. })));
        assert!(entries.iter().any(|entry| entry.class == WindowClass::Stats));
        assert!(entries.iter().any(|entry| entry.class == WindowClass::Emotes));
        assert!(entries.iter().any(|entry| entry.covers(WindowClass::Keybinds)));
        assert!(press_menu_bar(entries.len(), |_| false).is_none());
    }

    #[test]
    fn tooltips_carry_the_name_and_hotkey() {
        let entries = menu_bar_entries();
        let inventory = entries.iter().find(|entry| entry.class == WindowClass::Inventory).unwrap();
        assert_eq!(inventory.tooltip, "Inventory (Alt+E)");
        let settings = entries.iter().find(|entry| entry.covers(WindowClass::AudioSettings)).unwrap();
        assert_eq!(settings.tooltip, "Settings: Interface, Graphics, Audio, Game, Keys");
        let chat = entries.iter().find(|entry| entry.class == WindowClass::Chat).unwrap();
        assert_eq!(chat.tooltip, "Chat");
    }

    #[test]
    fn robrowser_icons_cover_the_main_windows() {
        let with_icon: Vec<&super::MenuBarEntry> = menu_bar_entries().iter().filter(|entry| entry.icon.is_some()).collect();
        for class in [
            WindowClass::Stats,
            WindowClass::Equipment,
            WindowClass::Inventory,
            WindowClass::SkillTree,
            WindowClass::QuestJournal,
            WindowClass::FriendList,
            WindowClass::Guild,
            WindowClass::WorldMap,
        ] {
            assert!(with_icon.iter().any(|entry| entry.covers(class)), "{class:?} has no icon");
        }
    }

    #[test]
    fn open_bits_follow_the_registry_order() {
        let entries = menu_bar_entries();
        let wanted = [WindowClass::Inventory, WindowClass::Stats, WindowClass::Chat];
        let bits = menu_bar_open_bits(|class| wanted.contains(&class));
        for (index, entry) in entries.iter().enumerate() {
            let on = bits & (1_u64 << index) != 0;
            assert_eq!(on, wanted.iter().any(|class| entry.covers(*class)));
        }
        assert_eq!(menu_bar_open_bits(|_| false), 0);
    }

    #[test]
    fn layout_cycles_through_every_mode() {
        let mut layout = MenuBarLayout::default();
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(layout);
            layout = layout.next();
        }
        assert_eq!(layout, MenuBarLayout::default());
        for mode in LAYOUTS {
            assert!(seen.contains(&mode));
        }
    }

    #[test]
    fn the_layout_button_always_changes_the_shape() {
        for (screen_width, screen_height) in SCREENS {
            for layout in LAYOUTS {
                let next = super::next_distinct_layout(layout, screen_width, screen_height);
                let count = menu_bar_cell_count();
                assert_ne!(
                    menu_bar_grid(layout, count, screen_width, screen_height),
                    menu_bar_grid(next, count, screen_width, screen_height),
                    "{layout:?} -> {next:?} at {screen_width}x{screen_height}"
                );
            }
        }
    }

    #[test]
    fn grid_shapes_follow_the_layout() {
        let count = menu_bar_cell_count();
        let (columns, rows) = menu_bar_grid(MenuBarLayout::TwoRows, count, 1920.0, 1080.0);
        assert_eq!(rows, 2);
        assert!(columns * rows >= count);
        let (_, rows) = menu_bar_grid(MenuBarLayout::ThreeRows, count, 1920.0, 1080.0);
        assert_eq!(rows, 3);
        let (columns, rows) = menu_bar_grid(MenuBarLayout::Vertical, count, 1280.0, 720.0);
        assert!(rows > columns, "vertical is {columns}x{rows}");
        // Auto uses as few rows as the corner beside the hotbar allows.
        let (_, wide) = menu_bar_grid(MenuBarLayout::Auto, count, 3840.0, 2160.0);
        assert!(wide <= 2, "auto at 4K uses {wide} rows");
    }

    #[test]
    fn every_layout_keeps_full_size_buttons_on_screen() {
        for (screen_width, screen_height) in SCREENS {
            for layout in LAYOUTS {
                let frame = default_menu_bar_frame_for(layout, screen_width, screen_height);
                assert!(
                    frame.left >= -0.5 && frame.top >= -0.5,
                    "{layout:?} {screen_width}x{screen_height}"
                );
                assert!(
                    frame.right() <= screen_width + 0.5,
                    "{layout:?} {screen_width}x{screen_height} too wide"
                );
                assert!(
                    frame.bottom() <= screen_height + 0.5,
                    "{layout:?} {screen_width}x{screen_height} too tall"
                );

                let buttons = menu_bar_button_frames(layout, frame, screen_width, screen_height);
                assert_eq!(buttons.len(), menu_bar_cell_count());
                for (index, button) in buttons.iter().enumerate() {
                    // Captions keep the normal font: no cell shrinks.
                    assert_eq!(button.width, MENU_BAR_CELL_WIDTH);
                    assert_eq!(button.height, MENU_BAR_CELL_HEIGHT);
                    assert!(
                        button.left + 0.5 >= frame.left && button.right() <= frame.right() + 0.5,
                        "{layout:?} cell {index}"
                    );
                    assert!(
                        button.top + 0.5 >= frame.top && button.bottom() <= frame.bottom() + 0.5,
                        "{layout:?} cell {index}"
                    );
                    for other in &buttons[index + 1..] {
                        assert!(!rects_overlap(*button, *other), "{layout:?} cells overlap");
                    }
                }
            }
        }
    }

    #[test]
    fn default_dock_sits_on_the_bottom_edge_clear_of_chat_and_the_hotbar() {
        for (screen_width, screen_height) in [(1280.0, 720.0), (1279.0, 700.0), (1024.0, 560.0), (1024.0, 576.0), (1920.0, 1080.0)] {
            let hud = first_open_hud(screen_width, screen_height);
            let bottom = screen_height - crate::interface::windows::placement::hud_bottom_margin(screen_width, screen_height);
            for layout in [
                MenuBarLayout::Auto,
                MenuBarLayout::TwoRows,
                MenuBarLayout::ThreeRows,
                MenuBarLayout::Vertical,
            ] {
                let frame = default_menu_bar_frame_for(layout, screen_width, screen_height);
                let label = format!("{layout:?} at {screen_width}x{screen_height}");
                let on_edge = (frame.top + frame.height - bottom).abs() < 0.5;
                if matches!(layout, MenuBarLayout::Auto | MenuBarLayout::Vertical) {
                    assert!(on_edge, "{label}: not on the bottom edge");
                } else {
                    let above_hotbar = frame.top + frame.height <= hud.hotbar.top + 0.5;
                    assert!(on_edge || above_hotbar, "{label}: neither on the edge nor above the hotbar");
                }
                assert!(
                    frame.left >= 0.0 && frame.left + frame.width <= screen_width + 0.01,
                    "{label}: off screen"
                );
                assert!(frame.top >= 0.0, "{label}: above the screen");
                assert!(!rects_overlap(frame, hud.chat), "{label}: covers chat");
                assert!(!rects_overlap(frame, hud.hotbar), "{label}: covers the hotbar");
                // Mid-screen over Gear was the old default. Small screens have no
                // room beside the 500px Gear window.
                if layout == MenuBarLayout::Auto && screen_height >= 700.0 {
                    assert!(!rects_overlap(frame, hud.gear), "{label}: covers Gear");
                }
            }
            assert!(hud.hotbar.top >= 0.0, "hotbar above the top at {screen_width}x{screen_height}");
        }
    }

    #[test]
    fn a_resize_redocks_to_the_new_bottom_edge() {
        let (anchor, size) = docked_menu_bar_window(MenuBarLayout::Auto, 1280.0, 720.0);
        let small = crate::graphics::ScreenSize {
            width: 1024.0,
            height: 560.0,
        };
        let (redocked, redocked_size) = docked_menu_bar_window(MenuBarLayout::Auto, small.width, small.height);
        let expected = default_menu_bar_frame_for(MenuBarLayout::Auto, small.width, small.height);
        let position = redocked.to_position(small);
        assert!((position.left - expected.left).abs() < 0.5);
        assert!((position.top - expected.top).abs() < 0.5);
        assert!((redocked_size.width - expected.width).abs() < 0.5);
        // The 1280 anchor, read on the smaller screen, is not the new dock.
        let stale = anchor.to_position(small);
        assert!((stale.left - expected.left).abs() > 0.5 || (size.width - expected.width).abs() > 0.5);
    }

    #[test]
    fn menu_bar_starts_docked_and_remembers_a_drag() {
        let screen = crate::graphics::ScreenSize {
            width: 1280.0,
            height: 720.0,
        };
        let frame = default_menu_bar_frame(screen.width, screen.height);
        let (mut anchor, size) = default_menu_bar_window(screen.width, screen.height);
        let start = anchor.to_position(screen);
        assert!((start.left - frame.left).abs() < 0.5);
        assert!((start.top - frame.top).abs() < 0.5);

        let dragged = ScreenPosition { left: 120.0, top: 80.0 };
        anchor.update(screen, dragged, size);
        let remembered = anchor.to_position(screen);
        assert!((remembered.left - dragged.left).abs() < 0.5);
        assert!((remembered.top - dragged.top).abs() < 0.5);
    }

    #[test]
    fn icons_fall_back_to_captions_after_a_miss() {
        let mut icons = MenuBarIcons::default();
        let pending = icons.pending(64);
        assert!(!pending.is_empty());
        assert!(pending.iter().all(|(index, _)| menu_bar_entries()[*index].icon.is_some()));

        let (first, _) = pending[0];
        icons.resolve(first, None, true);
        assert!(icons.texture(first).is_none());
        assert!(
            icons.pending(64).iter().all(|(index, _)| *index != first),
            "a confirmed miss is not retried"
        );

        let (second, _) = pending[1];
        for _ in 0..super::ICON_ATTEMPTS {
            assert!(icons.pending(64).iter().any(|(index, _)| *index == second));
            icons.resolve(second, None, false);
        }
        assert!(
            icons.pending(64).iter().all(|(index, _)| *index != second),
            "failed reads stop after a few tries"
        );
        assert_eq!(icons.pending(2).len(), 2);
    }

    #[test]
    fn menu_bar_background_is_always_opaque() {
        let translucent = Color {
            red: 0.1,
            green: 0.2,
            blue: 0.3,
            alpha: 0.4,
        };
        let solid = opaque(translucent);
        assert_eq!(solid.alpha, 1.0);
        assert_eq!((solid.red, solid.green, solid.blue), (0.1, 0.2, 0.3));
    }
}
