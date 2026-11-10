//! Chat rooms: boards over room owners, the room this client is in, and the
//! create and password forms. Modeled on roBrowser `ChatRoom`,
//! `ChatRoomCreate` and `EntityRoom`.

use korangar_interface::element::StateElement;
use korangar_networking::{CHAT_ROOM_LIMITS, ChatRoomMember};
use ragnarok_packets::{AccountId, ChatRoomId};
use rust_state::RustState;

/// A room title board hanging over its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRoomBoard {
    pub owner_id: AccountId,
    pub chat_id: ChatRoomId,
    pub limit: u16,
    pub users: u16,
    pub public: bool,
    pub title: String,
}

impl ChatRoomBoard {
    /// `Title (3/20)`, with a lock mark for password rooms.
    pub fn label(&self) -> String {
        let lock = if self.public { "" } else { "[P] " };
        format!("{lock}{} ({}/{})", self.title, self.users, self.limit)
    }
}

/// Id of this client's own room until the server names it.
pub const OWN_ROOM_PLACEHOLDER: ChatRoomId = ChatRoomId(0);

/// What the client should do after a member left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberLeft {
    /// Someone else left; the list was updated.
    Other,
    /// This character left or was kicked; the room window closes.
    Myself { kicked: bool },
}

#[derive(Debug, Clone, Default, RustState, StateElement)]
pub struct ChatRoomState {
    #[hidden_element]
    boards: Vec<ChatRoomBoard>,
    #[hidden_element]
    chat_id: Option<ChatRoomId>,
    #[hidden_element]
    members: Vec<ChatRoomMember>,
    /// Header line of the room window.
    pub heading: String,
    /// One member per line, owner first and marked.
    pub member_lines: String,
    /// Name typed into the owner's Kick / Make owner box.
    pub member_name: String,
    // Create / settings form.
    pub form_title: String,
    pub form_limit: String,
    pub form_password: String,
    pub form_public: bool,
    /// The form edits the current room instead of creating one.
    #[hidden_element]
    form_changes_room: bool,
    // Password prompt.
    #[hidden_element]
    pending_create: Option<(String, u16, bool)>,
    #[hidden_element]
    pending_join: Option<ChatRoomId>,
    pub join_password: String,
    pub join_prompt: String,
}

impl ChatRoomState {
    pub fn boards(&self) -> &[ChatRoomBoard] {
        &self.boards
    }

    pub fn is_in_room(&self) -> bool {
        self.chat_id.is_some()
    }

    #[cfg(test)]
    pub fn members(&self) -> &[ChatRoomMember] {
        &self.members
    }

    pub fn is_owner(&self, my_name: &str) -> bool {
        self.members.iter().any(|member| member.owner && member.name == my_name)
    }

    /// One board per owner. A board for this client's own room replaces the
    /// local placeholder and gives the room its server id.
    pub fn upsert_board(&mut self, board: ChatRoomBoard) {
        match self
            .boards
            .iter_mut()
            .find(|existing| existing.chat_id == board.chat_id || existing.owner_id == board.owner_id)
        {
            Some(existing) => {
                if self.chat_id == Some(existing.chat_id) {
                    self.chat_id = Some(board.chat_id);
                }
                *existing = board;
            }
            None => self.boards.push(board),
        }
        self.refresh_heading();
    }

    /// Remember the form values until the server answers the create request.
    pub fn set_pending_create(&mut self, title: String, limit: u16, public: bool) {
        self.pending_create = Some((title, limit, public));
    }

    /// The server created the room this client asked for. Its id arrives
    /// later (if at all), so a placeholder id stands in until then.
    pub fn created(&mut self, my_account: AccountId, my_name: &str) -> bool {
        let Some((title, limit, public)) = self.pending_create.take() else {
            return false;
        };
        self.upsert_board(ChatRoomBoard {
            owner_id: my_account,
            chat_id: OWN_ROOM_PLACEHOLDER,
            limit,
            users: 1,
            public,
            title,
        });
        self.entered(OWN_ROOM_PLACEHOLDER, vec![ChatRoomMember {
            name: my_name.to_owned(),
            owner: true,
        }]);
        true
    }

    pub fn create_failed(&mut self) {
        self.pending_create = None;
    }

    pub fn remove_board(&mut self, chat_id: ChatRoomId) {
        self.boards.retain(|board| board.chat_id != chat_id);
    }

    pub fn board(&self, chat_id: ChatRoomId) -> Option<&ChatRoomBoard> {
        self.boards.iter().find(|board| board.chat_id == chat_id)
    }

