//! Party tab inside the Party / Friends window.
//!
//! Layout follows roBrowser PartyFriends: create, member list with HP bars,
//! leader crown, invite by name, kick, change leader, and share options.

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use ragnarok_packets::AccountId;
use rust_state::{ManuallyAssertExt, Path, PathExt, State, VecIndexExt};

use super::friend_list::{FriendListWindowState, FriendListWindowStatePathExt};
use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::input::InputEvent;
use crate::state::party::{PartyMember, PartyState, PartyStatePathExt};
use crate::state::theme::{ChatThemePathExt, InterfaceThemePathExt, SkillTreeThemePathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

const NAME_LIMIT: usize = 23;

struct Hidden;

impl Element<ClientState> for Hidden {
    type LayoutInfo = ();

    fn create_layout_info(&mut self, _: &State<ClientState>, _: ElementStoreMut, _: &mut dyn Resolvers<ClientState>) {}

    fn lay_out<'a>(
        &'a self,
        _: &'a State<ClientState>,
        _: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        _: &mut WindowLayout<'a, ClientState>,
    ) {
    }
}

struct PartyMemberLine<P> {
    member_path: P,
}

impl<P> PartyMemberLine<P> {
    fn new(member_path: P) -> Self {
        Self { member_path }
    }
}

impl<P> Element<ClientState> for PartyMemberLine<P>
where
    P: Path<ClientState, PartyMember>,
{
    type LayoutInfo = Area;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            let max_spell = state.get(&self.member_path).maximum_spell_points;
            let text_height = state.get(&client_theme().text()).height;
            let spell_row = if max_spell > 0 { 10.0 } else { 0.0 };
            resolver.with_height(text_height + 8.0 + spell_row)
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let text_theme = client_theme().text();
        let text = state.get(&text_theme);
        let font_size = text.font_size;
        let text_height = text.height;
        let horizontal = text.horizontal_alignment;
        let vertical = text.vertical_alignment;
        let overflow = text.overflow_behavior;
        let neutral = text.color;
        let highlight = state.get(&client_theme().window()).highlight_color;
        let offline = *state.get(&client_theme().skill_tree().unlearned_skill_color());
        let dead_color = *state.get(&client_theme().chat().error_color());
        let world_theme = client_state().world_theme();
        let world = state.get(&world_theme);
        let bar_background = world.status_bar.background_color;
        let health_color = world.status_bar.player_health_color;
        let spell_color = world.status_bar.spell_point_color;

        let member = state.get(&self.member_path);
        let color = if member.leader {
            highlight
        } else if member.dead {
            dead_color
        } else if !member.online {
            offline
        } else {
            neutral
        };
        let text_area = Area {
            left: layout_info.left,
            top: layout_info.top,
            width: layout_info.width,
            height: text_height,
        };
        layout.add_text(
            text_area,
            member.label.as_str(),
            font_size,
            color,
            highlight,
            horizontal,
            vertical,
            overflow,
        );

        let health_area = Area {
            left: layout_info.left,
            top: layout_info.top + text_height,
            width: layout_info.width,
            height: 8.0,
        };
        draw_bar(
            layout,
            health_area,
            member.health,
            member.maximum_health,
            bar_background,
            health_color,
        );
        if member.maximum_spell_points > 0 {
            let spell_area = Area {
                left: layout_info.left,
                top: health_area.top + 10.0,
                width: layout_info.width,
                height: 8.0,
            };
            draw_bar(
                layout,
                spell_area,
                member.spell_points,
                member.maximum_spell_points,
                bar_background,
                spell_color,
            );
        }
    }
}

fn draw_bar(layout: &mut WindowLayout<ClientState>, area: Area, current: i32, maximum: i32, background: Color, fill: Color) {
    let corner = CornerDiameter::uniform(2.0);
    let shadow = ShadowPadding::uniform(0.0);
    layout.add_rectangle(area, corner, background, background, shadow);
    if maximum <= 0 || area.width <= 0.0 {
        return;
    }
    let ratio = (current.max(0) as f32 / maximum as f32).clamp(0.0, 1.0);
    if ratio <= 0.0 {
        return;
    }
    let filled = Area {
        width: area.width * ratio,
        ..area
    };
    layout.add_rectangle(filled, corner, fill, fill, shadow);
}

