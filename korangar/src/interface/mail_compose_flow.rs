//! Rodex compose, reply and search flow. `lib.rs` forwards the input and
//! network events here; the form model is `state::mail_compose`.

use korangar_networking::{MessageColor, RodexMailSummary};

use crate::Client;
use crate::input::InputEvent;
use crate::interface::windows::{MailComposeBodyBox, MailComposeToBox, MailComposeWindow, WindowClass};
use crate::state::{ChatMessage, ClientStatePathExt, client_state};

impl Client {
    fn mail_chat(&mut self, text: String, color: MessageColor) {
        self.client_state
            .follow_mut(client_state().chat_messages())
            .push(ChatMessage::new(text, color));
    }

    /// Start a mail to `to` (empty for a blank form). The window opens when
    /// the server allows writing (`0x0A12`).
    pub(crate) fn begin_mail_compose(&mut self, to: &str, title: &str) {
        self.client_state.follow_mut(client_state().mail_compose()).begin(to, title);
        let _ = self.networking_system.open_rodex_write(to);
    }

    pub(crate) fn on_mail_compose_input(&mut self, event: InputEvent) {
        match event {
            InputEvent::OpenMailCompose => self.begin_mail_compose("", ""),
            InputEvent::ComposeMailTo { receiver } => self.begin_mail_compose(&receiver, ""),
            InputEvent::ReadMailId { mail_id } => {
                self.client_state.follow_mut(client_state().mail_window()).select_mail(mail_id);
                self.input_event_buffer.push(InputEvent::ReadMail);
            }
            InputEvent::ReplyMail => {
                let reply = self
                    .client_state
                    .follow(client_state().mail_window())
                    .mail_id()
                    .ok()
                    .and_then(|mail_id| self.client_state.follow(client_state().mail_compose()).reply_to(mail_id));
                match reply {
                    Some((sender, title)) => self.begin_mail_compose(&sender, &title),
                    None => self
                        .client_state
                        .follow_mut(client_state().mail_window())
                        .set_status("Read a mail from this page first, then reply.".to_owned()),
                }
            }
            InputEvent::SearchMail => self.client_state.follow_mut(client_state().mail_compose()).search(),
            InputEvent::SendMail => {
                let check = self.client_state.follow_mut(client_state().mail_compose()).start_send();
                match check {
                    Ok(name) => {
                        let _ = self.networking_system.check_rodex_receiver(&name);
                    }
                    Err(message) => self.client_state.follow_mut(client_state().mail_compose()).status = message,
                }
            }
            InputEvent::CancelMailCompose => {
                self.client_state.follow_mut(client_state().mail_compose()).cancel();
                let _ = self.networking_system.cancel_rodex_write();
                self.interface.close_window_with_class(WindowClass::MailCompose);
            }
            _ => {}
        }
    }

    pub(crate) fn on_rodex_write_opened(&mut self, allowed: bool) {
        let (show, addressed) = {
            let compose = self.client_state.follow_mut(client_state().mail_compose());
            let show = compose.opened(allowed);
            (show, !compose.to.is_empty())
        };
        if show {
            if !self.interface.is_window_with_class_open(WindowClass::MailCompose) {
                self.interface.open_window(MailComposeWindow);
            }
            match addressed {
                true => self.interface.request_focus(MailComposeBodyBox),
                false => self.interface.request_focus(MailComposeToBox),
            }
        } else if !allowed {
            let status = self.client_state.follow(client_state().mail_compose()).status.clone();
            self.mail_chat(status, MessageColor::Error);
        }
    }

    pub(crate) fn on_rodex_receiver(&mut self, character_id: u32) {
        let outcome = self
            .client_state
            .follow_mut(client_state().mail_compose())
            .receiver_checked(character_id);
        match outcome {
            Some(Ok(request)) => {
                let sender = self.client_state.follow(client_state().player_name()).clone();
                let _ = self.networking_system.send_rodex_mail(
                    &request.receiver,
                    &sender,
                    request.character_id,
                    request.zeny,
                    &request.title,
                    &request.body,
                );
            }
            Some(Err(message)) => self.mail_chat(message, MessageColor::Error),
            None => {}
        }
    }

    /// `0x09ED` for a compose this client started.
    pub(crate) fn on_rodex_send_result(&mut self, result: u8) {
        let close = self.client_state.follow_mut(client_state().mail_compose()).send_result(result);
        let Some(close) = close else {
            return;
        };
        let status = self.client_state.follow(client_state().mail_compose()).status.clone();
        if close {
            self.interface.close_window_with_class(WindowClass::MailCompose);
            self.mail_chat(status, MessageColor::Information);
        } else {
            self.mail_chat(status, MessageColor::Error);
        }
    }

    pub(crate) fn on_rodex_page(&mut self, mails: Vec<RodexMailSummary>) {
        self.client_state.follow_mut(client_state().mail_compose()).set_mails(mails);
    }
}
