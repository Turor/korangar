//! Rodex compose, reply and inbox search.
//!
//! Sending is three steps on Hercules: open the write form (`0x0A08` /
//! `0x0A12`), check the receiver (`0x0B97` / `0x0A51`, which also arms the
//! server-side receiver), then send (`0x0A6E` / `0x09ED`).

use korangar_interface::element::StateElement;
use korangar_networking::RodexMailSummary;
use rust_state::RustState;

/// Longest character name Hercules accepts (`NAME_LENGTH` minus the NUL).
pub const RECEIVER_MAX: usize = 23;
/// Title used when the player leaves it empty.
pub const DEFAULT_TITLE: &str = "No subject";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ComposePhase {
    #[default]
    Idle,
    /// Waiting for `0x0A12`.
    Opening,
    /// Waiting for `0x0A51` before the send.
    Checking,
    /// Waiting for `0x09ED`.
    Sending,
}

/// What the client should send next after a receiver check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendRequest {
    pub receiver: String,
    pub character_id: u32,
    pub title: String,
    pub body: String,
    pub zeny: i64,
}

#[derive(Debug, Clone, Default, RustState, StateElement)]
pub struct MailComposeState {
    pub to: String,
    pub title: String,
    pub body: String,
    pub zeny_text: String,
    pub status: String,
    /// Inbox search query and its result lines.
    pub search_text: String,
    pub search_results: String,
    #[hidden_element]
    phase: ComposePhase,
    #[hidden_element]
    mails: Vec<RodexMailSummary>,
}

impl MailComposeState {
    #[cfg(test)]
    pub fn phase(&self) -> ComposePhase {
        self.phase
    }

    /// A fresh form, optionally addressed (context menu or reply).
    pub fn begin(&mut self, to: &str, title: &str) {
        self.to = to.trim().to_owned();
        self.title = title.to_owned();
        self.body.clear();
        self.zeny_text.clear();
        self.status = "Waiting for the mailbox...".to_owned();
        self.phase = ComposePhase::Opening;
    }

    /// `0x0A12`. Returns whether the compose window should show.
    pub fn opened(&mut self, allowed: bool) -> bool {
        if self.phase != ComposePhase::Opening {
            return false;
        }
        if allowed {
            self.phase = ComposePhase::Idle;
            self.status = "Write your mail.".to_owned();
        } else {
            self.phase = ComposePhase::Idle;
            self.status = "Mail cannot be written right now (talking to an NPC?).".to_owned();
        }
        allowed
    }

    /// Validate the form and start the receiver check. Returns the name to
    /// check.
    pub fn start_send(&mut self) -> Result<String, String> {
        if self.phase == ComposePhase::Checking || self.phase == ComposePhase::Sending {
            return Err("The mail is already on its way.".to_owned());
        }
        let to = self.to.trim();
        if to.is_empty() {
            return Err("Enter who the mail is for.".to_owned());
        }
        if to.len() > RECEIVER_MAX {
            return Err(format!("A character name has at most {RECEIVER_MAX} letters."));
        }
        self.zeny()?;
        self.phase = ComposePhase::Checking;
        self.status = format!("Looking up {to}...");
        Ok(to.to_owned())
    }

    fn zeny(&self) -> Result<i64, String> {
        let text = self.zeny_text.trim();
        if text.is_empty() {
            return Ok(0);
        }
        text.parse::<i64>()
            .ok()
            .filter(|zeny| *zeny >= 0)
            .ok_or_else(|| "Zeny must be a whole number, 0 or more.".to_owned())
    }

    /// `0x0A51` while checking: the send to make, or why not.
    pub fn receiver_checked(&mut self, character_id: u32) -> Option<Result<SendRequest, String>> {
        if self.phase != ComposePhase::Checking {
            return None;
        }
        let to = self.to.trim().to_owned();
        if character_id == 0 {
            self.phase = ComposePhase::Idle;
            let message = format!("There is no character named {to}.");
            self.status = message.clone();
            return Some(Err(message));
        }
        // Hercules compares the send against the name it stored from the
        // check, which is the one typed here.
        let title = match self.title.trim() {
            "" => DEFAULT_TITLE.to_owned(),
            title => title.to_owned(),
        };
        self.phase = ComposePhase::Sending;
        self.status = format!("Sending to {to}...");
        Some(Ok(SendRequest {
            receiver: to,
            character_id,
            title,
            body: self.body.clone(),
            zeny: self.zeny().unwrap_or(0),
        }))
    }

    /// `0x09ED` while sending. `Some(true)` when the compose window should
    /// close.
    pub fn send_result(&mut self, result: u8) -> Option<bool> {
        if self.phase != ComposePhase::Sending {
            return None;
        }
        self.phase = ComposePhase::Idle;
        self.status = send_result_message(result, self.to.trim());
        Some(result == 0)
    }

    pub fn cancel(&mut self) {
        self.phase = ComposePhase::Idle;
        self.status.clear();
    }

    pub fn set_mails(&mut self, mails: Vec<RodexMailSummary>) {
        self.mails = mails;
        self.search();
    }

    /// Sender and `Re:` title for a reply to `mail_id`.
    pub fn reply_to(&self, mail_id: i64) -> Option<(String, String)> {
        let mail = self.mails.iter().find(|mail| mail.mail_id == mail_id)?;
        let title = match mail.title.starts_with("Re: ") {
            true => mail.title.clone(),
            false => format!("Re: {}", mail.title),
        };
        Some((mail.sender.clone(), title))
    }

