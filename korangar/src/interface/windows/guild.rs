//! Guild window: create, roster, positions, skills, notice, relations, and
//! chat.

use std::cmp::Ordering;

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element, ElementBox, StateElement};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, PathExt, RustState, State, VecIndexExt};

use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::guild::{
    GPERM_EXPEL, GPERM_INVITE, GPERM_STORAGE, GuildMember, GuildMemberPathExt, GuildPosition, GuildPositionPathExt, GuildRelation,
    GuildRelationPathExt, GuildSkill, GuildSkillPathExt, GuildState, GuildStatePathExt, with_permission_bits,
};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

const NAME_MIN: usize = 4;
const NAME_LIMIT: usize = 24;
const SUBJECT_LIMIT: usize = 60;
const NOTICE_LIMIT: usize = 120;
const REASON_LIMIT: usize = 40;
const CHAT_LIMIT: usize = 70;
const TAX_LIMIT: usize = 3;
const EMBLEM_PIXEL: f32 = 2.0;

/// Drafts for the guild window. The server copy lives on [`GuildState`].
#[derive(RustState, StateElement)]
pub struct GuildWindowState {
    pub subject: String,
    pub notice: String,
    pub tab: u8,
    pub create_name: String,
    pub invite_name: String,
    pub leave_reason: String,
    pub expel_reason: String,
    pub chat_text: String,
    pub position_name: String,
    pub tax_text: String,
    pub assign_label: String,
    pub invite_perm: bool,
    pub expel_perm: bool,
    pub storage_perm: bool,
    pub draft_mode: i32,
    pub draft_ranking: i32,
    pub editing_position: i32,
}

impl Default for GuildWindowState {
    fn default() -> Self {
        Self {
            subject: String::new(),
            notice: String::new(),
            tab: 0,
            create_name: String::new(),
            invite_name: String::new(),
            leave_reason: String::new(),
            expel_reason: String::new(),
            chat_text: String::new(),
            position_name: String::new(),
            tax_text: "0".to_owned(),
            assign_label: "Assign selected rank".to_owned(),
            invite_perm: false,
            expel_perm: false,
            storage_perm: false,
            draft_mode: 0,
            draft_ranking: 0,
            editing_position: -1,
        }
    }
}

impl GuildWindowState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

struct GuildInfoBody<A> {
    guild_path: A,
}

impl<A> GuildInfoBody<A> {
    fn new(guild_path: A) -> Self {
        Self { guild_path }
    }
}

struct GuildInfoLayout {
    area: Area,
    row_height: f32,
    row_count: usize,
    emblem_height: u32,
    emblem_width: u32,
}

