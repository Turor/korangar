//! Chat room flow: server events and window buttons for chat rooms.
//!
//! Packet sends stay in `korangar-networking`; the room model lives in
//! `state::chat_room`. `lib.rs` only forwards the events here.

use korangar_networking::{MessageColor, NetworkEvent};
use ragnarok_packets::{AccountId, ChatRoomId};

use crate::Client;
use crate::input::InputEvent;
use crate::interface::windows::{ChatRoomCreateWindow, ChatRoomPasswordWindow, ChatRoomTitleBox, ChatRoomWindow, WindowClass};
use crate::state::chat_room::{self, MemberLeft};
use crate::state::{ChatMessage, ClientStatePathExt, client_state};

impl Client {
    fn own_account_id(&self) -> AccountId {
        self.saved_login_data.as_ref().map(|login| login.account_id).unwrap_or(AccountId(0))
    }

    fn own_name(&self) -> String {
        self.client_state.follow(client_state().player_name()).clone()
    }

    fn chat_room_chat(&mut self, text: impl Into<String>, color: MessageColor) {
        self.client_state
            .follow_mut(client_state().chat_messages())
            .push(ChatMessage::new(text.into(), color));
    }

    fn show_chat_room_window(&mut self) {
        if !self.interface.is_window_with_class_open(WindowClass::ChatRoom) {
            self.interface.open_window(ChatRoomWindow);
        }
    }

    /// Drop every room (map change or disconnect).
    pub(crate) fn clear_chat_rooms(&mut self) {
        self.client_state.follow_mut(client_state().chat_room()).clear();
        for class in [WindowClass::ChatRoom, WindowClass::ChatRoomCreate, WindowClass::ChatRoomPassword] {
            self.interface.close_window_with_class(class);
        }
    }

    pub(crate) fn on_chat_room_network_event(&mut self, event: NetworkEvent) {
        let my_name = self.own_name();
        let my_account = self.own_account_id();
        match event {
            NetworkEvent::ChatRoomCreateResult { result } => match chat_room::create_result_message(result) {
                None => {
                    let created = self
                        .client_state
                        .follow_mut(client_state().chat_room())
                        .created(my_account, &my_name);
                    if created {
                        self.interface.close_window_with_class(WindowClass::ChatRoomCreate);
                        self.show_chat_room_window();
                    }
                }
                Some(message) => {
                    self.client_state.follow_mut(client_state().chat_room()).create_failed();
                    self.chat_room_chat(message, MessageColor::Error);
                }
            },
            NetworkEvent::ChatRoomBoard {
                owner_id,
                chat_id,
                limit,
                users,
                public,
                title,
            } => {
                self.client_state
                    .follow_mut(client_state().chat_room())
                    .upsert_board(chat_room::ChatRoomBoard {
                        owner_id,
                        chat_id,
                        limit,
                        users,
                        public,
                        title,
                    });
            }
            NetworkEvent::ChatRoomDestroyed { chat_id } => {
                self.client_state.follow_mut(client_state().chat_room()).remove_board(chat_id);
            }
            NetworkEvent::ChatRoomRefused { result } => {
                self.chat_room_chat(chat_room::refused_message(result), MessageColor::Error);
            }
            NetworkEvent::ChatRoomEntered { chat_id, members } => {
                self.client_state.follow_mut(client_state().chat_room()).entered(chat_id, members);
                self.interface.close_window_with_class(WindowClass::ChatRoomPassword);
                self.show_chat_room_window();
            }
            NetworkEvent::ChatRoomMemberJoined { users, name } => {
                self.chat_room_chat(format!("{name} joined the chat room."), MessageColor::Information);
                self.client_state.follow_mut(client_state().chat_room()).member_joined(name, users);
            }
            NetworkEvent::ChatRoomMemberLeft { users, name, kicked } => {
                let outcome = self
                    .client_state
                    .follow_mut(client_state().chat_room())
                    .member_left(&name, users, kicked, &my_name, my_account);
                match outcome {
                    MemberLeft::Myself { kicked } => {
                        self.interface.close_window_with_class(WindowClass::ChatRoom);
                        self.interface.close_window_with_class(WindowClass::ChatRoomCreate);
                        let message = match kicked {
                            true => "You were kicked from the chat room.",
                            false => "You left the chat room.",
                        };
                        self.chat_room_chat(message, MessageColor::Information);
                    }
                    MemberLeft::Other => {
                        let verb = if kicked { "was kicked from" } else { "left" };
                        self.chat_room_chat(format!("{name} {verb} the chat room."), MessageColor::Information);
                    }
                }
            }
            NetworkEvent::ChatRoomRoleChanged { name, owner } => {
                self.client_state.follow_mut(client_state().chat_room()).role_changed(&name, owner);
            }
            _ => {}
        }
    }

