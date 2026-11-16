//! Rodex compose, reply, search and attachment flow. `lib.rs` forwards the
//! input and network events here; the form model is `state::mail_compose`
//! and `state::mail_attach`.

use korangar_networking::{InventoryItemDetails, MessageColor, RodexAttachment, RodexMailSummary};
use ragnarok_packets::{EquipPosition, ItemId};

use crate::Client;
use crate::input::InputEvent;
use crate::interface::windows::{
    MailAttachAmountBox, MailAttachAmountWindow, MailComposeBodyBox, MailComposeToBox, MailComposeWindow, WindowClass,
};
use crate::state::inventory::InventoryPathExt;
use crate::state::mail_attach::{AttachCandidate, AttachStep};
use crate::state::{ChatMessage, ClientStatePathExt, client_state};
use crate::world::{ItemName, ItemNameKey};

impl Client {
    fn mail_chat(&mut self, text: String, color: MessageColor) {
        self.client_state
            .follow_mut(client_state().chat_messages())
            .push(ChatMessage::new(text, color));
    }

    /// Start a mail to `to` (empty for a blank form). The window opens when
    /// the server allows writing (`0x0A12`).
    pub(crate) fn begin_mail_compose(&mut self, to: &str, title: &str) {
        // Cancel first: it runs `rodex_clean`, which drops a form left open
        // server-side and marks the attachment slots free. Hercules only
        // does that on the mailbox, cancel, send and logout, so the first
        // gear attach after login fails with "no space" without it.
        let _ = self.networking_system.cancel_rodex_write();
        self.interface.close_window_with_class(WindowClass::MailAttachAmount);
        self.client_state.follow_mut(client_state().mail_compose()).begin(to, title);
        let _ = self.networking_system.open_rodex_write(to);
    }

    /// Once per frame after the interface events. A write form the player
    /// closed (title bar or Escape) is cancelled so Hercules releases it
    /// (`workinprogress & 2` blocks NPC clicks), an amount prompt that was
    /// closed is dropped, and the fee follows the zeny box.
    pub(crate) fn sync_mail_compose(&mut self) {
        let compose_open = self.interface.is_window_with_class_open(WindowClass::MailCompose);
        let prompt_open = self.interface.is_window_with_class_open(WindowClass::MailAttachAmount);
        let compose = self.client_state.follow(client_state().mail_compose());
        let cancel = !compose_open && compose.needs_cancel_on_close();
        let drop_prompt = compose.attachments.is_asking() && (!prompt_open || !compose_open);
        let refresh = compose_open && compose.fee_is_stale();
        if cancel {
            self.client_state.follow_mut(client_state().mail_compose()).cancel();
            let _ = self.networking_system.cancel_rodex_write();
        }
        if drop_prompt || cancel {
            self.client_state
                .follow_mut(client_state().mail_compose())
                .attachments
                .cancel_amount();
            if prompt_open {
                self.interface.close_window_with_class(WindowClass::MailAttachAmount);
            }
        }
        if refresh {
            self.client_state.follow_mut(client_state().mail_compose()).refresh_fee();
        }
    }

    fn attach_candidate(&self, index: u16) -> Option<AttachCandidate> {
        let items = self.client_state.follow(client_state().inventory().items());
        let item = items.iter().find(|item| item.index.0 == index)?;
        let (amount, equipped) = match &item.details {
            InventoryItemDetails::Regular {
                amount, equipped_position, ..
            } => (*amount, *equipped_position != EquipPosition::NONE),
            InventoryItemDetails::Equippable { equipped_position, .. } => (1u16, *equipped_position != EquipPosition::NONE),
        };
        let name = match item.metadata.name.trim() {
            "" => format!("item {}", item.item_id.0),
            name => name.to_owned(),
        };
        Some(AttachCandidate {
            index,
            item_id: item.item_id.0,
            name,
            texture: item.metadata.texture.clone(),
            amount,
            equipped,
        })
    }

    fn mail_compose_status(&mut self, text: String) {
        self.client_state.follow_mut(client_state().mail_compose()).status = text;
    }

    fn send_attach(&mut self, index: u16, count: u16) {
        if self.networking_system.add_rodex_item(index, count).is_err() {
            let compose = self.client_state.follow_mut(client_state().mail_compose());
            compose.attachments.attached(2, None, 0, 0, 0);
            compose.status = "Not connected to the map server.".to_owned();
        }
    }

    /// An inventory stack dropped on the form, or the autopilot form with an
    /// explicit amount.
    fn attach_mail_item(&mut self, index: u16, amount: Option<u16>) {
        let Some(candidate) = self.attach_candidate(index) else {
            self.mail_compose_status("That item is no longer in your inventory.".to_owned());
            return;
        };
        let step = {
            let compose = self.client_state.follow_mut(client_state().mail_compose());
            let write_open = compose.write_open();
            compose.attachments.offer(write_open, candidate)
        };
        match (step, amount) {
            (AttachStep::Send { index, count }, _) => self.send_attach(index, count),
            (AttachStep::AskAmount, Some(amount)) => {
                self.client_state.follow_mut(client_state().mail_compose()).attachments.amount_text = amount.to_string();
                self.confirm_mail_attach_amount();
            }
            (AttachStep::AskAmount, None) => {
                if !self.interface.is_window_with_class_open(WindowClass::MailAttachAmount) {
                    self.interface.open_window(MailAttachAmountWindow);
                }
                self.interface.request_focus(MailAttachAmountBox);
            }
            (AttachStep::Rejected(message), _) => {
                self.mail_compose_status(message.clone());
                self.mail_chat(message, MessageColor::Error);
            }
        }
    }