struct PartyRoster<P> {
    party_path: P,
    elements: Vec<ElementBox<ClientState>>,
    signature: (bool, AccountId, Vec<AccountId>),
}

impl<P> PartyRoster<P>
where
    P: Path<ClientState, PartyState> + Clone + 'static,
{
    fn new(party_path: P) -> Self {
        Self {
            party_path,
            elements: Vec::new(),
            signature: (false, AccountId(0), Vec::new()),
        }
    }

    fn sync(&mut self, state: &State<ClientState>) {
        use korangar_interface::prelude::*;

        let party = state.get(&self.party_path);
        let signature = (
            party.self_is_leader,
            party.local_account_id,
            party.members.iter().map(|member| member.account_id).collect::<Vec<_>>(),
        );
        if signature == self.signature {
            return;
        }
        self.signature = signature;
        self.elements.clear();

        let leader = state.get(&self.party_path).self_is_leader;
        let local_account = state.get(&self.party_path).local_account_id;
        let count = state.get(&self.party_path).members.len();
        for index in 0..count {
            let member_path = self.party_path.members().index(index).manually_asserted();
            let account_id = state.get(&member_path).account_id;
            let show_actions = leader && account_id != local_account;
            let line = PartyMemberLine::new(member_path);
            if show_actions {
                self.elements.push(ErasedElement::new(split! {
                    gaps: theme().window().gaps(),
                    children: (
                        line,
                        button! {
                            text: "Kick",
                            event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                let member = state.get(&member_path);
                                queue.queue(InputEvent::KickPartyMember {
                                    account_id: member.account_id,
                                    name: member.name.clone(),
                                });
                            },
                        },
                        button! {
                            text: "Make leader",
                            event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                let account_id = state.get(&member_path).account_id;
                                queue.queue(InputEvent::ChangePartyLeader { account_id });
                            },
                        },
                    ),
                }));
            } else {
                self.elements.push(ErasedElement::new(line));
            }
        }
    }
}

impl<P> Element<ClientState> for PartyRoster<P>
where
    P: Path<ClientState, PartyState> + Clone + 'static,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            self.sync(state);
            for (index, element) in self.elements.iter_mut().enumerate() {
                element.create_layout_info(state, store.child_store(index as u64), resolver);
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        for (index, element) in self.elements.iter().enumerate() {
            element.lay_out(state, store.child_store(index as u64), &(), layout);
        }
    }
}

pub struct PartyTab<W, P> {
    element: ElementBox<ClientState>,
    _window: std::marker::PhantomData<W>,
    _party: std::marker::PhantomData<P>,
}

