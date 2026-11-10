//! Small server-driven windows: Sense, memorial dungeon, battleground score,
//! reputation, and the item pickup popup.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use korangar_interface::element::StateElement;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{BattlefieldUpdate, MemorialDungeonUpdate, format_remaining};
use rust_state::RustState;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// How long the pickup popup stays up.
pub const ITEM_OBTAIN_DURATION: Duration = Duration::from_secs(3);

/// Memorial dungeon as the server last described it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemorialDungeonView {
    pub name: String,
    /// Queue position while the dungeon waits to be created.
    pub priority: Option<u16>,
    pub destroy_date: u32,
    pub enter_time_out_date: u32,
    pub created: bool,
}

impl MemorialDungeonView {
    pub fn apply(&mut self, update: &MemorialDungeonUpdate) {
        match update {
            MemorialDungeonUpdate::Subscription { name, priority } => {
                *self = Self {
                    name: name.clone(),
                    priority: Some(*priority),
                    ..Self::default()
                };
            }
            MemorialDungeonUpdate::Priority { priority } => self.priority = Some(*priority),
            MemorialDungeonUpdate::Info {
                name,
                destroy_date,
                enter_time_out_date,
            } => {
                *self = Self {
                    name: name.clone(),
                    priority: None,
                    destroy_date: *destroy_date,
                    enter_time_out_date: *enter_time_out_date,
                    created: true,
                };
            }
            MemorialDungeonUpdate::Notify { .. } => {}
        }
    }

    pub fn render(&self, now_unix: u64) -> String {
        let mut lines = vec![self.name.clone()];
        if !self.created {
            match self.priority {
                // Hercules sends a negative priority when the reservation is cancelled.
                Some(priority) if priority >= 0x8000 => lines.push("Reservation cancelled.".to_owned()),
                Some(priority) => lines.push(format!("Waiting to be created. Queue position: {priority}")),
                None => lines.push("Waiting to be created.".to_owned()),
            }
            return lines.join("\n");
        }
        match format_remaining(self.destroy_date, now_unix) {
            Some(left) => lines.push(format!("Time left: {left}")),
            None => lines.push("Time left: no limit".to_owned()),
        }
        if let Some(left) = format_remaining(self.enter_time_out_date, now_unix) {
            lines.push(format!("Closes if empty in: {left}"));
        }
        lines.join("\n")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BattlegroundMember {
    pub name: String,
    pub camp: Option<u16>,
    pub hp: i32,
    pub maximum_hp: i32,
    pub position: Option<(u16, u16)>,
}

/// Score and team list from the battleground packets.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BattlegroundView {
    pub camp_a: u16,
    pub camp_b: u16,
    pub has_score: bool,
    pub members: BTreeMap<u32, BattlegroundMember>,
}

impl BattlegroundView {
    /// Apply one update. Chat is handled by the caller. Returns `true` when the
    /// window should show (any non-chat update).
    pub fn apply(&mut self, update: &BattlefieldUpdate) -> bool {
        match update {
            BattlefieldUpdate::Chat { .. } => false,
            BattlefieldUpdate::Camp { account_id, name, camp } => {
                let member = self.members.entry(account_id.0).or_default();
                member.name = name.clone();
                member.camp = Some(*camp);
                true
            }
            BattlefieldUpdate::Points { camp_a, camp_b } => {
                self.camp_a = *camp_a;
                self.camp_b = *camp_b;
                self.has_score = true;
                true
            }
            BattlefieldUpdate::Position {
                account_id, name, x, y, ..
            } => {
                let member = self.members.entry(account_id.0).or_default();
                member.name = name.clone();
                member.position = Some((*x, *y));
                true
            }
            BattlefieldUpdate::Hp {
                account_id,
                name,
                hp,
                maximum_hp,
            } => {
                let member = self.members.entry(account_id.0).or_default();
                if let Some(name) = name {
                    member.name = name.clone();
                }
                member.hp = *hp;
                member.maximum_hp = *maximum_hp;
                true
            }
        }
    }

    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        if self.has_score {
            lines.push(format!("Score   Camp A {}  :  {} Camp B", self.camp_a, self.camp_b));
        } else {
            lines.push("Waiting for the score.".to_owned());
        }
        for member in self.members.values() {
            let name = if member.name.is_empty() { "?" } else { member.name.as_str() };
            let camp = match member.camp {
                Some(camp) => format!("[{camp}] "),
                None => String::new(),
            };
            let hp = if member.maximum_hp > 0 {
                format!("  HP {}/{}", member.hp, member.maximum_hp)
            } else {
                String::new()
            };
            let position = match member.position {
                Some((x, y)) => format!("  at {x}, {y}"),
                None => String::new(),
            };
            lines.push(format!("{camp}{name}{hp}{position}"));
        }
        lines.join("\n")
    }
}