impl<A> Element<ClientState> for GuildInfoBody<A>
where
    A: Path<ClientState, GuildState>,
{
    type LayoutInfo = GuildInfoLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let guild = state.get(&self.guild_path);
            let row_count = guild.rows.len().max(1);
            let (emblem_width, emblem_height) = guild
                .emblem(guild.guild_id)
                .map(|emblem| (emblem.width, emblem.height))
                .unwrap_or((0, 0));
            let row_height = 18.0;
            let emblem_block = if emblem_height == 0 {
                0.0
            } else {
                emblem_height as f32 * EMBLEM_PIXEL + 6.0
            };
            let area = resolver.with_height(row_height * row_count as f32 + emblem_block);
            Self::LayoutInfo {
                area,
                row_height,
                row_count,
                emblem_height,
                emblem_width,
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let guild = state.get(&self.guild_path);
        let text_color = *state.get(&client_theme().tokens().text());
        let muted = *state.get(&client_theme().tokens().muted());
        let highlight = *state.get(&client_theme().tokens().gold());
        let row_count = guild.rows.len().min(layout_info.row_count);

        for (index, line) in guild.rows.iter().take(row_count).enumerate() {
            let area = Area {
                left: layout_info.area.left,
                top: layout_info.area.top + layout_info.row_height * index as f32,
                width: layout_info.area.width,
                height: layout_info.row_height,
            };
            let color = if index + 1 == guild.rows.len() { muted } else { text_color };
            layout.add_text(
                area,
                line.as_str(),
                FontSize(12.0),
                color,
                highlight,
                HorizontalAlignment::Left { offset: 4.0, border: 2.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        }

        let Some(emblem) = guild.emblem(guild.guild_id) else {
            return;
        };
        if emblem.width == 0 || emblem.height == 0 {
            return;
        }
        let origin_top = layout_info.area.top + layout_info.row_height * row_count as f32 + 4.0;
        let width = emblem.width.min(layout_info.emblem_width) as usize;
        let height = emblem.height.min(layout_info.emblem_height) as usize;
        for y in 0..height {
            for x in 0..width {
                let index = (y * emblem.width as usize + x) * 4;
                let Some(alpha) = emblem.pixels.get(index + 3).copied() else {
                    continue;
                };
                if alpha == 0 {
                    continue;
                }
                layout.add_rectangle(
                    Area {
                        left: layout_info.area.left + 4.0 + x as f32 * EMBLEM_PIXEL,
                        top: origin_top + y as f32 * EMBLEM_PIXEL,
                        width: EMBLEM_PIXEL,
                        height: EMBLEM_PIXEL,
                    },
                    CornerDiameter::default(),
                    Color::rgba_u8(emblem.pixels[index], emblem.pixels[index + 1], emblem.pixels[index + 2], alpha),
                    Color::rgba_u8(0, 0, 0, 0),
                    ShadowPadding::default(),
                );
            }
        }
    }
}

struct MemberList<M, G, W> {
    members_path: M,
    guild_path: G,
    window_path: W,
    elements: Vec<ElementBox<ClientState>>,
}

impl<M, G, W> MemberList<M, G, W> {
    fn new(members_path: M, guild_path: G, window_path: W) -> Self {
        Self {
            members_path,
            guild_path,
            window_path,
            elements: Vec::new(),
        }
    }
}

impl<M, G, W> Element<ClientState> for MemberList<M, G, W>
where
    M: Path<ClientState, Vec<GuildMember>> + Copy + 'static,
    G: Path<ClientState, GuildState> + Copy + 'static,
    W: Path<ClientState, GuildWindowState> + Copy + 'static,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        use korangar_interface::prelude::*;

        with_single_resolver(resolvers, |resolver| {
            let members = state.get(&self.members_path);
            let members_path = self.members_path;
            let guild_path = self.guild_path;
            let window_path = self.window_path;

            match members.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(members.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..members.len() {
                        let member_path = members_path.index(index).manually_asserted();
                        let label_path = member_path.label();
                        let reason_path = window_path.expel_reason();
                        let editing_path = window_path.editing_position();
                        let assign_path = window_path.assign_label();
                        self.elements.push(ErasedElement::new(collapsible! {
                            text: label_path,
                            children: fragment! {
                                gaps: theme().window().gaps(),
                                children: (
                                    button! {
                                        text: "Expel",
                                        disabled: ComputedSelector::new_default(move |state: &ClientState| {
                                            let guild = guild_path.follow_safe(state);
                                            let name = client_state().player_name().follow_safe(state);
                                            !guild.can(name, GPERM_EXPEL)
                                        }),
                                        event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                            let member = state.get(&member_path);
                                            let account_id = member.account_id;
                                            let character_id = member.character_id;
                                            let reason = state.get(&reason_path).clone();
                                            queue.queue(InputEvent::ExpelGuildMember {
                                                account_id,
                                                character_id,
                                                reason,
                                            });
                                        },
                                    },
                                    button! {
                                        text: assign_path,
                                        disabled: ComputedSelector::new_default(move |state: &ClientState| {
                                            *editing_path.follow_safe(state) < 0 || !*guild_path.is_master().follow_safe(state)
                                        }),
                                        event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                            let member = state.get(&member_path);
                                            let account_id = member.account_id;
                                            let character_id = member.character_id;
                                            let position_id = *state.get(&editing_path);
                                            if position_id >= 0 {
                                                queue.queue(InputEvent::SetGuildMemberPosition {
                                                    account_id,
                                                    character_id,
                                                    position_id,
                                                });
                                            }
                                        },
                                    },
                                ),
                            },
                        }));
                    }
                }
            }

            self.elements.iter_mut().zip(members.iter()).for_each(|(element, member)| {
                element.create_layout_info(state, store.child_store(member.character_id.0 as u64), resolver);
            });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let members = state.get(&self.members_path);
        self.elements.iter().zip(members.iter()).for_each(|(element, member)| {
            element.lay_out(state, store.child_store(member.character_id.0 as u64), &(), layout);
        });
    }
}

