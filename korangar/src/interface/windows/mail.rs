use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::StateElement;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::parse_rodex_mail_id;
use rust_state::{RustState, State};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Inbox page and the unread-mail flag from `0x09E7`.
#[derive(RustState, StateElement)]
pub struct MailWindowState {
    status: String,
    list_text: String,
    mail_id_text: String,
    mail_id_ghost: String,
    new_mail: bool,
}

impl Default for MailWindowState {
    fn default() -> Self {
        Self {
            status: "Open the inbox to load Rodex mail.".to_owned(),
            list_text: String::new(),
            mail_id_text: String::new(),
            mail_id_ghost: "mail id".to_owned(),
            new_mail: false,
        }
    }
}

impl MailWindowState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn set_new_mail(&mut self, available: bool) {
        self.new_mail = available;
        if available {
            self.status = "New mail is waiting.".to_owned();
        } else if self.list_text.is_empty() {
            self.status = "No unread mail.".to_owned();
        }
    }

    pub fn set_list(&mut self, summary: String) {
        self.list_text = summary;
        self.status = if self.new_mail {
            "New mail is waiting.".to_owned()
        } else {
            "Inbox updated.".to_owned()
        };
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn mail_id(&self) -> Result<i64, &'static str> {
        parse_rodex_mail_id(&self.mail_id_text).ok_or("Enter a mail id.")
    }
}

struct MailIdBox;

pub struct MailWindow;

impl MailWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for MailWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Mail)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().mail_window();
        let mail_id_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Mail",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.list_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text_box! {
                    ghost_text: state_path.mail_id_ghost(),
                    state: state_path.mail_id_text(),
                    input_handler: DefaultHandler::<_, _, 16>::new(state_path.mail_id_text(), mail_id_action),
                    focus_id: MailIdBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Open inbox",
                    event: InputEvent::RefreshMail,
                },
                button! {
                    text: "Read",
                    event: InputEvent::ReadMail,
                },
                button! {
                    text: "Delete",
                    event: InputEvent::DeleteMail,
                },
                button! {
                    text: "Close mailbox",
                    event: InputEvent::CloseMail,
                },
            ),
        }
    }
}