    /// Filter the loaded page by sender or title, case-insensitively. An
    /// empty query lists every mail.
    pub fn search(&mut self) {
        let query = self.search_text.trim().to_lowercase();
        let lines: Vec<String> = self
            .mails
            .iter()
            .filter(|mail| query.is_empty() || mail.sender.to_lowercase().contains(&query) || mail.title.to_lowercase().contains(&query))
            .map(|mail| {
                let unread = if mail.is_read { "" } else { " [unread]" };
                format!("#{} from {}: {}{unread}", mail.mail_id, mail.sender, mail.title)
            })
            .collect();
        self.search_results = match (lines.is_empty(), self.mails.is_empty()) {
            (_, true) => "Open the inbox to search it.".to_owned(),
            (true, false) => format!("No mail matches \"{}\".", self.search_text.trim()),
            (false, false) => lines.join("\n"),
        };
    }
}

/// `ZC_ACK_WRITE_MAIL` results (Hercules `rodex_send_mail_result`).
pub fn send_result_message(result: u8, to: &str) -> String {
    match result {
        0 => format!("Mail sent to {to}."),
        2 => "You cannot send more mail today.".to_owned(),
        3 => "An attached item could not be sent.".to_owned(),
        4 => format!("{to} cannot receive this mail. Check the name and try again."),
        _ => "The mail could not be sent.".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mail(mail_id: i64, sender: &str, title: &str, is_read: bool) -> RodexMailSummary {
        RodexMailSummary {
            mail_id,
            sender: sender.to_owned(),
            title: title.to_owned(),
            is_read,
        }
    }

    #[test]
    fn a_mail_goes_open_check_send() {
        let mut state = MailComposeState::default();
        state.begin("Bob", "");
        assert!(!state.opened(false));
        assert_eq!(state.phase(), ComposePhase::Idle);
        assert!(!state.opened(true), "no open pending");

        state.begin("Bob", "");
        assert!(state.opened(true));
        state.body = "Hello".to_owned();
        state.zeny_text = "25".to_owned();
        assert_eq!(state.start_send(), Ok("Bob".to_owned()));
        assert!(state.start_send().is_err(), "a second send waits");

        let request = state.receiver_checked(150001).unwrap().unwrap();
        assert_eq!(request, SendRequest {
            receiver: "Bob".to_owned(),
            character_id: 150001,
            title: DEFAULT_TITLE.to_owned(),
            body: "Hello".to_owned(),
            zeny: 25,
        });
        assert_eq!(state.send_result(0), Some(true));
        assert_eq!(state.status, "Mail sent to Bob.");
        assert_eq!(state.send_result(0), None, "only one result per send");
    }

    #[test]
    fn the_form_is_checked_before_anything_is_sent() {
        let mut state = MailComposeState::default();
        state.begin("", "");
        state.opened(true);
        assert!(state.start_send().unwrap_err().contains("who"));
        state.to = "A".repeat(24);
        assert!(state.start_send().unwrap_err().contains("23"));
        state.to = "Bob".to_owned();
        state.zeny_text = "-5".to_owned();
        assert!(state.start_send().unwrap_err().contains("Zeny"));
        state.zeny_text = "abc".to_owned();
        assert!(state.start_send().is_err());
        assert_eq!(state.phase(), ComposePhase::Idle);
    }

    #[test]
    fn unknown_receivers_and_failed_sends_explain_themselves() {
        let mut state = MailComposeState::default();
        state.begin("Nobody", "Hi");
        state.opened(true);
        state.start_send().unwrap();
        assert!(
            state
                .receiver_checked(0)
                .unwrap()
                .unwrap_err()
                .contains("no character named Nobody")
        );
        assert_eq!(state.receiver_checked(1), None, "no check pending");

        state.start_send().unwrap();
        state.receiver_checked(7).unwrap().unwrap();
        assert_eq!(state.send_result(2), Some(false));
        assert!(state.status.contains("today"));
    }

    #[test]
    fn replies_take_the_sender_and_one_re_prefix() {
        let mut state = MailComposeState::default();
        state.set_mails(vec![mail(1, "Kafra", "Welcome", false), mail(2, "Bob", "Re: Party", true)]);
        assert_eq!(state.reply_to(1), Some(("Kafra".to_owned(), "Re: Welcome".to_owned())));
        assert_eq!(state.reply_to(2), Some(("Bob".to_owned(), "Re: Party".to_owned())));
        assert_eq!(state.reply_to(3), None);
    }

    #[test]
    fn search_matches_sender_or_title_ignoring_case() {
        let mut state = MailComposeState::default();
        state.search();
        assert!(state.search_results.contains("Open the inbox"));
        state.set_mails(vec![mail(1, "Kafra", "Welcome", false), mail(2, "Bob", "Party tonight", true)]);
        assert_eq!(state.search_results.lines().count(), 2);
        state.search_text = "kaf".to_owned();
        state.search();
        assert_eq!(state.search_results, "#1 from Kafra: Welcome [unread]");
        state.search_text = "PARTY".to_owned();
        state.search();
        assert_eq!(state.search_results, "#2 from Bob: Party tonight");
        state.search_text = "zzz".to_owned();
        state.search();
        assert!(state.search_results.contains("No mail matches"));
    }
}
