//! Accept or decline a guild invitation (`0x016A` / `0x016B`).

use korangar_interface::window::{CustomWindow, Window};

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::state::ClientState;
use crate::state::theme::InterfaceThemeType;

pub struct GuildInviteWindow {
    guild_id: u32,
    guild_name: String,
}

impl GuildInviteWindow {
    pub fn new(guild_id: u32, guild_name: String) -> Self {
        Self { guild_id, guild_name }
    }
}

impl CustomWindow<ClientState> for GuildInviteWindow {
    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let guild_id = self.guild_id;
        let decline_id = guild_id;
        let message = format!("{} invited you to join their guild", self.guild_name);

        window! {
            title: "Guild invite",
            class: Some(WindowClass::GuildInvite),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: message,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Decline",
                            event: InputEvent::RejectGuildInvite { guild_id: decline_id },
                        },
                        button! {
                            text: "Accept",
                            event: InputEvent::AcceptGuildInvite { guild_id },
                        },
                    ),
                },
            ),
        }
    }
}