    /// Boards of players that left view or the map.
    pub fn forget_owner(&mut self, owner_id: AccountId) {
        self.boards.retain(|board| board.owner_id != owner_id);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn entered(&mut self, chat_id: ChatRoomId, members: Vec<ChatRoomMember>) {
        self.chat_id = Some(chat_id);
        self.members = members;
        self.pending_join = None;
        self.refresh();
    }

    pub fn member_joined(&mut self, name: String, users: u16) {
        if !self.members.iter().any(|member| member.name == name) {
            self.members.push(ChatRoomMember { name, owner: false });
        }
        self.set_users(users);
        self.refresh();
    }

    pub fn member_left(&mut self, name: &str, users: u16, kicked: bool, my_name: &str, my_account: AccountId) -> MemberLeft {
        if name == my_name {
            self.forget_owner(my_account);
            self.leave_room();
            return MemberLeft::Myself { kicked };
        }
        self.members.retain(|member| member.name != name);
        self.set_users(users);
        self.refresh();
        MemberLeft::Other
    }

    pub fn role_changed(&mut self, name: &str, owner: bool) {
        for member in &mut self.members {
            if member.name == name {
                member.owner = owner;
            } else if owner {
                member.owner = false;
            }
        }
        self.refresh();
    }

    /// Drop the current room locally (left, kicked, or map change).
    pub fn leave_room(&mut self) {
        self.chat_id = None;
        self.members.clear();
        self.member_name.clear();
        self.refresh();
    }

    fn set_users(&mut self, users: u16) {
        if let Some(chat_id) = self.chat_id
            && let Some(board) = self.boards.iter_mut().find(|board| board.chat_id == chat_id)
        {
            board.users = users;
        }
    }

    fn refresh(&mut self) {
        self.refresh_heading();
        let mut members: Vec<&ChatRoomMember> = self.members.iter().collect();
        members.sort_by_key(|member| !member.owner);
        self.member_lines = members
            .iter()
            .map(|member| match member.owner {
                true => format!("{} (owner)", member.name),
                false => member.name.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n");
    }

    fn refresh_heading(&mut self) {
        let Some(chat_id) = self.chat_id else {
            self.heading.clear();
            return;
        };
        self.heading = match self.boards.iter().find(|board| board.chat_id == chat_id) {
            Some(board) => board.label(),
            None => format!("Chat room ({} members)", self.members.len()),
        };
    }

    /// Fresh create form. `my_name` seeds a title so one click creates a room.
    pub fn begin_create(&mut self, my_name: &str) {
        self.form_changes_room = false;
        self.form_title = format!("{my_name}'s room");
        self.form_limit = CHAT_ROOM_LIMITS.end().to_string();
        self.form_password.clear();
        self.form_public = true;
    }

    /// Settings form for the room this client owns.
    pub fn begin_change(&mut self) {
        let board = self.chat_id.and_then(|chat_id| self.board(chat_id).cloned());
        self.form_changes_room = true;
        if let Some(board) = board {
            self.form_title = board.title;
            self.form_limit = board.limit.to_string();
            self.form_public = board.public;
        }
        self.form_password.clear();
    }

    pub fn form_changes_room(&self) -> bool {
        self.form_changes_room
    }

    /// Title, limit, public flag and password, or the reason the form is not
    /// ready.
    pub fn form_request(&self) -> Result<(String, u16, bool, String), String> {
        let title = self.form_title.trim();
        if title.is_empty() {
            return Err("Enter a room title.".to_owned());
        }
        let limit = self
            .form_limit
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|limit| CHAT_ROOM_LIMITS.contains(limit))
            .ok_or_else(|| {
                format!(
                    "Enter a member limit from {} to {}.",
                    CHAT_ROOM_LIMITS.start(),
                    CHAT_ROOM_LIMITS.end()
                )
            })?;
        let password = self.form_password.trim();
        if !self.form_public && password.is_empty() {
            return Err("A private room needs a password.".to_owned());
        }
        Ok((title.to_owned(), limit, self.form_public, password.to_owned()))
    }

    /// A click on a board: public rooms join at once, private rooms ask for a
    /// password first. `None` means nothing to send yet.
    pub fn click_board(&mut self, chat_id: ChatRoomId) -> Option<ChatRoomId> {
        let board = self.board(chat_id)?.clone();
        if self.chat_id == Some(chat_id) {
            return None;
        }
        if board.public {
            return Some(chat_id);
        }
        self.pending_join = Some(chat_id);
        self.join_password.clear();
        self.join_prompt = format!("\"{}\" needs a password.", board.title);
        None
    }

    pub fn has_pending_join(&self) -> bool {
        self.pending_join.is_some()
    }

    pub fn take_pending_join(&mut self) -> Option<(ChatRoomId, String)> {
        let chat_id = self.pending_join.take()?;
        Some((chat_id, std::mem::take(&mut self.join_password)))
    }

    pub fn cancel_join(&mut self) {
        self.pending_join = None;
        self.join_password.clear();
    }
}

/// Chat text for `ZC_ACK_CREATE_CHATROOM`, or `None` on success.
pub fn create_result_message(result: u8) -> Option<&'static str> {
    match result {
        0 => None,
        1 => Some("The chat room could not be created: too many rooms."),
        2 => Some("You are already in a chat room."),
        _ => Some("The chat room could not be created."),
    }
}

/// Chat text for `ZC_REFUSE_ENTER_ROOM`.
pub fn refused_message(result: u8) -> &'static str {
    match result {
        0 => "The chat room is full.",
        1 => "Wrong chat room password.",
        2 => "You were kicked from this chat room.",
        4 => "You do not have enough zeny to enter.",
        5 => "Your level is too low for this chat room.",
        6 => "Your level is too high for this chat room.",
        7 => "Your job cannot enter this chat room.",
        _ => "You cannot enter this chat room.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(chat_id: u32, public: bool) -> ChatRoomBoard {
        ChatRoomBoard {
            owner_id: AccountId(2000000 + chat_id),
            chat_id: ChatRoomId(chat_id),
            limit: 20,
            users: 1,
            public,
            title: format!("Room {chat_id}"),
        }
    }