struct PositionList<P, W> {
    positions_path: P,
    window_path: W,
    elements: Vec<ElementBox<ClientState>>,
}

impl<P, W> PositionList<P, W> {
    fn new(positions_path: P, window_path: W) -> Self {
        Self {
            positions_path,
            window_path,
            elements: Vec::new(),
        }
    }
}

impl<P, W> Element<ClientState> for PositionList<P, W>
where
    P: Path<ClientState, Vec<GuildPosition>> + Copy + 'static,
    W: Path<ClientState, GuildWindowState> + Copy + 'static,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        use korangar_interface::prelude::*;

        with_single_resolver(resolvers, |resolver| {
            let positions = state.get(&self.positions_path);
            let positions_path = self.positions_path;
            let window_path = self.window_path;

            match positions.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(positions.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..positions.len() {
                        let position_path = positions_path.index(index).manually_asserted();
                        let label_path = position_path.label();
                        self.elements.push(ErasedElement::new(collapsible! {
                            text: label_path,
                            children: button! {
                                text: "Edit",
                                event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                    let position = state.get(&position_path);
                                    let id = position.id;
                                    let mode = position.mode;
                                    let ranking = position.ranking;
                                    let pay_rate = position.pay_rate;
                                    let name = position.name.clone();
                                    let title = if name.is_empty() {
                                        format!("Position {id}")
                                    } else {
                                        name.clone()
                                    };
                                    state.update_value(window_path.editing_position(), id);
                                    state.update_value(window_path.position_name(), name);
                                    state.update_value(window_path.tax_text(), pay_rate.to_string());
                                    state.update_value(window_path.invite_perm(), mode & GPERM_INVITE != 0);
                                    state.update_value(window_path.expel_perm(), mode & GPERM_EXPEL != 0);
                                    state.update_value(window_path.storage_perm(), mode & GPERM_STORAGE != 0);
                                    state.update_value(window_path.draft_mode(), mode);
                                    state.update_value(window_path.draft_ranking(), ranking);
                                    state.update_value(window_path.assign_label(), format!("Assign {title}"));
                                },
                            },
                        }));
                    }
                }
            }

            self.elements.iter_mut().zip(positions.iter()).for_each(|(element, position)| {
                element.create_layout_info(state, store.child_store(position.id as u64), resolver);
            });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let positions = state.get(&self.positions_path);
        self.elements.iter().zip(positions.iter()).for_each(|(element, position)| {
            element.lay_out(state, store.child_store(position.id as u64), &(), layout);
        });
    }
}

const SKILL_ICON: f32 = 24.0;

/// 24×24 skill sprite beside the guild skill row. The resource is the skill
/// tree bitmap for this skill id, not the Aegis name from `0x0162`.
struct GuildSkillIcon<P> {
    skill_path: P,
}

impl<P> GuildSkillIcon<P> {
    fn new(skill_path: P) -> Self {
        Self { skill_path }
    }
}

impl<P> Element<ClientState> for GuildSkillIcon<P>
where
    P: Path<ClientState, GuildSkill>,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let area = resolver.with_height(SKILL_ICON);
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
        let skill = state.get(&self.skill_path);
        let side = SKILL_ICON.min(layout_info.area.width).min(layout_info.area.height);
        let area = Area {
            left: layout_info.area.left,
            top: layout_info.area.top + (layout_info.area.height - side).max(0.0) / 2.0,
            width: side,
            height: side,
        };
        layout.add_rectangle(
            area,
            CornerDiameter::default(),
            *state.get(&client_theme().tokens().panel()),
            *state.get(&client_theme().tokens().border()),
            ShadowPadding::default(),
        );
        if let (Some(actions), Some(sprite)) = (&skill.actions, &skill.sprite) {
            // Identity tint. The bitmap supplies the color, same as the skill tree.
            layout.add_sprite(area, actions, sprite, &skill.animation_state, Color::WHITE, 1.0);
        }
    }
}