#[derive(Default, RustState, StateElement)]
pub struct MiscInfoState {
    sense: String,
    memorial: String,
    battleground: String,
    reputation: String,
    item_obtain: String,
    #[hidden_element]
    memorial_view: MemorialDungeonView,
    #[hidden_element]
    battleground_view: BattlegroundView,
    #[hidden_element]
    item_obtain_until: Option<Instant>,
    #[hidden_element]
    last_refresh_unix: u64,
}

impl MiscInfoState {
    pub fn set_sense(&mut self, text: String) {
        self.sense = text;
    }

    pub fn reputation(&self) -> &str {
        &self.reputation
    }

    pub fn set_reputation(&mut self, text: String) {
        self.reputation = text;
    }

    pub fn apply_memorial(&mut self, update: &MemorialDungeonUpdate, now_unix: u64) {
        self.memorial_view.apply(update);
        self.memorial = self.memorial_view.render(now_unix);
    }

    pub fn clear_memorial(&mut self) {
        self.memorial_view = MemorialDungeonView::default();
        self.memorial.clear();
    }

    pub fn apply_battleground(&mut self, update: &BattlefieldUpdate) -> bool {
        let show = self.battleground_view.apply(update);
        if show {
            self.battleground = self.battleground_view.render();
        }
        show
    }

    pub fn clear_battleground(&mut self) {
        self.battleground_view = BattlegroundView::default();
        self.battleground.clear();
    }

    pub fn show_item_obtain(&mut self, text: String, now: Instant) {
        self.item_obtain = text;
        self.item_obtain_until = Some(now + ITEM_OBTAIN_DURATION);
    }

    /// `true` once the popup's time is up (and only once).
    pub fn item_obtain_expired(&mut self, now: Instant) -> bool {
        match self.item_obtain_until {
            Some(until) if now >= until => {
                self.item_obtain_until = None;
                true
            }
            _ => false,
        }
    }

    /// Re-render the memorial countdown once a second.
    pub fn refresh_timers(&mut self, now_unix: u64) {
        if self.memorial_view.created && now_unix != self.last_refresh_unix {
            self.last_refresh_unix = now_unix;
            self.memorial = self.memorial_view.render(now_unix);
        }
    }
}

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

macro_rules! text_window {
    ($window:ident, $class:ident, $title:literal, $field:ident, $width:literal) => {
        pub struct $window;

        impl CustomWindow<ClientState> for $window {
            fn window_class() -> Option<WindowClass> {
                Some(WindowClass::$class)
            }

            fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
                use korangar_interface::prelude::*;

                window! {
                    title: $title,
                    class: Self::window_class(),
                    theme: InterfaceThemeType::InGame,
                    minimum_width: $width,
                    maximum_width: 520.0,
                    closable: true,
                    elements: (
                        text! {
                            text: client_state().misc_info().$field(),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                    ),
                }
            }
        }
    };
}

