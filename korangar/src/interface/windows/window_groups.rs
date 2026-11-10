//! Tabbed window groups.
//!
//! Related windows share one menu-bar button and a tab strip at the top of
//! each member window ([`WindowGroupTabs`]). A tab click closes the current
//! member and opens the chosen one at the same anchor, so the group reads as
//! one window with tabs. Every member keeps its own class, hotkey and saved
//! size; a hotkey opens that member directly, with the strip showing.

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::alignment::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use rust_state::State;

use super::WindowClass;
use crate::input::InputEvent;
use crate::loaders::OverflowBehavior;
use crate::state::{ClientState, client_theme};

/// Height of the tab strip.
pub const GROUP_TAB_HEIGHT: f32 = 22.0;
const GROUP_TAB_GAP: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowGroup {
    Settings,
    Character,
    Social,
    Journal,
    PetStable,
}

/// One tab: its label and the window it shows.
#[derive(Debug, Clone, Copy)]
pub struct GroupTab {
    pub label: &'static str,
    pub class: WindowClass,
    /// The window's own hotkey, shown in the menu-bar tooltip.
    pub hotkey: Option<&'static str>,
}

const fn tab(label: &'static str, class: WindowClass) -> GroupTab {
    GroupTab {
        label,
        class,
        hotkey: None,
    }
}

const fn keyed(label: &'static str, class: WindowClass, hotkey: &'static str) -> GroupTab {
    GroupTab {
        label,
        class,
        hotkey: Some(hotkey),
    }
}

const SETTINGS_TABS: [GroupTab; 5] = [
    keyed("Interface", WindowClass::InterfaceSettings, "Ctrl+I"),
    keyed("Graphics", WindowClass::GraphicsSettings, "Ctrl+G"),
    keyed("Audio", WindowClass::AudioSettings, "Ctrl+A"),
    keyed("Game", WindowClass::GameSettings, "Ctrl+S"),
    tab("Keys", WindowClass::Keybinds),
];
const CHARACTER_TABS: [GroupTab; 5] = [
    keyed("Stats", WindowClass::Stats, "Alt+A"),
    keyed("Gear", WindowClass::Equipment, "Alt+Q"),
    tab("Sets", WindowClass::SwitchEquip),
    tab("Buffs", WindowClass::StatusEffects),
    tab("Profile", WindowClass::CharacterOverview),
];
const SOCIAL_TABS: [GroupTab; 3] = [
    keyed("Party", WindowClass::FriendList, "Alt+Z"),
    keyed("Guild", WindowClass::Guild, "Alt+G"),
    keyed("Clan", WindowClass::Clan, "Alt+C"),
];
const JOURNAL_TABS: [GroupTab; 2] = [
    tab("Quests", WindowClass::QuestJournal),
    tab("Achievements", WindowClass::Achievements),
];
const PET_STABLE_TABS: [GroupTab; 2] = [tab("Stable", WindowClass::Stable), tab("Pets", WindowClass::Companions)];

impl WindowGroup {
    pub const ALL: [WindowGroup; 5] = [Self::Settings, Self::Character, Self::Social, Self::Journal, Self::PetStable];

    pub fn name(self) -> &'static str {
        match self {
            Self::Settings => "Settings",
            Self::Character => "Character",
            Self::Social => "Social",
            Self::Journal => "Journal",
            Self::PetStable => "Pet Stable",
        }
    }

    pub fn tabs(self) -> &'static [GroupTab] {
        match self {
            Self::Settings => &SETTINGS_TABS,
            Self::Character => &CHARACTER_TABS,
            Self::Social => &SOCIAL_TABS,
            Self::Journal => &JOURNAL_TABS,
            Self::PetStable => &PET_STABLE_TABS,
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|group| *group == self).unwrap_or(0)
    }

    /// Group a window belongs to.
    pub fn of(class: WindowClass) -> Option<WindowGroup> {
        Self::ALL
            .into_iter()
            .find(|group| group.tabs().iter().any(|tab| tab.class == class))
    }

    pub fn contains(self, class: WindowClass) -> bool {
        self.tabs().iter().any(|tab| tab.class == class)
    }

    /// Open members, in tab order.
    pub fn open_members(self, is_open: impl Fn(WindowClass) -> bool) -> Vec<WindowClass> {
        self.tabs().iter().map(|tab| tab.class).filter(|class| is_open(*class)).collect()
    }
}

