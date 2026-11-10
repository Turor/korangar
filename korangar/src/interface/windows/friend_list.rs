use std::cmp::Ordering;

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox, StateElement};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{ManuallyAssertExt, Path, RustState, State, VecIndexExt};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::state::friends::{FriendListEntry, FriendListEntryPathExt};
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

// TODO: These constants are duplicated troughout the code base. Unify this
// somewhere, maybe a `consts.rs` would be a good idea at this point?
const MINIMUM_NAME_LENGTH: usize = 4;
const MAXIMUM_NAME_LENGTH: usize = 24;
/// Wide enough for a member row (name and HP bar, Kick, Make leader).
const FRIEND_WINDOW_MINIMUM_WIDTH: f32 = 300.0;

struct FriendList<A> {
    friend_list_path: A,
    elements: Vec<ElementBox<ClientState>>,
}

impl<A> FriendList<A> {
    fn new(friend_list_path: A) -> Self {
        Self {
            friend_list_path,
            elements: Vec::new(),
        }
    }
}

impl<A> Element<ClientState> for FriendList<A>
where
    A: Path<ClientState, Vec<FriendListEntry>>,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            use korangar_interface::prelude::*;

            let friend_list = state.get(&self.friend_list_path);

            match friend_list.len().cmp(&self.elements.len()) {
                Ordering::Less => {
                    self.elements.truncate(friend_list.len());
                }
                Ordering::Equal => {}
                Ordering::Greater => {
                    for index in self.elements.len()..friend_list.len() {
                        let friend_path = self.friend_list_path.index(index).manually_asserted();
                        let name_path = friend_path.label();

                        self.elements.push(ErasedElement::new(collapsible! {
                            text: name_path,
                            children: button! {
                                text: client_state().localization().remove_button_text(),
                                event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                    let entry = state.get(&friend_path);

                                    queue.queue(
                                        InputEvent::RemoveFriend {
                                            account_id: entry.account_id,
                                            character_id: entry.character_id,
                                        }
                                    );
                                },
                            },
                        }));
                    }
                }
            }

            self.elements.iter_mut().zip(friend_list.iter()).for_each(|(element, friend)| {
                element.create_layout_info(state, store.child_store(friend.character_id.0 as u64), resolver);
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
        let friend_list = state.get(&self.friend_list_path);

        self.elements.iter().zip(friend_list.iter()).for_each(|(element, friend)| {
            element.lay_out(state, store.child_store(friend.character_id.0 as u64), &(), layout);
        });
    }
}

/// Internal state of the chat window.
#[derive(Default, RustState, StateElement)]
pub struct FriendListWindowState {
    currently_adding: String,
    /// Friends tab. The window opens on the party tab.
    pub show_friends: bool,
    pub create_name: String,
    pub invite_name: String,
    pub create_share_pickup: bool,
    pub create_share_loot: bool,
}

pub struct FriendListWindow<A, B, P> {
    window_state_path: A,
    friend_list_path: B,
    party_path: P,
}

impl<A, B, P> FriendListWindow<A, B, P> {
    pub fn new(window_state_path: A, friend_list_path: B, party_path: P) -> Self {
        Self {
            window_state_path,
            friend_list_path,
            party_path,
        }
    }
}

impl<A, B, P> CustomWindow<ClientState> for FriendListWindow<A, B, P>
where
    A: Path<ClientState, FriendListWindowState> + Clone + 'static,
    B: Path<ClientState, Vec<FriendListEntry>>,
    P: Path<ClientState, crate::state::party::PartyState> + Clone + 'static,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::FriendList)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        struct AddFriendTextBox;

        let add_action = move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
            let character_name = state.get(&self.window_state_path.currently_adding()).clone();

            // TODO: Give some sort of error if the name is too short.
            if character_name.len() >= MINIMUM_NAME_LENGTH {
                state.update_value_with(self.window_state_path.currently_adding(), |input| input.clear());
                queue.queue(InputEvent::AddFriend { character_name });
                queue.queue(Event::Unfocus);
            }
        };

        window! {
            title: "Party / Friends",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            minimum_width: FRIEND_WINDOW_MINIMUM_WIDTH,
            closable: true,
            elements: (
                crate::interface::windows::WindowGroupTabs::new(WindowClass::FriendList),
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Party",
                            event: SetToFalse(self.window_state_path.show_friends()),
                        },
                        button! {
                            text: "Friends",
                            event: SetToTrue(self.window_state_path.show_friends()),
                        },
                    ),
                },
                either! {
                    selector: self.window_state_path.show_friends(),
                    on_true: fragment! {
                        gaps: theme().window().gaps(),
                        children: (
                            text_box! {
                                ghost_text: client_state().localization().friend_list_text_box_message(),
                                state: self.window_state_path.currently_adding(),
                                input_handler: DefaultHandler::<_, _, MAXIMUM_NAME_LENGTH>::new(self.window_state_path.currently_adding(), add_action),
                                focus_id: AddFriendTextBox,
                            },
                            FriendList::new(self.friend_list_path),
                        ),
                    },
                    on_false: super::party::PartyTab::new(self.window_state_path, self.party_path),
                },
            )
        }
    }
}