text_window!(SenseWindow, Sense, "Monster information", sense, 300.0);
text_window!(BattlegroundWindow, Battleground, "Battleground", battleground, 320.0);
text_window!(ReputationWindow, Reputation, "Reputation", reputation, 280.0);

pub struct MemorialDungeonWindow;

impl CustomWindow<ClientState> for MemorialDungeonWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::MemorialDungeon)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Memorial dungeon",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: 300.0,
            maximum_width: 480.0,
            closable: true,
            elements: (
                text! {
                    text: client_state().misc_info().memorial(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Destroy dungeon",
                    event: InputEvent::DestroyMemorialDungeon,
                },
            ),
        }
    }
}

pub struct ItemObtainWindow;

impl CustomWindow<ClientState> for ItemObtainWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ItemObtain)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Item obtained",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: 240.0,
            maximum_width: 420.0,
            closable: true,
            elements: (
                text! {
                    text: client_state().misc_info().item_obtain(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::AccountId;

    use super::*;

    #[test]
    fn memorial_window_follows_queue_then_countdown() {
        let mut view = MemorialDungeonView::default();
        view.apply(&MemorialDungeonUpdate::Subscription {
            name: "Endless Tower".to_owned(),
            priority: 2,
        });
        assert_eq!(view.render(0), "Endless Tower\nWaiting to be created. Queue position: 2");
        view.apply(&MemorialDungeonUpdate::Priority { priority: 1 });
        assert!(view.render(0).ends_with("Queue position: 1"));
        view.apply(&MemorialDungeonUpdate::Priority { priority: 0xFFFF });
        assert!(view.render(0).ends_with("Reservation cancelled."));

        view.apply(&MemorialDungeonUpdate::Info {
            name: "Endless Tower".to_owned(),
            destroy_date: 1000 + 7200,
            enter_time_out_date: 1000 + 300,
        });
        assert_eq!(
            view.render(1000),
            "Endless Tower\nTime left: 2:00:00\nCloses if empty in: 0:05:00"
        );
        assert!(view.render(1001).contains("Time left: 1:59:59"));
    }

    #[test]
    fn memorial_without_limits_says_so() {
        let mut view = MemorialDungeonView::default();
        view.apply(&MemorialDungeonUpdate::Info {
            name: "Orc's Memory".to_owned(),
            destroy_date: 0,
            enter_time_out_date: 0,
        });
        assert_eq!(view.render(5), "Orc's Memory\nTime left: no limit");
    }

    #[test]
    fn battleground_tracks_score_camps_hp_and_positions() {
        let mut view = BattlegroundView::default();
        assert!(!view.apply(&BattlefieldUpdate::Chat {
            account_id: AccountId(1),
            name: "a".to_owned(),
            message: "hi".to_owned(),
        }));
        assert!(view.apply(&BattlefieldUpdate::Points { camp_a: 3, camp_b: 1 }));
        view.apply(&BattlefieldUpdate::Camp {
            account_id: AccountId(7),
            name: "Guard".to_owned(),
            camp: 1,
        });
        view.apply(&BattlefieldUpdate::Hp {
            account_id: AccountId(7),
            name: None,
            hp: 900,
            maximum_hp: 1200,
        });
        view.apply(&BattlefieldUpdate::Position {
            account_id: AccountId(7),
            name: "Guard".to_owned(),
            class: 7,
            x: 50,
            y: 60,
        });
        assert_eq!(
            view.render(),
            "Score   Camp A 3  :  1 Camp B\n[1] Guard  HP 900/1200  at 50, 60"
        );
    }

    #[test]
    fn item_popup_expires_once() {
        let mut state = MiscInfoState::default();
        let now = Instant::now();
        state.show_item_obtain("You got Red Potion (1).".to_owned(), now);
        assert!(!state.item_obtain_expired(now));
        assert!(state.item_obtain_expired(now + ITEM_OBTAIN_DURATION));
        assert!(!state.item_obtain_expired(now + ITEM_OBTAIN_DURATION * 2));
    }
}