struct SkillList<S, G> {
    skills_path: S,
    guild_path: G,
    elements: Vec<ElementBox<ClientState>>,
}

impl<S, G> SkillList<S, G> {
    fn new(skills_path: S, guild_path: G) -> Self {
        Self {
            skills_path,
            guild_path,
            elements: Vec::new(),
        }
    }
}

impl<S, G> Element<ClientState> for SkillList<S, G>
where
    S: Path<ClientState, Vec<GuildSkill>> + Copy + 'static,
    G: Path<ClientState, GuildState> + Copy + 'static,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        use korangar_interface::prelude::*;

        with_single_resolver(resolvers, |resolver| {
            let skills = state.get(&self.skills_path);
            let skills_path = self.skills_path;
            let guild_path = self.guild_path;

            match skills.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(skills.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..skills.len() {
                        let skill_path = skills_path.index(index).manually_asserted();
                        let label_path = skill_path.label();
                        let points_path = guild_path.skill_points();
                        self.elements.push(ErasedElement::new(split! {
                            gaps: theme().window().gaps(),
                            children: (
                                GuildSkillIcon::new(skill_path),
                                text! { text: label_path },
                                button! {
                                    text: "Level up",
                                    disabled: ComputedSelector::new_default(move |state: &ClientState| {
                                        let skill = skill_path.follow_safe(state);
                                        let points = *points_path.follow_safe(state);
                                        !skill.upgradable || points <= 0
                                    }),
                                    event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                        let skill_id = state.get(&skill_path).skill_id;
                                        queue.queue(InputEvent::LevelUpGuildSkill { skill_id });
                                    },
                                },
                            ),
                        }));
                    }
                }
            }

            self.elements.iter_mut().zip(skills.iter()).for_each(|(element, skill)| {
                element.create_layout_info(state, store.child_store(u64::from(skill.skill_id)), resolver);
            });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let skills = state.get(&self.skills_path);
        self.elements.iter().zip(skills.iter()).for_each(|(element, skill)| {
            element.lay_out(state, store.child_store(u64::from(skill.skill_id)), &(), layout);
        });
    }
}

struct RelationList<R, G> {
    relations_path: R,
    guild_path: G,
    elements: Vec<ElementBox<ClientState>>,
}

impl<R, G> RelationList<R, G> {
    fn new(relations_path: R, guild_path: G) -> Self {
        Self {
            relations_path,
            guild_path,
            elements: Vec::new(),
        }
    }
}

impl<R, G> Element<ClientState> for RelationList<R, G>
where
    R: Path<ClientState, Vec<GuildRelation>> + Copy + 'static,
    G: Path<ClientState, GuildState> + Copy + 'static,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        use korangar_interface::prelude::*;

        with_single_resolver(resolvers, |resolver| {
            let relations = state.get(&self.relations_path);
            let relations_path = self.relations_path;
            let guild_path = self.guild_path;

            match relations.len().cmp(&self.elements.len()) {
                Ordering::Less => self.elements.truncate(relations.len()),
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..relations.len() {
                        let relation_path = relations_path.index(index).manually_asserted();
                        let label_path = relation_path.label();
                        self.elements.push(ErasedElement::new(collapsible! {
                            text: label_path,
                            children: button! {
                                text: "Break",
                                disabled: ComputedSelector::new_default(move |state: &ClientState| {
                                    !*guild_path.is_master().follow_safe(state)
                                }),
                                event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                    let relation = state.get(&relation_path);
                                    queue.queue(InputEvent::BreakGuildRelation {
                                        guild_id: relation.guild_id,
                                        opposition: relation.opposition,
                                    });
                                },
                            },
                        }));
                    }
                }
            }

            self.elements.iter_mut().zip(relations.iter()).for_each(|(element, relation)| {
                let key = (u64::from(relation.guild_id) << 1) | u64::from(relation.opposition == 1);
                element.create_layout_info(state, store.child_store(key), resolver);
            });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let relations = state.get(&self.relations_path);
        self.elements.iter().zip(relations.iter()).for_each(|(element, relation)| {
            let key = (u64::from(relation.guild_id) << 1) | u64::from(relation.opposition == 1);
            element.lay_out(state, store.child_store(key), &(), layout);
        });
    }
}