    /// A click on a room board above a player.
    pub(crate) fn on_chat_room_board_click(&mut self, chat_id: ChatRoomId) {
        let join = self.client_state.follow_mut(client_state().chat_room()).click_board(chat_id);
        match join {
            Some(chat_id) => {
                let _ = self.networking_system.enter_chat_room(chat_id, "");
            }
            None => {
                let prompting = self.client_state.follow(client_state().chat_room()).has_pending_join();
                if prompting && !self.interface.is_window_with_class_open(WindowClass::ChatRoomPassword) {
                    self.interface.open_window(ChatRoomPasswordWindow);
                }
            }
        }
    }

    pub(crate) fn on_chat_room_input(&mut self, event: InputEvent) {
        match event {
            InputEvent::OpenChatRoomCreate => {
                if self.client_state.follow(client_state().chat_room()).is_in_room() {
                    self.show_chat_room_window();
                    return;
                }
                let my_name = self.own_name();
                self.client_state.follow_mut(client_state().chat_room()).begin_create(&my_name);
                self.open_chat_room_form();
            }
            InputEvent::OpenChatRoomSettings => {
                let my_name = self.own_name();
                let room = self.client_state.follow_mut(client_state().chat_room());
                if !room.is_owner(&my_name) {
                    self.chat_room_chat("Only the room owner can change the room.", MessageColor::Error);
                    return;
                }
                room.begin_change();
                self.open_chat_room_form();
            }
            InputEvent::CloseChatRoomSettings => {
                self.interface.close_window_with_class(WindowClass::ChatRoomCreate);
            }
            InputEvent::SubmitChatRoomSettings => {
                let (request, changes_room) = {
                    let room = self.client_state.follow(client_state().chat_room());
                    (room.form_request(), room.form_changes_room())
                };
                let (title, limit, public, password) = match request {
                    Ok(request) => request,
                    Err(message) => {
                        self.chat_room_chat(message, MessageColor::Error);
                        return;
                    }
                };
                if changes_room {
                    let _ = self.networking_system.change_chat_room(&title, limit, public, &password);
                    self.interface.close_window_with_class(WindowClass::ChatRoomCreate);
                } else {
                    self.client_state
                        .follow_mut(client_state().chat_room())
                        .set_pending_create(title.clone(), limit, public);
                    let _ = self.networking_system.create_chat_room(&title, limit, public, &password);
                }
            }
            InputEvent::LeaveChatRoom => {
                let _ = self.networking_system.leave_chat_room();
            }
            InputEvent::KickChatRoomMember | InputEvent::GiveChatRoomOwnership => {
                let my_name = self.own_name();
                let (owner, target) = {
                    let room = self.client_state.follow(client_state().chat_room());
                    (room.is_owner(&my_name), room.member_name.trim().to_owned())
                };
                if !owner {
                    self.chat_room_chat("Only the room owner can do that.", MessageColor::Error);
                    return;
                }
                if target.is_empty() || target == my_name {
                    self.chat_room_chat("Type the name of another member first.", MessageColor::Error);
                    return;
                }
                let _ = match event {
                    InputEvent::KickChatRoomMember => self.networking_system.expel_chat_room_member(&target),
                    _ => self.networking_system.give_chat_room_ownership(&target),
                };
                self.client_state.follow_mut(client_state().chat_room()).member_name.clear();
            }
            InputEvent::ConfirmChatRoomPassword => {
                let pending = self.client_state.follow_mut(client_state().chat_room()).take_pending_join();
                if let Some((chat_id, password)) = pending {
                    let _ = self.networking_system.enter_chat_room(chat_id, &password);
                }
                self.interface.close_window_with_class(WindowClass::ChatRoomPassword);
            }
            InputEvent::CancelChatRoomPassword => {
                self.client_state.follow_mut(client_state().chat_room()).cancel_join();
                self.interface.close_window_with_class(WindowClass::ChatRoomPassword);
            }
            InputEvent::ClickChatRoomBoard { chat_id } => self.on_chat_room_board_click(chat_id),
            _ => {}
        }
    }

    fn open_chat_room_form(&mut self) {
        if !self.interface.is_window_with_class_open(WindowClass::ChatRoomCreate) {
            self.interface.open_window(ChatRoomCreateWindow);
        }
        self.interface.request_focus(ChatRoomTitleBox);
    }
}