impl<W, P> PartyTab<W, P>
where
    W: Path<ClientState, FriendListWindowState> + Clone + 'static,
    P: Path<ClientState, PartyState> + Clone + 'static,
{
    pub fn new(window_path: W, party_path: P) -> Self {
        use korangar_interface::prelude::*;

        struct CreatePartyName;
        struct InvitePartyName;

        let create_name = window_path.create_name();
        let create_pickup = window_path.create_share_pickup();
        let create_division = window_path.create_share_loot();
        let create_from_box = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue_create_party(state, queue, create_name, create_pickup, create_division);
        };
        let create_from_button = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue_create_party(state, queue, create_name, create_pickup, create_division);
        };

        let invite_name = window_path.invite_name();
        let invite_from_box = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue_invite(state, queue, invite_name);
        };
        let invite_from_button = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue_invite(state, queue, invite_name);
        };

        let experience = party_path.experience_share();
        let pickup = party_path.item_pickup();
        let division = party_path.item_division();
        let locked = party_path.experience_share_locked();
        let toggle_experience = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            let experience_share = u32::from(*state.get(&experience) == 0);
            queue.queue(InputEvent::SetPartyOptions {
                experience_share,
                item_pickup: *state.get(&pickup),
                item_division: *state.get(&division),
            });
        };
        let toggle_pickup = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::SetPartyOptions {
                experience_share: *state.get(&experience),
                item_pickup: !*state.get(&pickup),
                item_division: *state.get(&division),
            });
        };
        let toggle_division = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            queue.queue(InputEvent::SetPartyOptions {
                experience_share: *state.get(&experience),
                item_pickup: *state.get(&pickup),
                item_division: !*state.get(&division),
            });
        };

        let element = ErasedElement::new(split! {
            gaps: theme().window().gaps(),
            children: (
                either! {
                    selector: party_path.joined(),
                    on_true: split! {
                        gaps: theme().window().gaps(),
                        children: (
                            text! { text: party_path.name(), },
                            PartyRoster::new(party_path),
                            button! {
                                text: "Leave",
                                event: InputEvent::LeaveParty,
                            },
                        ),
                    },
                    on_false: split! {
                        gaps: theme().window().gaps(),
                        children: (
                            text! { text: "You are not in a party.", },
                            text_box! {
                                ghost_text: "Party name",
                                state: create_name,
                                input_handler: DefaultHandler::<_, _, NAME_LIMIT>::new(create_name, create_from_box),
                                focus_id: CreatePartyName,
                            },
                            state_button! {
                                text: "Share item pickup",
                                state: create_pickup,
                                event: Toggle(create_pickup),
                            },
                            state_button! {
                                text: "Share loot",
                                state: create_division,
                                event: Toggle(create_division),
                            },
                            button! {
                                text: "Create",
                                event: create_from_button,
                            },
                        ),
                    },
                },
                either! {
                    selector: party_path.self_is_leader(),
                    on_true: split! {
                        gaps: theme().window().gaps(),
                        children: (
                            text! { text: "Invite by name", },
                            text_box! {
                                ghost_text: "Character name",
                                state: invite_name,
                                input_handler: DefaultHandler::<_, _, NAME_LIMIT>::new(invite_name, invite_from_box),
                                focus_id: InvitePartyName,
                            },
                            button! {
                                text: "Invite",
                                event: invite_from_button,
                            },
                            button! {
                                text: "Share experience",
                                disabled: ComputedSelector::new_default(move |state: &ClientState| *locked.follow_safe(state)),
                                event: toggle_experience,
                            },
                            button! {
                                text: "Share item pickup",
                                event: toggle_pickup,
                            },
                            button! {
                                text: "Share loot",
                                event: toggle_division,
                            },
                        ),
                    },
                    on_false: Hidden,
                },
                either! {
                    selector: party_path.invitations_allowed(),
                    on_true: button! {
                        text: "Refuse party invites",
                        event: InputEvent::SetPartyInviteBlock { refuse: true },
                    },
                    on_false: button! {
                        text: "Allow party invites",
                        event: InputEvent::SetPartyInviteBlock { refuse: false },
                    },
                },
            ),
        });

        Self {
            element,
            _window: std::marker::PhantomData,
            _party: std::marker::PhantomData,
        }
    }
}

impl<W, P> Element<ClientState> for PartyTab<W, P> {
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        self.element.create_layout_info(state, store, resolvers)
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        self.element.lay_out(state, store, layout_info, layout)
    }
}

fn queue_create_party<N, A, B>(
    state: &State<ClientState>,
    queue: &mut korangar_interface::event::EventQueue<ClientState>,
    name_path: N,
    pickup_path: A,
    division_path: B,
) where
    N: Path<ClientState, String> + Clone,
    A: Path<ClientState, bool>,
    B: Path<ClientState, bool>,
{
    let name = state.get(&name_path).trim().to_owned();
    if name.is_empty() {
        return;
    }
    let item_pickup = *state.get(&pickup_path);
    let item_division = *state.get(&division_path);
    state.update_value_with(name_path, |input| input.clear());
    queue.queue(InputEvent::CreateParty {
        name,
        item_pickup,
        item_division,
    });
    queue.queue(korangar_interface::event::Event::Unfocus);
}

fn queue_invite<N>(state: &State<ClientState>, queue: &mut korangar_interface::event::EventQueue<ClientState>, name_path: N)
where
    N: Path<ClientState, String> + Clone,
{
    let character_name = state.get(&name_path).trim().to_owned();
    if character_name.is_empty() {
        return;
    }
    state.update_value_with(name_path, |input| input.clear());
    queue.queue(InputEvent::InviteToParty { character_name });
    queue.queue(korangar_interface::event::Event::Unfocus);
}

#[cfg(test)]
mod tests {
    use super::super::friend_list::FriendListWindowState;

    #[test]
    fn party_tab_is_the_default() {
        let window = FriendListWindowState::default();
        assert!(!window.show_friends);
    }
}