pub struct GuildWindow<A, B> {
    window_state_path: A,
    guild_path: B,
}

impl<A, B> GuildWindow<A, B> {
    pub fn new(window_state_path: A, guild_path: B) -> Self {
        Self {
            window_state_path,
            guild_path,
        }
    }
}

impl<A, B> CustomWindow<ClientState> for GuildWindow<A, B>
where
    A: Path<ClientState, GuildWindowState> + Copy + 'static,
    B: Path<ClientState, GuildState> + Copy + 'static,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Guild)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct SubjectBox;
        struct NoticeBox;
        struct CreateNameBox;
        struct InviteNameBox;
        struct LeaveReasonBox;
        struct ExpelReasonBox;
        struct ChatBox;
        struct PositionNameBox;
        struct TaxBox;

        let window_path = self.window_state_path;
        let guild_path = self.guild_path;
        let tab_path = window_path.tab();
        let subject_path = window_path.subject();
        let notice_path = window_path.notice();
        let create_name_path = window_path.create_name();
        let invite_name_path = window_path.invite_name();
        let leave_reason_path = window_path.leave_reason();
        let expel_reason_path = window_path.expel_reason();
        let chat_path = window_path.chat_text();
        let position_name_path = window_path.position_name();
        let tax_path = window_path.tax_text();
        let editing_path = window_path.editing_position();
        let status_path = guild_path.status();

        let save_notice = || {
            move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                let subject = state.get(&subject_path).clone();
                let notice = state.get(&notice_path).clone();
                queue.queue(InputEvent::UpdateGuildNotice { subject, notice });
            }
        };
        let create_guild = || {
            move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                let name = state.get(&create_name_path).trim().to_owned();
                if name.len() >= NAME_MIN {
                    state.update_value_with(create_name_path, String::clear);
                    queue.queue(InputEvent::CreateGuild { name });
                }
            }
        };
        let invite = || {
            move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                let name = state.get(&invite_name_path).trim().to_owned();
                if name.len() >= NAME_MIN {
                    state.update_value_with(invite_name_path, String::clear);
                    queue.queue(InputEvent::InviteToGuild { name });
                }
            }
        };
        let leave = || {
            move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                let reason = state.get(&leave_reason_path).clone();
                queue.queue(InputEvent::LeaveGuild { reason });
            }
        };
        let send_chat = || {
            move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                let text = state.get(&chat_path).trim().to_owned();
                if !text.is_empty() {
                    state.update_value_with(chat_path, String::clear);
                    queue.queue(InputEvent::SendGuildMessage { text });
                }
            }
        };
        let save_position = || {
            move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                let position_id = *state.get(&editing_path);
                if position_id < 0 {
                    return;
                }
                let tax_text = state.get(&tax_path).trim();
                let Ok(pay_rate) = tax_text.parse::<i32>() else {
                    state.update_value(status_path, "Tax is a percent from 0 to 100.".to_owned());
                    return;
                };
                if !(0..=100).contains(&pay_rate) {
                    state.update_value(status_path, "Tax is a percent from 0 to 100.".to_owned());
                    return;
                }
                let name = state.get(&position_name_path).clone();
                let mode = with_permission_bits(
                    *state.get(&window_path.draft_mode()),
                    *state.get(&window_path.invite_perm()),
                    *state.get(&window_path.expel_perm()),
                    *state.get(&window_path.storage_perm()),
                );
                let ranking = *state.get(&window_path.draft_ranking());
                queue.queue(InputEvent::SaveGuildPosition {
                    position_id,
                    name,
                    mode,
                    ranking,
                    pay_rate,
                });
            }
        };

        let select_tab = |tab: u8| {
            move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                state.update_value(tab_path, tab);
            }
        };
        let tab_is = |tab: u8| ComputedSelector::new_default(move |state: &ClientState| *tab_path.follow_safe(state) == tab);
        let not_editing = || ComputedSelector::new_default(move |state: &ClientState| *editing_path.follow_safe(state) < 0);
        let not_master = || ComputedSelector::new_default(move |state: &ClientState| !*guild_path.is_master().follow_safe(state));
        let can_invite = ComputedSelector::new_default(move |state: &ClientState| {
            let guild = guild_path.follow_safe(state);
            let name = client_state().player_name().follow_safe(state);
            guild.can(name, GPERM_INVITE)
        });

        window! {
            title: "Guild",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: 420.0,
            maximum_width: 640.0,
            closable: true,
            elements: (
                crate::interface::windows::WindowGroupTabs::new(WindowClass::Guild),
                button! {
                    text: "Refresh",
                    event: InputEvent::RequestGuildWindow,
                },
                text! { text: status_path },
                either! {
                    selector: guild_path.joined(),
                    on_true: split! {
                        gaps: theme().window().gaps(),
                        children: (
                            split! {
                                gaps: theme().window().gaps(),
                                children: (
                                    button! {
                                        text: "Info",
                                        event: select_tab(0),
                                        disabled: tab_is(0),
                                    },
                                    button! {
                                        text: "Members",
                                        event: select_tab(1),
                                        disabled: tab_is(1),
                                    },
                                    button! {
                                        text: "Positions",
                                        event: select_tab(2),
                                        disabled: tab_is(2),
                                    },
                                    button! {
                                        text: "Skills",
                                        event: select_tab(3),
                                        disabled: tab_is(3),
                                    },
                                    button! {
                                        text: "Notice",
                                        event: select_tab(4),
                                        disabled: tab_is(4),
                                    },
                                ),
                            },
                            either! {
                                selector: tab_is(0),
                                on_true: split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        GuildInfoBody::new(guild_path),
                                        button! {
                                            text: "Upload emblem",
                                            disabled: not_master(),
                                            event: InputEvent::UploadGuildEmblem,
                                        },
                                        RelationList::new(guild_path.relations(), guild_path),
                                    ),
                                },
                                on_false: either! {
                                    selector: tab_is(1),
                                    on_true: split! {
                                        gaps: theme().window().gaps(),
                                        children: (
                                            text! { text: "Expel reason" },
                                            text_box! {
                                                ghost_text: "Reason",
                                                state: expel_reason_path,
                                                input_handler: DefaultHandler::<_, _, REASON_LIMIT>::new(expel_reason_path, |_state: &State<ClientState>, _: &mut EventQueue<ClientState>| {}),
                                                focus_id: ExpelReasonBox,
                                            },
                                            MemberList::new(guild_path.members(), guild_path, window_path),
                                        ),
                                    },
                                    on_false: either! {
                                        selector: tab_is(2),
                                        on_true: split! {
                                            gaps: theme().window().gaps(),
                                            children: (
                                                text_box! {
                                                    ghost_text: "Position title",
                                                    state: position_name_path,
                                                    input_handler: DefaultHandler::<_, _, NAME_LIMIT>::new(position_name_path, save_position()),
                                                    focus_id: PositionNameBox,
                                                },
                                                state_button! {
                                                    text: "Invite",
                                                    state: window_path.invite_perm(),
                                                    event: Toggle(window_path.invite_perm()),
                                                    disabled: not_editing(),
                                                },
                                                state_button! {
                                                    text: "Expel",
                                                    state: window_path.expel_perm(),
                                                    event: Toggle(window_path.expel_perm()),
                                                    disabled: not_editing(),
                                                },
                                                state_button! {
                                                    text: "Storage",
                                                    state: window_path.storage_perm(),
                                                    event: Toggle(window_path.storage_perm()),
                                                    disabled: not_editing(),
                                                },
                                                text_box! {
                                                    ghost_text: "Tax %",
                                                    state: tax_path,
                                                    input_handler: DefaultHandler::<_, _, TAX_LIMIT>::new(tax_path, save_position()),
                                                    focus_id: TaxBox,
                                                },
                                                button! {
                                                    text: "Save position",
                                                    disabled: not_editing(),
                                                    event: save_position(),
                                                },
                                                PositionList::new(guild_path.positions(), window_path),
                                            ),
                                        },
                                        on_false: either! {
                                            selector: tab_is(3),
                                            on_true: split! {
                                                gaps: theme().window().gaps(),
                                                children: (
                                                    text! { text: guild_path.skill_points_label() },
                                                    SkillList::new(guild_path.skills(), guild_path),
                                                ),
                                            },
                                            on_false: split! {
                                                gaps: theme().window().gaps(),
                                                children: (
                                                    text! { text: "Subject" },
                                                    text_box! {
                                                        ghost_text: "Guild subject",
                                                        state: subject_path,
                                                        input_handler: DefaultHandler::<_, _, SUBJECT_LIMIT>::new(subject_path, save_notice()),
                                                        focus_id: SubjectBox,
                                                    },
                                                    text! { text: "Notice" },
                                                    text_box! {
                                                        ghost_text: "Guild notice",
                                                        state: notice_path,
                                                        input_handler: DefaultHandler::<_, _, NOTICE_LIMIT>::new(notice_path, save_notice()),
                                                        focus_id: NoticeBox,
                                                    },
                                                    button! {
                                                        text: "Save notice",
                                                        disabled: not_master(),
                                                        event: save_notice(),
                                                    },
                                                ),
                                            },
                                        },
                                    },
                                },
                            },
                            either! {
                                selector: can_invite,
                                on_true: split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        text_box! {
                                            ghost_text: "Character name to invite",
                                            state: invite_name_path,
                                            input_handler: DefaultHandler::<_, _, NAME_LIMIT>::new(invite_name_path, invite()),
                                            focus_id: InviteNameBox,
                                        },
                                        button! {
                                            text: "Invite",
                                            event: invite(),
                                        },
                                    ),
                                },
                                on_false: EmptyElement,
                            },
                            text_box! {
                                ghost_text: "Leave reason",
                                state: leave_reason_path,
                                input_handler: DefaultHandler::<_, _, REASON_LIMIT>::new(leave_reason_path, leave()),
                                focus_id: LeaveReasonBox,
                            },
                            button! {
                                text: "Leave guild",
                                event: leave(),
                            },
                            text_box! {
                                ghost_text: "Guild chat",
                                state: chat_path,
                                input_handler: DefaultHandler::<_, _, CHAT_LIMIT>::new(chat_path, send_chat()),
                                focus_id: ChatBox,
                            },
                            button! {
                                text: "Send",
                                event: send_chat(),
                            },
                        ),
                    },
                    on_false: split! {
                        gaps: theme().window().gaps(),
                        children: (
                            text! { text: "You are not in a guild." },
                            text_box! {
                                ghost_text: "Guild name",
                                state: create_name_path,
                                input_handler: DefaultHandler::<_, _, NAME_LIMIT>::new(create_name_path, create_guild()),
                                focus_id: CreateNameBox,
                            },
                            button! {
                                text: "Create guild",
                                event: create_guild(),
                            },
                        ),
                    },
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drafts_clear_back_to_no_position() {
        let mut window = GuildWindowState {
            create_name: "Wolves".to_owned(),
            editing_position: 2,
            assign_label: "Assign Raider".to_owned(),
            ..GuildWindowState::default()
        };
        window.clear();
        assert!(window.create_name.is_empty());
        assert_eq!(window.editing_position, -1);
        assert_eq!(window.assign_label, "Assign selected rank");
        assert_eq!(window.tab, 0);
    }
}