    fn confirm_mail_attach_amount(&mut self) {
        let confirmed = self
            .client_state
            .follow_mut(client_state().mail_compose())
            .attachments
            .confirm_amount();
        match confirmed {
            Ok((index, count)) => {
                self.interface.close_window_with_class(WindowClass::MailAttachAmount);
                self.send_attach(index, count);
            }
            Err(message) => self.mail_compose_status(message),
        }
    }

    pub(crate) fn on_rodex_item_attached(&mut self, result: u8, index: Option<u16>, count: u16, item_id: u32, weight: u16) {
        let line = {
            let compose = self.client_state.follow_mut(client_state().mail_compose());
            let line = compose.attachments.attached(result, index, count, item_id, weight);
            compose.attachments_changed();
            line
        };
        if let Some(line) = line {
            self.mail_compose_status(line.clone());
            let color = if result == 0 {
                MessageColor::Information
            } else {
                MessageColor::Error
            };
            self.mail_chat(line, color);
        }
    }

    pub(crate) fn on_rodex_item_detached(&mut self, removed: bool, index: Option<u16>, count: u16, weight: u16) {
        let line = {
            let compose = self.client_state.follow_mut(client_state().mail_compose());
            let line = compose.attachments.detached(removed, index, count, weight);
            compose.attachments_changed();
            line
        };
        if let Some(line) = line {
            self.mail_compose_status(line.clone());
            let color = if removed { MessageColor::Information } else { MessageColor::Error };
            self.mail_chat(line, color);
        }
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
                self.interface.close_window_with_class(WindowClass::MailAttachAmount);
                self.interface.close_window_with_class(WindowClass::MailCompose);
            }
            InputEvent::MailAttachItem { index, amount } => self.attach_mail_item(index, amount),
            InputEvent::ConfirmMailAttachAmount => self.confirm_mail_attach_amount(),
            InputEvent::CancelMailAttachAmount => {
                self.client_state
                    .follow_mut(client_state().mail_compose())
                    .attachments
                    .cancel_amount();
                self.interface.close_window_with_class(WindowClass::MailAttachAmount);
            }
            InputEvent::MailDetachItem { index } => {
                let request = self
                    .client_state
                    .follow_mut(client_state().mail_compose())
                    .attachments
                    .detach(index);
                match request {
                    Ok((index, count)) => {
                        if self.networking_system.remove_rodex_item(index, count).is_err() {
                            self.client_state
                                .follow_mut(client_state().mail_compose())
                                .attachments
                                .detached(false, None, 0, 0);
                        }
                    }
                    Err(message) => self.mail_compose_status(message),
                }
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

    pub(crate) fn on_rodex_receiver(&mut self, character_id: u32, name: &str) {
        let outcome = self
            .client_state
            .follow_mut(client_state().mail_compose())
            .receiver_checked(character_id, name);
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
        self.interface.close_window_with_class(WindowClass::MailAttachAmount);
        if close {
            self.interface.close_window_with_class(WindowClass::MailCompose);
            self.mail_chat(status, MessageColor::Information);
        } else {
            self.mail_chat(status, MessageColor::Error);
            // Hercules closed the form; open it again so the typed mail can be
            // retried. The window stays up and shows why.
            if self.interface.is_window_with_class_open(WindowClass::MailCompose) {
                let to = self.client_state.follow(client_state().mail_compose()).to.clone();
                self.client_state.follow_mut(client_state().mail_compose()).reopen();
                let _ = self.networking_system.open_rodex_write(&to);
            }
        }
    }

    /// Item names for the attachments of a read mail.
    pub(crate) fn on_rodex_read_attachments(&mut self, attachments: &[RodexAttachment]) {
        let names = attachments
            .iter()
            .map(|attachment| {
                let name = self
                    .library
                    .get::<ItemName>(ItemNameKey {
                        item_id: ItemId(attachment.item_id),
                        is_identified: true,
                    })
                    .to_string();
                let name = if name == "NOTFOUND" || name.trim().is_empty() {
                    format!("item {}", attachment.item_id)
                } else {
                    name
                };
                format!("{name} x{}", attachment.amount)
            })
            .collect::<Vec<_>>()
            .join(", ");
        self.client_state
            .follow_mut(client_state().mail_window())
            .set_attachment_names(&names);
    }

    pub(crate) fn on_rodex_page(&mut self, mails: Vec<RodexMailSummary>) {
        self.client_state.follow_mut(client_state().mail_compose()).set_mails(mails);
    }
}