/// Event that opens a group member. Only sent while that window is closed, so
/// toggle events open it.
pub fn group_member_open_event(class: WindowClass) -> Option<InputEvent> {
    Some(match class {
        WindowClass::InterfaceSettings => InputEvent::ToggleInterfaceSettingsWindow,
        WindowClass::GraphicsSettings => InputEvent::ToggleGraphicsSettingsWindow,
        WindowClass::AudioSettings => InputEvent::ToggleAudioSettingsWindow,
        WindowClass::GameSettings => InputEvent::ToggleGameSettingsWindow,
        WindowClass::Keybinds => InputEvent::ToggleKeybindsWindow,
        WindowClass::Stats => InputEvent::ToggleStatsWindow,
        WindowClass::Equipment => InputEvent::ToggleEquipmentWindow,
        WindowClass::SwitchEquip => InputEvent::ToggleSwitchEquipWindow,
        WindowClass::StatusEffects => InputEvent::ToggleStatusEffectsWindow,
        WindowClass::CharacterOverview => InputEvent::ToggleCharacterOverviewWindow,
        WindowClass::FriendList => InputEvent::ToggleFriendListWindow,
        WindowClass::Guild => InputEvent::ToggleGuildWindow,
        WindowClass::Clan => InputEvent::ToggleClanWindow,
        WindowClass::QuestJournal => InputEvent::ToggleQuestJournalWindow,
        WindowClass::Achievements => InputEvent::ToggleAchievementsWindow,
        WindowClass::Stable => InputEvent::OpenStable,
        WindowClass::Companions => InputEvent::OpenCompanions,
        _ => return None,
    })
}

/// Last member shown per group. The menu-bar button reopens it.
#[derive(Debug, Default)]
pub struct WindowGroupMemory {
    last: [Option<WindowClass>; WindowGroup::ALL.len()],
}

impl WindowGroupMemory {
    /// Remember the open member of every group. Called once a frame.
    pub fn observe(&mut self, is_open: impl Fn(WindowClass) -> bool) {
        for group in WindowGroup::ALL {
            if let Some(class) = group.open_members(&is_open).last() {
                self.last[group.index()] = Some(*class);
            }
        }
    }

    pub fn remember(&mut self, class: WindowClass) {
        if let Some(group) = WindowGroup::of(class) {
            self.last[group.index()] = Some(class);
        }
    }

    /// Member the group button opens: the last one shown, else the first tab.
    pub fn member_to_open(&self, group: WindowGroup) -> WindowClass {
        self.last[group.index()].unwrap_or(group.tabs()[0].class)
    }
}

struct SwitchTabClick {
    from: WindowClass,
    to: WindowClass,
}

impl ClickHandler<ClientState> for SwitchTabClick {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        if self.from != self.to {
            queue.queue(InputEvent::SwitchWindowTab {
                from: self.from,
                to: self.to,
            });
        }
    }
}

/// Tab strip for the group `class` belongs to. Put it first in the member
/// window's elements. Draws nothing for a class without a group.
pub struct WindowGroupTabs {
    class: WindowClass,
    tabs: &'static [GroupTab],
    clicks: Vec<SwitchTabClick>,
}

impl WindowGroupTabs {
    pub fn new(class: WindowClass) -> Self {
        let tabs = WindowGroup::of(class).map_or(&[][..], WindowGroup::tabs);
        let clicks = tabs
            .iter()
            .map(|tab| SwitchTabClick {
                from: class,
                to: tab.class,
            })
            .collect();
        Self { class, tabs, clicks }
    }
}

/// Tab `index` of `count` in a strip `width` wide.
pub fn group_tab_area(strip: Area, count: usize, index: usize) -> Area {
    let count = count.max(1) as f32;
    let width = ((strip.width - GROUP_TAB_GAP * (count - 1.0)) / count).max(1.0);
    Area {
        left: strip.left + (width + GROUP_TAB_GAP) * index as f32,
        top: strip.top,
        width,
        height: strip.height,
    }
}

pub struct WindowGroupTabsLayoutInfo {
    area: Area,
}

