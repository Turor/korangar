use korangar_interface::window::{CustomWindow, Window};
use rust_state::Path;

use super::basic_info_view::{ExpPercentLabel, ExpTotalLabel, WeightColor, WeightLabel, ZenyLabel};
use crate::graphics::Color;
use crate::input::InputEvent;
use crate::interface::components::exp_bar::ExpBar;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};
use crate::world::{BasicInfoStatsPathExt, Player, PlayerPathExt};

pub struct CharacterOverviewWindow<A, P> {
    player_name_path: A,
    player_path: P,
}

impl<A, P> CharacterOverviewWindow<A, P> {
    pub fn new(player_name_path: A, player_path: P) -> Self {
        Self {
            player_name_path,
            player_path,
        }
    }
}

impl<A, P> CustomWindow<ClientState> for CharacterOverviewWindow<A, P>
where
    A: Path<ClientState, String>,
    P: Path<ClientState, Player>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::CharacterOverview)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let player = self.player_path;
        let info = player.basic_info();

        window! {
            title: client_state().localization().character_overview_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: 300.0,
            maximum_width: 300.0,
            elements: (
                fragment! {
                    gaps: 4.0,
                    children: (
                        split! {
                            children: (
                                text! {
                                    text: client_state().localization().name_text(),
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                text! {
                                    text: self.player_name_path,
                                    color: Color::rgb_u8(255, 144, 13),
                                    horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                            ),
                        },
                        split! {
                            children: (
                                text! {
                                    text: client_state().localization().base_level_text(),
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                text! {
                                    text: PartialEqDisplaySelector::new(player.base_level()),
                                    color: Color::rgb_u8(13, 231, 255),
                                    horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                            ),
                        },
                        split! {
                            children: (
                                text! {
                                    text: client_state().localization().job_level_text(),
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                text! {
                                    text: PartialEqDisplaySelector::new(player.job_level()),
                                    color: Color::rgb_u8(13, 231, 255),
                                    horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                            ),
                        },
                    ),
                },
                text! {
                    text: ExpPercentLabel::new("Base EXP", info.base_experience(), info.next_base_experience()),
                    color: client_theme().basic_info().base_exp_color(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                ExpBar::base(info.base_experience(), info.next_base_experience()),
                text! {
                    text: ExpTotalLabel::new(info.base_experience(), info.next_base_experience()),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: ExpPercentLabel::new("Job EXP", info.job_experience(), info.next_job_experience()),
                    color: client_theme().basic_info().job_exp_color(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                ExpBar::job(info.job_experience(), info.next_job_experience()),
                text! {
                    text: ExpTotalLabel::new(info.job_experience(), info.next_job_experience()),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: WeightLabel::new(info.weight(), info.maximum_weight()),
                    color: WeightColor::new(info.weight(), info.maximum_weight()),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: ZenyLabel::new(info.zeny()),
                    color: client_theme().basic_info().zeny_color(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: client_state().localization().inventory_button_text(),
                    event: InputEvent::ToggleInventoryWindow,
                },
                button! {
                    text: client_state().localization().cart_button_text(),
                    event: InputEvent::ToggleCartWindow,
                },
                button! {
                    text: client_state().localization().equipment_button_text(),
                    event: InputEvent::ToggleEquipmentWindow,
                },
                button! {
                    text: client_state().localization().skill_tree_button_text(),
                    event: InputEvent::ToggleSkillTreeWindow,
                },
                button! {
                    text: client_state().localization().stats_button_text(),
                    event: InputEvent::ToggleStatsWindow,
                },
                button! {
                    text: client_state().localization().friend_list_button_text(),
                    event: InputEvent::ToggleFriendListWindow,
                },
                button! {
                    text: "Guild",
                    event: InputEvent::ToggleGuildWindow,
                },
                button! {
                    text: "Clan",
                    event: InputEvent::ToggleClanWindow,
                },
                button! {
                    text: "Quests",
                    event: InputEvent::ToggleQuestJournalWindow,
                },
                button! {
                    text: "Achievements",
                    event: InputEvent::ToggleAchievementsWindow,
                },
                button! {
                    text: "Navigation",
                    event: InputEvent::ToggleNavigationWindow,
                },
                button! {
                    text: "Status",
                    event: InputEvent::ToggleStatusEffectsWindow,
                },
                button! {
                    text: client_state().localization().menu_button_text(),
                    event: InputEvent::ToggleMenuWindow,
                },
            ),
        }
    }
}
