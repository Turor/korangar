//! Bottom menu bar.
//!
//! One [`MenuBarEntry`] in [`menu_bar_entries`] is one button. A later gap task
//! adds a window by appending a single entry (id, label, icon, hotkey, window
//! class, and the existing toggle or open/close event). Do not add a Cash Shop
//! entry. Server-driven windows (storage, trade, shops, refine, enchant) stay
//! off this list: the server opens them. There is no separate party window.
//! The invite prompt is its own window, and the roster is a tab on Friends
//! when that tab is present, so Party is not a second button. Friends stays
//! on Alt+Z.
//!
//! The bar is [`WindowClass::MenuBar`]. It is not closable, so Escape still
//! closes the top closable window or toggles the old menu. The first open is
//! one row of buttons, centered, and as low as it can sit without covering
//! chat or the hotbar. Width is the screen width minus a margin on each side,
//! and the window's minimum and maximum size follow that preset so a narrower
//! client does not keep a wider bar. A drag is stored by the same
//! [`super::WindowCache`] anchor path as every other window.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::alignment::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{Anchor, AnchorPoint, CustomWindow, Window};
use rust_state::{PathExt, State};

use super::WindowClass;
use super::placement::{HudFrame, ScreenAnchor, first_open_hud, frame_anchor_offset};
use crate::graphics::{ScreenPosition, ScreenSize};
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

/// In-game window chrome ([`crate::state::theme::InterfaceThemeType::InGame`]).
/// `with_derived_borderless` also inserts `gaps` between the title and the
/// content, so the button row starts below title + gaps + title gap.
const MENU_BAR_TITLE: f32 = 22.0;
const MENU_BAR_GAPS: f32 = 6.0;
const MENU_BAR_TITLE_GAP: f32 = 2.0;
const MENU_BAR_BORDER: f32 = 8.0;
/// One button, matching the in-game button height.
const MENU_BAR_BUTTON_HEIGHT: f32 = 20.0;
const BUTTON_GAP: f32 = 4.0;
/// Empty margin on the left and right of the preset frame.
const MENU_BAR_EDGE_MARGIN: f32 = 8.0;
/// Space left between the bar and chat or the hotbar, whichever is higher.
const MENU_BAR_HUD_CLEARANCE: f32 = 4.0;

/// Height of one button row plus the title strip used as the drag handle.
pub const MENU_BAR_WINDOW_HEIGHT: f32 = MENU_BAR_TITLE + MENU_BAR_GAPS + MENU_BAR_TITLE_GAP + MENU_BAR_BORDER + MENU_BAR_BUTTON_HEIGHT;

/// What a menu-bar click should do.
#[derive(Clone, Debug)]
pub enum MenuBarPress {
    /// Run this input event. Toggle events ignore whether the window is open.
    Dispatch(Box<InputEvent>),
    /// Remove the window. Used when the open event has no matching close event.
    Close(WindowClass),
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
}

/// One button. `icon` is the caption drawn on the button. `label` is the name
/// in the tooltip, together with `hotkey` when the window has one.
pub struct MenuBarEntry {
    /// Stable id for the registry test. The button draws [`Self::icon`].
    #[allow(dead_code)]
    pub id: &'static str,
    /// Window name. Included in [`Self::tooltip`].
    #[allow(dead_code)]
    pub label: &'static str,
    pub icon: &'static str,
    /// Bind shown in the tooltip, when the window has one.
    #[allow(dead_code)]
    pub hotkey: Option<&'static str>,
    pub tooltip: &'static str,
    pub class: WindowClass,
    kind: MenuBarKind,
}

