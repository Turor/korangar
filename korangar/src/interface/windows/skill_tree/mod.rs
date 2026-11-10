mod slot;
mod state;
mod tabs;

use hashbrown::HashMap;
use korangar_interface::element::StateElement;
use korangar_interface::window::{CustomWindow, Window};
use ragnarok_packets::{SkillId, SkillLevel};
use rust_state::{ManuallyAssertExt, Path, PathExt, RustState, State, VecIndexExt};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::interface::windows::skill_tree::state::AvailablePointsDisplay;
use crate::interface::windows::skill_tree::tabs::{DropSkillWrapper, DynamicTabs, SkillTreeTab, TabSelector};
use crate::state::localization::LocalizationPathExt;
use crate::state::skills::{LearnedSkill, SkillTabLayoutPathExt, SkillTreeLayout, SkillTreeLayoutPathExt};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Internal state of the skill tree window.
#[derive(Default, RustState, StateElement)]
pub struct SkillTreeWindowState {
    currently_skilling: bool,
    /// List of pending skill points. Each new point gets appended to this list,
    /// meaning the same skill id can be in the list multiple times. The
    /// total count of each skill id defines how many points to raise the
    /// skill level by.
    pending_skill_points: Vec<SkillId>,
    selected_tab: usize,
    highlighted_skill: Option<SkillId>,
    /// Map of skills that are not selected at their highest learned level. Only
    /// contains skills that have this property.
    #[hidden_element]
    chosen_skill_level: HashMap<SkillId, SkillLevel>,
    description_title: String,
    description_body: String,
    /// Skill the description window was actually showing. A right-click toggles
    /// against this id, not the skill currently under the cursor.
    description_skill: Option<SkillId>,
    /// Hover asked to open or switch. Applied after input so a right-click in
    /// the same frame commits first.
    description_open: bool,
    description_pending_title: String,
    description_pending_body: String,
    description_pending_skill: Option<SkillId>,
}

impl SkillTreeWindowState {
    pub fn set_description(&mut self, skill_id: SkillId, title: String, body: String) {
        self.description_skill = Some(skill_id);
        self.description_title = title;
        self.description_body = body;
        self.description_open = false;
    }

    /// Drop a hover open recorded for this frame. The committed skill stays so
    /// a pointer that is still on that slot does not immediately ask to reopen.
    pub fn cancel_hover_open(&mut self) {
        self.description_open = false;
    }

    /// Copy a hover request into the committed description. Returns whether the
    /// window should be opened. A click that already ran clears
    /// `description_open`, so this does not replace the skill the click
    /// committed.
    pub fn take_hover_description(&mut self) -> bool {
        if !self.description_open {
            return false;
        }
        self.description_open = false;
        let Some(skill_id) = self.description_pending_skill else {
            return false;
        };
        self.description_skill = Some(skill_id);
        self.description_title.clone_from(&self.description_pending_title);
        self.description_body.clone_from(&self.description_pending_body);
        true
    }
}

pub struct SkillTreeWindow<A, B, C, D> {
    window_state_path: A,
    layout_path: B,
    skills_path: C,
    available_skill_points_path: D,
}

impl<A, B, C, D> SkillTreeWindow<A, B, C, D> {
    pub fn new(window_state_path: A, layout_path: B, skills_path: C, available_skill_points_path: D) -> Self {
        Self {
            window_state_path,
            layout_path,
            skills_path,
            available_skill_points_path,
        }
    }
}