    fn member(name: &str, owner: bool) -> ChatRoomMember {
        ChatRoomMember {
            name: name.to_owned(),
            owner,
        }
    }

    #[test]
    fn boards_update_in_place_and_disappear() {
        let mut state = ChatRoomState::default();
        state.upsert_board(board(1, true));
        let mut changed = board(1, false);
        changed.users = 4;
        state.upsert_board(changed);
        assert_eq!(state.boards().len(), 1);
        assert_eq!(state.boards()[0].label(), "[P] Room 1 (4/20)");
        state.remove_board(ChatRoomId(1));
        assert!(state.boards().is_empty());
    }

    #[test]
    fn members_join_leave_and_change_owner() {
        let mut state = ChatRoomState::default();
        state.upsert_board(board(7, true));
        state.entered(ChatRoomId(7), vec![member("Owner", true), member("Me", false)]);
        assert!(state.is_in_room());
        assert!(!state.is_owner("Me"));
        assert_eq!(state.member_lines, "Owner (owner)\nMe");

        state.member_joined("Guest".to_owned(), 3);
        assert_eq!(state.board(ChatRoomId(7)).unwrap().users, 3);
        assert_eq!(state.heading, "Room 7 (3/20)");

        state.role_changed("Me", true);
        assert!(state.is_owner("Me"));
        assert_eq!(state.member_lines, "Me (owner)\nOwner\nGuest");

        assert_eq!(state.member_left("Guest", 2, true, "Me", AccountId(1)), MemberLeft::Other);
        assert_eq!(state.members().len(), 2);
        assert_eq!(state.member_left("Me", 1, true, "Me", AccountId(1)), MemberLeft::Myself {
            kicked: true
        });
        assert!(!state.is_in_room());
        assert!(state.member_lines.is_empty());
        // The owner's board stays up for others to join.
        assert_eq!(state.boards().len(), 1);
    }

    #[test]
    fn an_own_room_gets_a_placeholder_until_the_server_names_it() {
        let mut state = ChatRoomState::default();
        assert!(!state.created(AccountId(5), "Me"));
        state.set_pending_create("Mine".to_owned(), 8, true);
        assert!(state.created(AccountId(5), "Me"));
        assert!(state.is_owner("Me"));
        assert_eq!(state.heading, "Mine (1/8)");

        let mut named = board(42, true);
        named.owner_id = AccountId(5);
        named.title = "Renamed".to_owned();
        state.upsert_board(named);
        assert_eq!(state.boards().len(), 1);
        assert_eq!(state.heading, "Renamed (1/20)");
        state.member_joined("Guest".to_owned(), 2);
        assert_eq!(state.board(ChatRoomId(42)).unwrap().users, 2);

        assert_eq!(state.member_left("Me", 1, false, "Me", AccountId(5)), MemberLeft::Myself {
            kicked: false
        });
        assert!(state.boards().is_empty());
    }

    #[test]
    fn public_boards_join_at_once_and_private_ones_ask_for_a_password() {
        let mut state = ChatRoomState::default();
        state.upsert_board(board(1, true));
        state.upsert_board(board(2, false));
        assert_eq!(state.click_board(ChatRoomId(1)), Some(ChatRoomId(1)));
        assert_eq!(state.click_board(ChatRoomId(2)), None);
        assert!(state.join_prompt.contains("Room 2"));
        state.join_password = "secret".to_owned();
        assert_eq!(state.take_pending_join(), Some((ChatRoomId(2), "secret".to_owned())));
        assert_eq!(state.take_pending_join(), None);
        assert_eq!(state.click_board(ChatRoomId(9)), None);
    }

    #[test]
    fn the_form_checks_title_limit_and_password() {
        let mut state = ChatRoomState::default();
        state.begin_create("Nova");
        assert_eq!(state.form_request(), Ok(("Nova's room".to_owned(), 20, true, String::new())));
        state.form_limit = "1".to_owned();
        assert!(state.form_request().unwrap_err().contains("2 to 20"));
        state.form_limit = "5".to_owned();
        state.form_public = false;
        assert!(state.form_request().unwrap_err().contains("password"));
        state.form_password = "pw".to_owned();
        assert_eq!(state.form_request(), Ok(("Nova's room".to_owned(), 5, false, "pw".to_owned())));
        state.form_title = "  ".to_owned();
        assert!(state.form_request().is_err());
    }

    #[test]
    fn server_results_read_as_sentences() {
        assert_eq!(create_result_message(0), None);
        assert!(create_result_message(2).unwrap().contains("already"));
        assert!(refused_message(1).contains("password"));
        assert!(refused_message(0).contains("full"));
    }
}