impl MenuBarEntry {
    fn toggle(
        id: &'static str,
        label: &'static str,
        icon: &'static str,
        hotkey: Option<&'static str>,
        tooltip: &'static str,
        class: WindowClass,
        event: InputEvent,
    ) -> Self {
        Self {
            id,
            label,
            icon,
            hotkey,
            tooltip,
            class,
            kind: MenuBarKind::Toggle(Box::new(event)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn open_close(
        id: &'static str,
        label: &'static str,
        icon: &'static str,
        hotkey: Option<&'static str>,
        tooltip: &'static str,
        class: WindowClass,
        open: InputEvent,
        close: InputEvent,
    ) -> Self {
        Self {
            id,
            label,
            icon,
            hotkey,
            tooltip,
            class,
            kind: MenuBarKind::OpenClose {
                open: Box::new(open),
                close: Box::new(close),
            },
        }
    }

    fn open_or_dismiss(
        id: &'static str,
        label: &'static str,
        icon: &'static str,
        hotkey: Option<&'static str>,
        tooltip: &'static str,
        class: WindowClass,
        open: InputEvent,
    ) -> Self {
        Self {
            id,
            label,
            icon,
            hotkey,
            tooltip,
            class,
            kind: MenuBarKind::OpenOrDismiss(Box::new(open)),
        }
    }

    /// True when `class` is one of the windows currently open.
    #[cfg(test)]
    pub fn is_open(&self, open_classes: &[WindowClass]) -> bool {
        open_classes.contains(&self.class)
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
        CloseBank, CloseMail, CloseRoulette, OpenBank, OpenCompanions, OpenEden, OpenMail, OpenRoulette, OpenStable, OpenWarpra,
        ToggleAchievementsWindow, ToggleAudioSettingsWindow, ToggleCartWindow, ToggleCharacterOverviewWindow, ToggleChatWindow,
        ToggleClanWindow, ToggleEmotesWindow, ToggleEquipmentWindow, ToggleFriendListWindow, ToggleGameSettingsWindow,
        ToggleGraphicsSettingsWindow, ToggleGuildWindow, ToggleHotbarWindow, ToggleInterfaceSettingsWindow, ToggleInventoryWindow,
        ToggleItemFinderWindow, ToggleKeybindsWindow, ToggleMenuWindow, ToggleMvpMinimap, ToggleNavigationWindow, ToggleQuestJournalWindow,
        ToggleSkillTreeWindow, ToggleStatsWindow, ToggleStatusEffectsWindow, ToggleStatusPanelWindow, ToggleSwitchEquipWindow,
        ToggleWorldMap,
    };
    use WindowClass::{
        Achievements, AudioSettings, Bank, Cart, CharacterOverview, Chat, Clan, Companions, Eden, Emotes, Equipment, FriendList,
        GameSettings, GraphicsSettings, Guild, Hotbar, InterfaceSettings, Inventory, ItemFinder, Keybinds, Mail, Menu, MvpMinimap,
        Navigation, QuestJournal, Roulette, SkillTree, Stable, Stats, StatusEffects, StatusPanel, SwitchEquip, Warpra, WorldMap,
    };

    vec![
        MenuBarEntry::toggle(
            "status",
            "Status",
            "Status",
            Some("Alt+A"),
            "Status (Alt+A)",
            Stats,
            ToggleStatsWindow,
        ),
        MenuBarEntry::toggle(
            "gear",
            "Gear",
            "Gear",
            Some("Alt+Q"),
            "Gear (Alt+Q)",
            Equipment,
            ToggleEquipmentWindow,
        ),
        MenuBarEntry::toggle(
            "inventory",
            "Inventory",
            "Items",
            Some("Alt+E"),
            "Inventory (Alt+E)",
            Inventory,
            ToggleInventoryWindow,
        ),
        MenuBarEntry::toggle(
            "skills",
            "Skills",
            "Skills",
            Some("Alt+S"),
            "Skills (Alt+S)",
            SkillTree,
            ToggleSkillTreeWindow,
        ),
        MenuBarEntry::toggle(
            "quests",
            "Quests",
            "Quests",
            None,
            "Quests",
            QuestJournal,
            ToggleQuestJournalWindow,
        ),
        MenuBarEntry::toggle(
            "friends",
            "Friends",
            "Friends",
            Some("Alt+Z"),
            "Friends (Alt+Z)",
            FriendList,
            ToggleFriendListWindow,
        ),
        MenuBarEntry::toggle(
            "guild",
            "Guild",
            "Guild",
            Some("Alt+G"),
            "Guild (Alt+G)",
            Guild,
            ToggleGuildWindow,
        ),
        MenuBarEntry::toggle("clan", "Clan", "Clan", Some("Alt+C"), "Clan (Alt+C)", Clan, ToggleClanWindow),
        MenuBarEntry::open_close("mail", "Mail", "Mail", None, "Mail", Mail, OpenMail, CloseMail),
        MenuBarEntry::open_close("bank", "Bank", "Bank", None, "Bank", Bank, OpenBank, CloseBank),
        MenuBarEntry::toggle(
            "achievements",
            "Achievements",
            "Achv",
            None,
            "Achievements",
            Achievements,
            ToggleAchievementsWindow,
        ),
        MenuBarEntry::toggle("map", "Map", "Map", None, "Map", MvpMinimap, ToggleMvpMinimap),
        MenuBarEntry::toggle(
            "world",
            "World Map",
            "World",
            Some("Alt+0"),
            "World Map (Alt+0)",
            WorldMap,
            ToggleWorldMap,
        ),
        MenuBarEntry::toggle("chat", "Chat", "Chat", None, "Chat", Chat, ToggleChatWindow),
        MenuBarEntry::toggle(
            "options",
            "Options",
            "Options",
            Some("Ctrl+I"),
            "Options (Ctrl+I)",
            InterfaceSettings,
            ToggleInterfaceSettingsWindow,
        ),
        MenuBarEntry::toggle("hotkeys", "Hotkeys", "Keys", None, "Hotkeys", Keybinds, ToggleKeybindsWindow),
        MenuBarEntry::toggle(
            "emotes",
            "Emotes",
            "Emotes",
            Some("Alt+1-9"),
            "Emotes (Alt+1-9)",
            Emotes,
            ToggleEmotesWindow,
        ),
        MenuBarEntry::toggle("menu", "Menu", "Menu", None, "Menu", Menu, ToggleMenuWindow),
        MenuBarEntry::toggle("cart", "Cart", "Cart", None, "Cart", Cart, ToggleCartWindow),
        MenuBarEntry::toggle("hotbar", "Hotbar", "Bar", None, "Hotbar", Hotbar, ToggleHotbarWindow),
        MenuBarEntry::toggle(
            "navigation",
            "Navigation",
            "Nav",
            None,
            "Navigation",
            Navigation,
            ToggleNavigationWindow,
        ),
        MenuBarEntry::toggle(
            "finder",
            "Item Finder",
            "Find",
            Some("Alt+F"),
            "Item Finder (Alt+F)",
            ItemFinder,
            ToggleItemFinderWindow,
        ),
        MenuBarEntry::open_or_dismiss("stable", "Stable", "Stable", None, "Stable", Stable, OpenStable),
        MenuBarEntry::open_or_dismiss("eden", "Eden", "Eden", None, "Eden", Eden, OpenEden),
        MenuBarEntry::open_or_dismiss("warpra", "Warpra", "Warp", None, "Warpra", Warpra, OpenWarpra),
        MenuBarEntry::open_or_dismiss(
            "companions",
            "Companions",
            "Pets",
            None,
            "Companions",
            Companions,
            OpenCompanions,
        ),
        MenuBarEntry::toggle(
            "switch",
            "Switch gear",
            "Sets",
            None,
            "Switch gear",
            SwitchEquip,
            ToggleSwitchEquipWindow,
        ),
        MenuBarEntry::toggle(
            "audio",
            "Audio",
            "Audio",
            Some("Ctrl+A"),
            "Audio (Ctrl+A)",
            AudioSettings,
            ToggleAudioSettingsWindow,
        ),
        MenuBarEntry::toggle(
            "graphics",
            "Graphics",
            "Video",
            Some("Ctrl+G"),
            "Graphics (Ctrl+G)",
            GraphicsSettings,
            ToggleGraphicsSettingsWindow,
        ),
        MenuBarEntry::toggle(
            "game",
            "Game",
            "Game",
            Some("Ctrl+S"),
            "Game (Ctrl+S)",
            GameSettings,
            ToggleGameSettingsWindow,
        ),
        MenuBarEntry::open_close(
            "roulette",
            "Roulette",
            "Wheel",
            None,
            "Roulette",
            Roulette,
            OpenRoulette,
            CloseRoulette,
        ),
        MenuBarEntry::toggle(
            "buffs",
            "Status effects",
            "Buffs",
            None,
            "Status effects",
            StatusEffects,
            ToggleStatusEffectsWindow,
        ),
        MenuBarEntry::toggle(
            "info",
            "Basic info",
            "Info",
            None,
            "Basic info",
            StatusPanel,
            ToggleStatusPanelWindow,
        ),
        MenuBarEntry::toggle(
            "character",
            "Character",
            "Char",
            None,
            "Character",
            CharacterOverview,
            ToggleCharacterOverviewWindow,
        ),
    ]
}

/// Bit `i` is set when entry `i`'s window is open. The menu bar reads this
/// from client state while it lays out.
pub fn menu_bar_open_bits(is_open: impl Fn(WindowClass) -> bool) -> u64 {
    let mut bits = 0;
    for (index, entry) in menu_bar_entries().iter().enumerate() {
        if index < 64 && is_open(entry.class) {
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

/// A zero size is the value before the first resize, so use the client's
/// initial 1280×720 window instead of collapsing the bar.
fn finite_screen(screen_width: f32, screen_height: f32) -> (f32, f32) {
    if screen_width.is_finite() && screen_height.is_finite() && screen_width >= 1.0 && screen_height >= 1.0 {
        (screen_width, screen_height)
    } else {
        (1280.0, 720.0)
    }
}

fn menu_bar_live_screen(state: &ClientState) -> (f32, f32) {
    let size = *client_state().window_size().follow_safe(state);
    finite_screen(size.width, size.height)
}

/// Width the window sizer locks to. Both minimum and maximum use this, so the
/// preset is the rendered width instead of a constant that ignores the screen.
fn menu_bar_locked_width(state: &ClientState) -> f32 {
    let (width, height) = menu_bar_live_screen(state);
    default_menu_bar_frame(width, height).width
}

fn menu_bar_locked_height(state: &ClientState) -> f32 {
    let (width, height) = menu_bar_live_screen(state);
    default_menu_bar_frame(width, height).height
}

/// Rectangle used the first time the bar is opened.
///
/// One row, centered, with its bottom just above chat and the hotbar. The
/// width is the screen minus the side margins. The window's minimum and
/// maximum size are this rectangle, so the sizer cannot force a wider bar.
pub fn default_menu_bar_frame(screen_width: f32, screen_height: f32) -> HudFrame {
    let (screen_width, screen_height) = finite_screen(screen_width, screen_height);
    let hud = first_open_hud(screen_width, screen_height);
    let ceiling = (hud.chat.top.min(hud.hotbar.top) - MENU_BAR_HUD_CLEARANCE).min(screen_height);
    let preferred = MENU_BAR_WINDOW_HEIGHT.min(screen_height);
    let height = if ceiling >= preferred {
        preferred
    } else {
        ceiling.max(1.0).min(screen_height)
    };
    let top = (ceiling - height).max(0.0);
    let height = height.min((screen_height - top).max(1.0));

    let width = (screen_width - MENU_BAR_EDGE_MARGIN * 2.0).clamp(1.0, screen_width);
    let left = ((screen_width - width) * 0.5).max(0.0);

    HudFrame { left, top, width, height }
}

/// One button in the single row drawn inside the content box.
fn menu_bar_cell(content_left: f32, content_top: f32, content_width: f32, content_height: f32, count: usize, index: usize) -> HudFrame {
    let columns = count.max(1) as f32;
    let gaps = count.saturating_sub(1) as f32;
    let gap = if gaps == 0.0 || content_width <= columns {
        0.0
    } else {
        BUTTON_GAP.min(((content_width - columns) / gaps).max(0.0))
    };
    let cell_width = ((content_width - gap * gaps) / columns).max(0.0);
    HudFrame {
        left: content_left + (cell_width + gap) * index as f32,
        top: content_top,
        width: cell_width,
        height: content_height.max(0.0),
    }
}

/// Content box under the title, using the same insets as the in-game window
/// resolver. Tests place buttons with this so they match `lay_out`.
#[cfg(test)]
fn menu_bar_content_frame(window: HudFrame) -> HudFrame {
    HudFrame {
        left: window.left + MENU_BAR_BORDER,
        top: window.top + MENU_BAR_TITLE + MENU_BAR_GAPS + MENU_BAR_TITLE_GAP,
        width: (window.width - MENU_BAR_BORDER * 2.0).max(0.0),
        height: (window.height - MENU_BAR_TITLE - MENU_BAR_GAPS - MENU_BAR_TITLE_GAP - MENU_BAR_BORDER).max(0.0),
    }
}

#[cfg(test)]
fn menu_bar_button_frames(window: HudFrame, count: usize) -> Vec<HudFrame> {
    let content = menu_bar_content_frame(window);
    (0..count)
        .map(|index| menu_bar_cell(content.left, content.top, content.width, content.height, count, index))
        .collect()
}

/// Anchor and size passed to the window cache when the bar has never been
/// moved.
pub fn default_menu_bar_window(screen_width: f32, screen_height: f32) -> (Anchor<ClientState>, ScreenSize) {
    let frame = default_menu_bar_frame(screen_width, screen_height);
    let (left, top) = frame_anchor_offset(ScreenAnchor::BottomCenter, frame, screen_width, screen_height);
    let anchor = Anchor::at(AnchorPoint::BottomCenter, ScreenPosition { left, top });
    let size = ScreenSize {
        width: frame.width,
        height: frame.height,
    };
    (anchor, size)
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
            title: "Menu bar",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
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

struct MenuBarButtons {
    clicks: Vec<MenuBarClick>,
}

impl MenuBarButtons {
    fn new() -> Self {
        let clicks = (0..menu_bar_entries().len())
            .map(|index| MenuBarClick { index: index as u8 })
            .collect();
        Self { clicks }
    }
}

struct MenuBarLayout {
    area: Area,
}

impl Element<ClientState> for MenuBarButtons {
    type LayoutInfo = MenuBarLayout;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let available = resolver.push_available_area();
            let height = available.height.unwrap_or(MENU_BAR_BUTTON_HEIGHT).max(1.0);
            let area = resolver.with_height(height);
            MenuBarLayout { area }
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
        if entries.is_empty() {
            return;
        }

        let theme_path = client_theme();
        let open_bits_path = client_state().menu_bar_open();
        let theme = state.get(&theme_path);
        let bits = *state.get(&open_bits_path);
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

        for (index, entry) in entries.iter().enumerate() {
            let cell = menu_bar_cell(content.left, content.top, content.width, content.height, entries.len(), index);
            let area = Area {
                left: cell.left,
                top: cell.top,
                width: cell.width.max(1.0),
                height: cell.height.max(1.0),
            };
            let hovered = area.check().run(layout);
            let open = index < 64 && bits & (1_u64 << index) != 0;
            if hovered {
                layout.add_tooltip(entry.tooltip, MenuBarTooltip.tooltip_id());
                if let Some(click) = self.clicks.get(index) {
                    layout.register_click_handler(korangar_interface::layout::MouseButton::Left, click);
                }
            }

            let (background, foreground) = match (open, hovered) {
                (true, true) => (hover_background, active),
                (true, false) => (active, active_foreground),
                (false, true) => (hover_background, active),
                (false, false) => (idle_background, idle_foreground),
            };

            layout.add_rectangle(area, corner, background, shadow, shadow_padding);
            layout.add_text(
                area,
                entry.icon,
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

struct MenuBarTooltip;

#[cfg(test)]
mod tests {
    use super::{
        MENU_BAR_WINDOW_HEIGHT, MenuBarKind, MenuBarPress, default_menu_bar_frame, default_menu_bar_window, menu_bar_button_frames,
        menu_bar_entries, menu_bar_event_window, menu_bar_open_bits, press_menu_bar,
    };
    use crate::graphics::ScreenPosition;
    use crate::interface::windows::WindowClass;
    use crate::interface::windows::placement::{HudFrame, first_open_hud};

    #[test]
    fn every_entry_toggles_its_window_and_reports_open() {
        let entries = menu_bar_entries();
        assert!(entries.len() > 8);
        assert!(entries.len() <= 64);

        let mut ids = std::collections::HashSet::new();
        let mut classes = std::collections::HashSet::new();
        for (index, entry) in entries.iter().enumerate() {
            assert!(ids.insert(entry.id), "duplicate id {}", entry.id);
            assert!(classes.insert(entry.class), "duplicate class {:?}", entry.class);
            assert_ne!(entry.class, WindowClass::CashShop);
            assert!(!entry.icon.is_empty());
            assert!(entry.tooltip.contains(entry.label));
            if let Some(hotkey) = entry.hotkey {
                assert!(entry.tooltip.contains(hotkey), "{} tooltip missing {hotkey}", entry.id);
            }

            assert!(!entry.is_open(&[]));
            assert!(entry.is_open(&[entry.class]));
            assert!(!entry.is_open(&[WindowClass::Login]));

            let closed = press_menu_bar(index, |_| false);
            let opened = press_menu_bar(index, |class| class == entry.class);
            match (&entry.kind, closed, opened) {
                (MenuBarKind::Toggle(event), Some(MenuBarPress::Dispatch(closed)), Some(MenuBarPress::Dispatch(opened))) => {
                    assert_eq!(
                        std::mem::discriminant(event.as_ref()),
                        std::mem::discriminant(closed.as_ref()),
                        "{}",
                        entry.id
                    );
                    assert_eq!(
                        std::mem::discriminant(event.as_ref()),
                        std::mem::discriminant(opened.as_ref()),
                        "{}",
                        entry.id
                    );
                    assert_eq!(menu_bar_event_window(closed.as_ref()), Some(entry.class), "{}", entry.id);
                }
                (MenuBarKind::OpenClose { open, close }, Some(MenuBarPress::Dispatch(closed)), Some(MenuBarPress::Dispatch(opened))) => {
                    assert_eq!(
                        std::mem::discriminant(open.as_ref()),
                        std::mem::discriminant(closed.as_ref()),
                        "{} closed press must be the open event",
                        entry.id
                    );
                    assert_eq!(
                        std::mem::discriminant(close.as_ref()),
                        std::mem::discriminant(opened.as_ref()),
                        "{} open press must be the close event",
                        entry.id
                    );
                    assert_ne!(
                        std::mem::discriminant(open.as_ref()),
                        std::mem::discriminant(close.as_ref()),
                        "{} open and close events are the same",
                        entry.id
                    );
                }
                (MenuBarKind::OpenOrDismiss(open), Some(MenuBarPress::Dispatch(closed)), Some(MenuBarPress::Close(class))) => {
                    assert_eq!(
                        std::mem::discriminant(open.as_ref()),
                        std::mem::discriminant(closed.as_ref()),
                        "{}",
                        entry.id
                    );
                    assert_eq!(class, entry.class);
                    assert_eq!(menu_bar_event_window(closed.as_ref()), Some(entry.class), "{}", entry.id);
                }
                other => panic!("{} press mismatch: {other:?}", entry.id),
            }
        }

        assert!(entries.iter().any(|entry| matches!(entry.kind, MenuBarKind::OpenClose { .. })));

        assert!(entries.iter().any(|entry| entry.id == "status"));
        assert!(entries.iter().any(|entry| entry.id == "emotes"));
        assert!(entries.iter().any(|entry| entry.id == "hotkeys"));
        assert!(entries.iter().all(|entry| entry.id != "cash_shop"));
        assert!(press_menu_bar(entries.len(), |_| false).is_none());
    }

    #[test]
    fn open_bits_follow_the_registry_order() {
        let entries = menu_bar_entries();
        let wanted = [WindowClass::Inventory, WindowClass::Stats, WindowClass::Chat];
        let bits = menu_bar_open_bits(|class| wanted.contains(&class));
        for (index, entry) in entries.iter().enumerate() {
            let on = bits & (1_u64 << index) != 0;
            assert_eq!(on, entry.is_open(&wanted));
        }
        assert_eq!(menu_bar_open_bits(|_| false), 0);
    }

    #[test]
    fn menu_bar_starts_docked_on_the_bottom_and_remembers_a_drag() {
        let screen_width = 1280.0;
        let screen_height = 720.0;
        let frame = default_menu_bar_frame(screen_width, screen_height);
        let hud = first_open_hud(screen_width, screen_height);
        let ceiling = hud.chat.top.min(hud.hotbar.top) - 4.0;
        assert!(
            (frame.top + frame.height - ceiling).abs() < 0.5,
            "bar bottom {} is not just above chat and the hotbar ({ceiling})",
            frame.top + frame.height
        );
        assert!(
            screen_height - (frame.top + frame.height) > 40.0,
            "bar still covers the bottom HUD"
        );
        assert!((frame.left + frame.width / 2.0 - screen_width / 2.0).abs() < 0.5);
        assert!((frame.height - MENU_BAR_WINDOW_HEIGHT).abs() < 0.5);

        let screen = crate::graphics::ScreenSize {
            width: screen_width,
            height: screen_height,
        };
        let (mut anchor, size) = default_menu_bar_window(screen_width, screen_height);
        let start = anchor.to_position(screen);
        assert!((start.left - frame.left).abs() < 0.5);
        assert!((start.top - frame.top).abs() < 0.5);

        let dragged = ScreenPosition { left: 120.0, top: 80.0 };
        anchor.update(screen, dragged, size);
        let remembered = anchor.to_position(screen);
        assert!((remembered.left - dragged.left).abs() < 0.5);
        assert!((remembered.top - dragged.top).abs() < 0.5);

        let again = anchor.to_position(screen);
        assert_eq!(again.left, remembered.left);
        assert_eq!(again.top, remembered.top);
    }

    fn rects_overlap(a: HudFrame, b: HudFrame) -> bool {
        a.left < b.right() && a.right() > b.left && a.top < b.bottom() && a.bottom() > b.top
    }

    #[test]
    fn menu_bar_buttons_fit_the_screen_and_clear_chat_and_hotbar() {
        // 1280×720 is the client layout. The harness shot is 700px tall, so a
        // button past y=700 is sliced off even though it is inside that layout.
        let cases = [(1280.0, 720.0, 700.0), (1279.0, 700.0, 700.0), (1024.0, 576.0, 576.0)];
        let count = menu_bar_entries().len();

        for (screen_width, screen_height, visible_bottom) in cases {
            let frame = default_menu_bar_frame(screen_width, screen_height);
            let hud = first_open_hud(screen_width, screen_height);
            assert!(
                (frame.width - (screen_width - 16.0)).abs() < 0.5,
                "{screen_width}x{screen_height} width {} is not the preset",
                frame.width
            );
            assert!(frame.left >= -0.5 && frame.top >= -0.5);
            assert!(frame.right() <= screen_width + 0.5);
            assert!(frame.bottom() <= visible_bottom + 0.5, "bar hangs off the visible window");
            assert!(
                !rects_overlap(frame, hud.chat),
                "bar covers chat at {screen_width}x{screen_height}"
            );
            assert!(
                !rects_overlap(frame, hud.hotbar),
                "bar covers hotbar at {screen_width}x{screen_height}"
            );

            let buttons = menu_bar_button_frames(frame, count);
            assert_eq!(buttons.len(), count);
            let row_top = buttons[0].top;
            let mut previous_right = frame.left;
            for button in &buttons {
                assert!((button.top - row_top).abs() < 0.5, "buttons are not one row");
                assert!(button.width > 1.0, "button has no width");
                assert!(button.left >= -0.5 && button.top >= -0.5);
                assert!(button.right() <= screen_width + 0.5);
                assert!(button.bottom() <= visible_bottom + 0.5);
                assert!(button.left + 0.5 >= frame.left && button.right() <= frame.right() + 0.5);
                assert!(button.bottom() <= frame.bottom() + 0.5);
                assert!(button.left + 0.5 >= previous_right, "buttons overlap each other");
                assert!(!rects_overlap(*button, hud.chat));
                assert!(!rects_overlap(*button, hud.hotbar));
                previous_right = button.right();
            }
            assert!(buttons.last().unwrap().right() <= screen_width + 0.5);
        }
    }
}