impl<A, B, C, D> CustomWindow<ClientState> for SkillTreeWindow<A, B, C, D>
where
    A: Path<ClientState, SkillTreeWindowState>,
    B: Path<ClientState, SkillTreeLayout>,
    C: Path<ClientState, Vec<LearnedSkill>>,
    D: Path<ClientState, u32>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::SkillTree)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: client_state().localization().skill_tree_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: 550.0,
            maximum_width: 800.0,
            closable: true,
            elements: (
                split! {
                    gaps: theme().window().gaps(),
                    children: TabSelector::new(self.window_state_path.selected_tab(), self.layout_path),
                },
                DropSkillWrapper::new(
                    tabs! {
                        selected_tab: self.window_state_path.selected_tab(),
                        children: DynamicTabs::new(move |index|
                            SkillTreeTab::new(
                                self.layout_path.tabs().index(index).skills().manually_asserted(),
                                self.skills_path,
                                self.window_state_path,
                            )
                        )
                    }
                ),
                split! {
                    children: (
                        text! {
                            text: AvailablePointsDisplay::new(
                                client_state().localization().available_skill_points_text(),
                                self.available_skill_points_path,
                                self.window_state_path.pending_skill_points(),
                            ),
                            height: 20.0,
                        },
                        either! {
                            selector: self.window_state_path.currently_skilling(),
                            on_true: split! {
                                gaps: theme().window().gaps(),
                                children: (
                                    button! {
                                        text: client_state().localization().reset_skill_points_button_text(),
                                        event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                            state.update_value_with(self.window_state_path.pending_skill_points(), Vec::clear);
                                        },
                                    },
                                    button! {
                                        text: client_state().localization().cancel_skill_points_button_text(),
                                        event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                            state.update_value_with(self.window_state_path.pending_skill_points(), Vec::clear);
                                            state.update_value(self.window_state_path.currently_skilling(), false);
                                        },
                                    },
                                    button! {
                                        text: client_state().localization().apply_skill_points_button_text(),
                                        disabled: ComputedSelector::new_default(move |state: &ClientState| {
                                            self.window_state_path.pending_skill_points().follow_safe(state).is_empty()
                                        }),
                                        event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                            let pending_skill_points_path = self.window_state_path.pending_skill_points();
                                            let pending_skill_points = state.get(&pending_skill_points_path);
                                            let skill_ids = pending_skill_points.clone();

                                            state.update_value(self.window_state_path.currently_skilling(), false);
                                            state.update_value_with(pending_skill_points_path, Vec::clear);

                                            queue.queue(InputEvent::LevelUpSkills { skill_ids });
                                        },
                                    },
                                ),
                            },
                            on_false: button! {
                                text: client_state().localization().distribute_skill_points_button_text(),
                                disabled: ComputedSelector::new_default(move |state: &ClientState| {
                                    *self.available_skill_points_path.follow_safe(state) == 0
                                }),
                                event: SetToTrue(self.window_state_path.currently_skilling()),
                            },
                        },
                    )
                }
            ),
        }
    }
}

#[cfg(test)]
mod description_state_tests {
    use ragnarok_packets::SkillId;

    use super::SkillTreeWindowState;

    #[test]
    fn click_commit_cancels_the_hover_open() {
        let mut state = SkillTreeWindowState {
            description_pending_skill: Some(SkillId(1)),
            description_pending_title: "Bash".to_owned(),
            description_pending_body: "Level: 1 / 10".to_owned(),
            description_open: true,
            ..Default::default()
        };

        state.set_description(SkillId(2), "Magnum".to_owned(), "Level: 4 / 10".to_owned());

        assert!(!state.take_hover_description());
        assert_eq!(state.description_skill, Some(SkillId(2)));
        assert_eq!(state.description_body, "Level: 4 / 10");
    }

    #[test]
    fn hover_open_commits_the_pending_skill_when_no_click_runs() {
        let mut state = SkillTreeWindowState {
            description_pending_skill: Some(SkillId(1)),
            description_pending_title: "Bash".to_owned(),
            description_pending_body: "Level: 3 / 10".to_owned(),
            description_open: true,
            ..Default::default()
        };

        assert!(state.take_hover_description());
        assert_eq!(state.description_skill, Some(SkillId(1)));
        assert_eq!(state.description_title, "Bash");
        assert_eq!(state.description_body, "Level: 3 / 10");
        assert!(!state.description_open);
        assert!(!state.take_hover_description());
    }

    #[test]
    fn closing_cancels_a_hover_open_and_keeps_the_shown_skill() {
        let mut state = SkillTreeWindowState {
            description_pending_skill: Some(SkillId(1)),
            description_pending_body: "Level: 2 / 10".to_owned(),
            description_open: true,
            ..Default::default()
        };
        state.set_description(SkillId(1), "Bash".to_owned(), "Level: 1 / 10".to_owned());
        state.description_open = true;

        state.cancel_hover_open();

        assert!(!state.take_hover_description());
        assert_eq!(state.description_skill, Some(SkillId(1)));
        assert_eq!(state.description_body, "Level: 1 / 10");
    }
}