impl Element<ClientState> for WindowGroupTabs {
    type LayoutInfo = WindowGroupTabsLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let height = if self.tabs.is_empty() { 0.0 } else { GROUP_TAB_HEIGHT };
        with_single_resolver(resolvers, |resolver| WindowGroupTabsLayoutInfo {
            area: resolver.with_height(height),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        if self.tabs.is_empty() {
            return;
        }

        let theme_path = client_theme();
        let theme = state.get(&theme_path);
        let corner = theme.button.corner_diameter;
        let shadow = theme.button.shadow_color;
        let shadow_padding = theme.button.shadow_padding;
        let font_size = theme.button.font_size;
        let highlight = theme.button.highlight_color;

        for (index, tab) in self.tabs.iter().enumerate() {
            let area = group_tab_area(layout_info.area, self.tabs.len(), index);
            let active = tab.class == self.class;
            let hovered = !active && area.check().run(layout);

            if hovered && let Some(click) = self.clicks.get(index) {
                layout.register_click_handler(korangar_interface::layout::MouseButton::Left, click);
            }

            let (background, foreground) = if active {
                (theme.tokens.gold, theme.tokens.bg)
            } else if hovered {
                (theme.button.hovered_background_color, theme.tokens.gold)
            } else {
                (theme.button.background_color, theme.button.foreground_color)
            };
            layout.add_rectangle(area, corner, background, shadow, shadow_padding);
            layout.add_text(
                area,
                tab.label,
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

#[cfg(test)]
mod tests {
    use korangar_interface::layout::area::Area;

    use super::{WindowGroup, WindowGroupMemory, group_member_open_event, group_tab_area};
    use crate::interface::windows::WindowClass;

    #[test]
    fn every_member_belongs_to_one_group_and_has_an_open_event() {
        let mut seen = std::collections::HashSet::new();
        for group in WindowGroup::ALL {
            assert!(group.tabs().len() >= 2, "{} needs two tabs", group.name());
            for tab in group.tabs() {
                assert!(seen.insert(tab.class), "{:?} is in two groups", tab.class);
                assert_eq!(WindowGroup::of(tab.class), Some(group));
                assert!(group_member_open_event(tab.class).is_some(), "{:?}", tab.class);
                assert!(!tab.label.is_empty());
            }
        }
        assert_eq!(WindowGroup::of(WindowClass::Inventory), None);
    }

    #[test]
    fn settings_and_pet_stable_hold_the_requested_windows() {
        for class in [
            WindowClass::InterfaceSettings,
            WindowClass::GraphicsSettings,
            WindowClass::AudioSettings,
            WindowClass::GameSettings,
            WindowClass::Keybinds,
        ] {
            assert!(WindowGroup::Settings.contains(class), "{class:?}");
        }
        assert!(WindowGroup::PetStable.contains(WindowClass::Stable));
        assert!(WindowGroup::PetStable.contains(WindowClass::Companions));
    }

    #[test]
    fn memory_reopens_the_last_member() {
        let mut memory = WindowGroupMemory::default();
        assert_eq!(memory.member_to_open(WindowGroup::Social), WindowClass::FriendList);
        memory.observe(|class| class == WindowClass::Guild);
        assert_eq!(memory.member_to_open(WindowGroup::Social), WindowClass::Guild);
        memory.observe(|_| false);
        assert_eq!(memory.member_to_open(WindowGroup::Social), WindowClass::Guild);
        memory.remember(WindowClass::Clan);
        assert_eq!(memory.member_to_open(WindowGroup::Social), WindowClass::Clan);
        assert_eq!(memory.member_to_open(WindowGroup::Settings), WindowClass::InterfaceSettings);
    }

    #[test]
    fn tabs_split_the_strip_without_overlap() {
        let strip = Area {
            left: 10.0,
            top: 5.0,
            width: 300.0,
            height: 22.0,
        };
        let areas: Vec<Area> = (0..5).map(|index| group_tab_area(strip, 5, index)).collect();
        for pair in areas.windows(2) {
            assert!(pair[0].left + pair[0].width <= pair[1].left);
        }
        let last = areas.last().unwrap();
        assert!((last.left + last.width - (strip.left + strip.width)).abs() < 0.01);
    }
}
