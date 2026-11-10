//! Rodex compose window: receiver, title, body and zeny.

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::window::{CustomWindow, Window};
use rust_state::State;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::mail_compose::MailComposeStatePathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

pub struct MailComposeToBox;
pub struct MailComposeTitleBox;
pub struct MailComposeBodyBox;
pub struct MailComposeZenyBox;

pub struct MailComposeWindow;

impl CustomWindow<ClientState> for MailComposeWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::MailCompose)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let compose = client_state().mail_compose();
        let ignore = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Write mail",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            minimum_width: 320.0,
            elements: (
                text! {
                    text: compose.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: "To",
                },
                text_box! {
                    ghost_text: "character name",
                    state: compose.to(),
                    input_handler: DefaultHandler::<_, _, 23>::new(compose.to(), ignore),
                    focus_id: MailComposeToBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: "Title",
                },
                text_box! {
                    ghost_text: "No subject",
                    state: compose.title(),
                    input_handler: DefaultHandler::<_, _, 39>::new(compose.title(), ignore),
                    focus_id: MailComposeTitleBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: "Message",
                },
                text_box! {
                    ghost_text: "message",
                    state: compose.body(),
                    input_handler: DefaultHandler::<_, _, 499>::new(compose.body(), ignore),
                    focus_id: MailComposeBodyBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: "Zeny (2% fee)",
                },
                text_box! {
                    ghost_text: "0",
                    state: compose.zeny_text(),
                    input_handler: DefaultHandler::<_, _, 10>::new(compose.zeny_text(), ignore),
                    focus_id: MailComposeZenyBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Cancel",
                            event: InputEvent::CancelMailCompose,
                        },
                        button! {
                            text: "Send",
                            event: InputEvent::SendMail,
                        },
                    ),
                },
            ),
        }
    }
}
