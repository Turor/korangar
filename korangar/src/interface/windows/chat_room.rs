//! Chat room windows: the room itself, the create / settings form, and the
//! password prompt for private rooms.

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::State;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::chat_room::ChatRoomStatePathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

pub struct ChatRoomTitleBox;
pub struct ChatRoomLimitBox;
pub struct ChatRoomPasswordBox;
pub struct ChatRoomMemberBox;
pub struct ChatRoomJoinPasswordBox;

/// The room this client is in: heading, members, and owner tools.
pub struct ChatRoomWindow;

impl CustomWindow<ClientState> for ChatRoomWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ChatRoom)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let room = client_state().chat_room();
        let ignore = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Chat room",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            minimum_width: 280.0,
            elements: (
                text! {
                    text: room.heading(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: room.member_lines(),
                },
                text_box! {
                    ghost_text: "member name (owner)",
                    state: room.member_name(),
                    input_handler: DefaultHandler::<_, _, 23>::new(room.member_name(), ignore),
                    focus_id: ChatRoomMemberBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Kick",
                            event: InputEvent::KickChatRoomMember,
                        },
                        button! {
                            text: "Make owner",
                            event: InputEvent::GiveChatRoomOwnership,
                        },
                    ),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Settings",
                            event: InputEvent::OpenChatRoomSettings,
                        },
                        button! {
                            text: "Leave",
                            event: InputEvent::LeaveChatRoom,
                        },
                    ),
                },
            ),
        }
    }
}

/// Create a room, or change the one this client owns.
pub struct ChatRoomCreateWindow;

impl CustomWindow<ClientState> for ChatRoomCreateWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ChatRoomCreate)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let room = client_state().chat_room();
        let ignore = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Chat room settings",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 280.0,
            elements: (
                text! {
                    text: "Title",
                },
                text_box! {
                    ghost_text: "room title",
                    state: room.form_title(),
                    input_handler: DefaultHandler::<_, _, 36>::new(room.form_title(), ignore),
                    focus_id: ChatRoomTitleBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: "Members (2-20)",
                },
                text_box! {
                    ghost_text: "20",
                    state: room.form_limit(),
                    input_handler: DefaultHandler::<_, _, 2>::new(room.form_limit(), ignore),
                    focus_id: ChatRoomLimitBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                state_button! {
                    text: "Public",
                    state: room.form_public(),
                    event: Toggle(room.form_public()),
                },
                text_box! {
                    ghost_text: "password (private rooms)",
                    state: room.form_password(),
                    input_handler: DefaultHandler::<_, _, 7>::new(room.form_password(), ignore),
                    focus_id: ChatRoomPasswordBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Cancel",
                            event: InputEvent::CloseChatRoomSettings,
                        },
                        button! {
                            text: "OK",
                            event: InputEvent::SubmitChatRoomSettings,
                        },
                    ),
                },
            ),
        }
    }
}

/// Password prompt for a private room.
pub struct ChatRoomPasswordWindow;

impl CustomWindow<ClientState> for ChatRoomPasswordWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::ChatRoomPassword)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let room = client_state().chat_room();
        let ignore = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Private chat room",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: room.join_prompt(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: "password",
                    state: room.join_password(),
                    input_handler: DefaultHandler::<_, _, 7>::new(room.join_password(), ignore),
                    focus_id: ChatRoomJoinPasswordBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Cancel",
                            event: InputEvent::CancelChatRoomPassword,
                        },
                        button! {
                            text: "Join",
                            event: InputEvent::ConfirmChatRoomPassword,
                        },
                    ),
                },
            ),
        }
    }
}
