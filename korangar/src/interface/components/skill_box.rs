use korangar_interface::MouseMode;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, DropHandler, Event, EventQueue};
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use ragnarok_packets::SkillLevel;
use rust_state::{Path, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::{InputEvent, MouseInputMode};
use crate::interface::resource::SkillSource;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::skills::{LearnableSkill, LearnedSkill};
use crate::state::theme::{GlobalThemePathExt, InterfaceThemePathExt, SkillTreeThemePathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

const COOLDOWN_LABELS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

struct LevelDisplay {
    maximum_level: SkillLevel,
    string: Option<String>,
}

impl Default for LevelDisplay {
    fn default() -> Self {
        Self {
            maximum_level: SkillLevel(0),
            string: Default::default(),
        }
    }
}

impl LevelDisplay {
    fn update(&mut self, new_maximum_level: SkillLevel) {
        if self.string.is_none() || new_maximum_level != self.maximum_level {
            self.string = Some(new_maximum_level.0.to_string());
            self.maximum_level = new_maximum_level;
        }
    }
}

struct SkillBoxHandler<A> {
    skill_path: A,
    source: SkillSource,
}

impl<A> SkillBoxHandler<A> {
    fn new(skill_path: A, source: SkillSource) -> Self {
        Self { skill_path, source }
    }
}

impl<A> ClickHandler<ClientState> for SkillBoxHandler<A>
where
    A: Path<ClientState, LearnableSkill, false>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        // Unwrapping here is fine since we only register the handler if the slot has a
        // skill.
        let skill = state.try_get(&self.skill_path).unwrap().clone();

        queue.queue(Event::SetMouseMode {
            mouse_mode: MouseMode::Custom {
                mode: MouseInputMode::MoveSkill {
                    skill,
                    source: self.source,
                },
            },
        });
    }
}

impl<P> DropHandler<ClientState> for SkillBoxHandler<P>
where
    P: Path<ClientState, LearnableSkill, false>,
{
    fn handle_drop(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>, mouse_mode: &MouseMode<ClientState>) {
        if let MouseMode::Custom {
            mode: MouseInputMode::MoveSkill { source, skill },
        } = mouse_mode
        {
            queue.queue(InputEvent::MoveSkill {
                source: *source,
                destination: self.source,
                skill: skill.clone(),
            });
        }
    }
}

pub struct SkillBox<A, B> {
    learnable_skill_path: A,
    learned_skill_path: B,
    handler: SkillBoxHandler<A>,
    level_display: LevelDisplay,
    source: SkillSource,
}

impl<A, B> SkillBox<A, B>
where
    A: Copy,
    B: Copy,
{
    /// This function is supposed to be called from a component macro
    /// and not intended to be called manually.
    #[inline(always)]
    pub fn component_new(learnable_skill_path: A, learned_skill_path: B, source: SkillSource) -> Self {
        Self {
            learnable_skill_path,
            learned_skill_path,
            handler: SkillBoxHandler::new(learnable_skill_path, source),
            level_display: LevelDisplay::default(),
            source,
        }
    }
}

impl<A, B> Element<ClientState> for SkillBox<A, B>
where
    A: Path<ClientState, LearnableSkill, false>,
    B: Path<ClientState, LearnedSkill, false>,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let area = resolver.with_height(40.0);

            if let Some(learnable_skill) = state.try_get(&self.learnable_skill_path) {
                self.level_display.update(learnable_skill.maximum_level);
            }

            Self::LayoutInfo { area }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        use korangar_interface::prelude::*;

        let is_drop_target = match layout.get_mouse_mode() {
            MouseMode::Custom {
                mode: MouseInputMode::MoveSkill { source, .. },
            } => *source != self.source,
            _ => false,
        };

        let is_hovered = match is_drop_target {
            true => layout_info.area.check().any_mouse_mode().run(layout),
            false => layout_info.area.check().run(layout),
        };

        let (border, fill) = crate::interface::components::item_box::slot_border_and_fill(state, is_hovered, is_drop_target);
        crate::interface::components::item_box::paint_item_slot(layout, layout_info.area, border, fill, 4.0, true);

        if is_drop_target {
            let color = match is_hovered {
                true => *state.get(&client_theme().global().hovered_drop_area_color()),
                false => *state.get(&client_theme().global().drop_area_color()),
            };

            layout.add_rectangle(
                layout_info.area,
                *state.get(&theme().window().corner_diameter()),
                color.multiply_alpha(*state.get(&client_theme().global().fill_alpha())),
                color,
                *state.get(&client_theme().global().drop_area_outline()),
            );

            if is_hovered {
                // Since we are not in default mouse mode we need to mark the window as
                // hovered.
                layout.set_hovered();

                layout.register_drop_handler(&self.handler);
            }
        }

        if let Some(learnable_skill) = state.try_get(&self.learnable_skill_path) {
            let color = match state
                .try_get(&self.learned_skill_path)
                .is_some_and(|learned_skill| learned_skill.skill_level.0 >= learnable_skill.maximum_level.0)
            {
                true => Color::WHITE,
                false => *state.get(&client_theme().skill_tree().unlearned_skill_color()),
            };

            if let Some(actions) = &learnable_skill.actions
                && let Some(sprite) = &learnable_skill.sprite
            {
                layout.add_sprite(layout_info.area, actions, sprite, &learnable_skill.animation_state, color, 1.0);
            }

            if let Some(remaining) = state
                .get(&client_state().skill_cooldowns())
                .remaining_seconds(learnable_skill.skill_id, *state.get(&client_state().client_tick()))
            {
                layout.add_rectangle(
                    layout_info.area,
                    CornerDiameter::uniform(8.0),
                    (*state.get(&client_theme().window().background_color())).with_alpha_u8(170),
                    Color::rgba_u8(0, 0, 0, 0),
                    ShadowPadding::uniform(0.0),
                );
                // Stable per-call label via display of remaining seconds only (single
                // digit/short). Use level_display-like caching would be nicer;
                // format into a small static is unsafe, so show via
                // tooltip-only is insufficient — add_text needs &'a str. Keep a tiny
                // leak-free approach: only digits 1..999 fit in a const table.
                let label = if (remaining as usize) < COOLDOWN_LABELS.len() {
                    COOLDOWN_LABELS[remaining as usize]
                } else {
                    "9+"
                };
                layout.add_text(
                    layout_info.area,
                    label,
                    FontSize(14.0),
                    *state.get(&client_theme().text().color()),
                    *state.get(&client_theme().skill_tree().highlight_color()),
                    HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                    VerticalAlignment::Center { offset: 0.0 },
                    OverflowBehavior::Shrink,
                );
            }

            if is_hovered {
                layout.register_click_handler(MouseButton::Left, &self.handler);

                struct SkillBoxTooltip;
                layout.add_tooltip(&learnable_skill.skill_name, SkillBoxTooltip.tooltip_id());
            }

            layout.add_text(
                layout_info.area,
                self.level_display.string.as_ref().unwrap(),
                // TODO: Put this in the theme
                FontSize(12.0),
                // TODO: Put this in the theme
                *state.get(&client_theme().skill_tree().name_text_color()),
                *state.get(&client_theme().skill_tree().highlight_color()),
                // TODO: Put this in the theme
                HorizontalAlignment::Right { offset: 3.0, border: 3.0 },
                // TODO: Put this in the theme
                VerticalAlignment::Bottom { offset: 3.0 },
                OverflowBehavior::Shrink,
            );
        }
    }
}
