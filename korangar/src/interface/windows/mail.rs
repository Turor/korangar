use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::StateElement;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::parse_rodex_mail_id;
use rust_state::{RustState, State};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::mail_compose::MailComposeStatePathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Inbox page and the unread-mail flag from `0x09E7`.
#[derive(RustState, StateElement)]
pub struct MailWindowState {
    status: String,
    list_text: String,
    body_text: String,
    page_text: String,
    zeny_text: String,
    attach_text: String,
    mail_id_text: String,
    mail_id_ghost: String,
    new_mail: bool,
    #[hidden_element]
    next_mail_id: i64,
    #[hidden_element]
    page_has_more: bool,
    #[hidden_element]
    open_mail_id: i64,
    #[hidden_element]
    open_zeny: i64,
    #[hidden_element]
    open_items: u8,
}

impl Default for MailWindowState {
    fn default() -> Self {
        Self {
            status: "Open the inbox to load Rodex mail.".to_owned(),
            list_text: String::new(),
            body_text: String::new(),
            page_text: "Page: newest".to_owned(),
            zeny_text: "Zeny: —".to_owned(),
            attach_text: "Attachments: —".to_owned(),
            mail_id_text: String::new(),
            mail_id_ghost: "mail id".to_owned(),
            new_mail: false,
            next_mail_id: 0,
            page_has_more: false,
            open_mail_id: 0,
            open_zeny: 0,
            open_items: 0,
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

    pub fn set_list(&mut self, summary: String, is_end: bool, next_mail_id: i64) {
        self.list_text = summary;
        self.next_mail_id = next_mail_id;
        self.page_has_more = !is_end && next_mail_id != 0;
        self.page_text = if self.page_has_more {
            format!("More mail after #{next_mail_id}.")
        } else if next_mail_id == 0 {
            "Page: newest".to_owned()
        } else {
            format!("Last page (through #{next_mail_id}).")
        };
        self.status = if self.new_mail {
            "New mail is waiting.".to_owned()
        } else {
            "Inbox updated.".to_owned()
        };
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn set_body(&mut self, summary: String) {
        self.body_text = summary.clone();
        self.status = summary;
    }

    pub fn set_read(&mut self, mail_id: i64, zeny: i64, item_count: u8, summary: String) {
        self.open_mail_id = mail_id;
        self.open_zeny = zeny;
        self.open_items = item_count;
        self.mail_id_text = mail_id.to_string();
        self.zeny_text = format!("Zeny: {zeny}");
        self.attach_text = format!("Attachments: {item_count}");
        self.set_body(summary);
    }

    pub fn note_zeny(&mut self, mail_id: i64, result: u8, summary: String) {
        if result == 0 && self.open_mail_id == mail_id {
            self.open_zeny = 0;
            self.zeny_text = "Zeny: 0".to_owned();
        }
        self.status = summary;
    }

    pub fn note_items(&mut self, mail_id: i64, result: u8, summary: String) {
        if result == 0 && self.open_mail_id == mail_id {
            self.open_items = 0;
            self.attach_text = "Attachments: 0".to_owned();
        }
        self.status = summary;
    }

    /// Put `mail_id` in the mail id box.
    pub fn select_mail(&mut self, mail_id: i64) {
        self.mail_id_text = mail_id.to_string();
    }

    pub fn mail_id(&self) -> Result<i64, &'static str> {
        parse_rodex_mail_id(&self.mail_id_text).ok_or("Enter a mail id.")
    }

    /// Cursor for the next `0x0AC1` page. Empty when this page is the last.
    pub fn next_page_id(&self) -> Result<i64, &'static str> {
        if self.page_has_more {
            Ok(self.next_mail_id)
        } else {
            Err("No further mail page.")
        }
    }

    pub fn zeny_request(&self) -> Result<i64, &'static str> {
        let mail_id = self.mail_id()?;
        if self.open_mail_id == mail_id && self.open_zeny > 0 {
            Ok(mail_id)
        } else {
            Err("This mail has no zeny to take.")
        }
    }

    pub fn item_request(&self) -> Result<i64, &'static str> {
        let mail_id = self.mail_id()?;
        if self.open_mail_id == mail_id && self.open_items > 0 {
            Ok(mail_id)
        } else {
            Err("This mail has no items to take.")
        }
    }
}

struct MailIdBox;
struct MailSearchBox;

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
        let compose = client_state().mail_compose();
        let mail_id_action = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Mail",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            // The title-bar close only drops the window. Hercules clears Rodex
            // state on `CZ_CLOSE_MAILBOX`, so close goes through the button.
            closable: false,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.page_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.list_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.body_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.zeny_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.attach_text(),
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
                    text: "Next page",
                    event: InputEvent::NextMailPage,
                },
                button! {
                    text: "Read",
                    event: InputEvent::ReadMail,
                },
                button! {
                    text: "Take zeny",
                    event: InputEvent::TakeMailZeny,
                },
                button! {
                    text: "Take items",
                    event: InputEvent::TakeMailItems,
                },
                button! {
                    text: "Delete",
                    event: InputEvent::DeleteMail,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Write",
                            event: InputEvent::OpenMailCompose,
                        },
                        button! {
                            text: "Reply",
                            event: InputEvent::ReplyMail,
                        },
                    ),
                },
                text_box! {
                    ghost_text: "search sender or title",
                    state: compose.search_text(),
                    input_handler: DefaultHandler::<_, _, 24>::new(compose.search_text(), mail_id_action),
                    focus_id: MailSearchBox,
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Search",
                    event: InputEvent::SearchMail,
                },
                text! {
                    text: compose.search_results(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! {
                    text: "Close mailbox",
                    event: InputEvent::CloseMail,
                },
            ),
        }
    }
}
