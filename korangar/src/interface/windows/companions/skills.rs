//! Homunculus and mercenary skill rows (SkillListMH): icon, name, level, and
//! skill-up when the server marked the skill upgradable.

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element, ErasedElement};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use rust_state::{Path, State};

use super::bars::paint_icon_line;
use super::state::CompanionSkill;
use crate::graphics::{CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::font_scale::from_theme_body;
use crate::state::theme::InterfaceThemePathExt;
use crate::state::{ClientState, client_theme};

const ROW_HEIGHT: f32 = 28.0;

struct SkillClick<P> {
    skills: P,
    index: usize,
    offer_level_up: bool,
}

impl<P> ClickHandler<ClientState> for SkillClick<P>
where
    P: Path<ClientState, Vec<CompanionSkill>>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        if !self.offer_level_up {
            return;
        }
        let Some(skill) = state.get(&self.skills).get(self.index) else {
            return;
        };
        if skill.upgradable {
            queue.queue(InputEvent::LevelUpCompanionSkill { skill_id: skill.skill_id });
        }
    }
}

pub(crate) struct SkillList<P> {
    skills: P,
    offer_level_up: bool,
    clicks: Vec<SkillClick<P>>,
}

impl<P> SkillList<P>
where
    P: Clone,
{
    pub(crate) fn new(skills: P, offer_level_up: bool) -> Self {
        Self {
            skills,
            offer_level_up,
            clicks: Vec::new(),
        }
    }
}

impl<P> Element<ClientState> for SkillList<P>
where
    P: Path<ClientState, Vec<CompanionSkill>> + Clone + 'static,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let len = state.get(&self.skills).len();
        if self.clicks.len() != len {
            self.clicks = (0..len)
                .map(|index| SkillClick {
                    skills: self.skills,
                    index,
                    offer_level_up: self.offer_level_up,
                })
                .collect();
        }
        let height = if len == 0 { 0.0 } else { len as f32 * ROW_HEIGHT };
        with_single_resolver(resolvers, |resolver| BaseLayoutInfo {
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
        let skills = state.get(&self.skills);
        if skills.is_empty() || self.clicks.len() != skills.len() {
            return;
        }
        let theme = client_theme().tokens();
        let tokens = state.get(&theme);
        let corner = CornerDiameter::uniform(tokens.corner_radius.min(6.0));
        let row_height = layout_info.area.height / skills.len() as f32;
        for (index, skill) in skills.iter().enumerate() {
            let row = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + row_height * index as f32,
                width: layout_info.area.width,
                height: row_height,
            };
            let hovered = row.check().run(layout);
            let border = if hovered && skill.upgradable && self.offer_level_up {
                tokens.hover
            } else {
                tokens.border
            };
            layout.add_rectangle(row, corner, border, tokens.raised, ShadowPadding::uniform(0.0));
            paint_icon_line(
                layout,
                row,
                skill.texture.as_ref(),
                skill.label.as_str(),
                tokens.text,
                from_theme_body(tokens.font_size, 14.0),
            );
            if hovered && skill.upgradable && self.offer_level_up {
                layout.register_click_handler(MouseButton::Left, &self.clicks[index]);
            }
        }
    }
}

pub(crate) fn boxed_skills<P>(skills: P, offer_level_up: bool) -> Box<ErasedElement<ClientState, SkillList<P>>>
where
    P: Path<ClientState, Vec<CompanionSkill>> + Clone + 'static,
{
    ErasedElement::new(SkillList::new(skills, offer_level_up))
}
