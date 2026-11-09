//! Local friend list rows with online/offline status.

use korangar_interface::element::StateElement;
use ragnarok_packets::{AccountId, CharacterId, Friend};
use rust_state::RustState;

/// One friend row shown in the Friends window.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct FriendListEntry {
    pub account_id: AccountId,
    pub character_id: CharacterId,
    pub name: String,
    pub online: bool,
    /// Stable chip/row label (`Name — Online`).
    pub label: String,
}

fn format_label(name: &str, online: bool) -> String {
    if online {
        format!("{name} — Online")
    } else {
        format!("{name} — Offline")
    }
}

impl FriendListEntry {
    pub fn from_friend(friend: Friend, online: bool) -> Self {
        let label = format_label(&friend.name, online);
        Self {
            account_id: friend.account_id,
            character_id: friend.character_id,
            name: friend.name,
            online,
            label,
        }
    }

    pub fn set_online(&mut self, online: bool) {
        self.online = online;
        self.label = format_label(&self.name, online);
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{AccountId, CharacterId, Friend};

    use super::FriendListEntry;

    #[test]
    fn label_tracks_online_flag() {
        let friend = Friend {
            account_id: AccountId(1),
            character_id: CharacterId(2),
            name: "Ada".to_owned(),
        };
        let mut entry = FriendListEntry::from_friend(friend, false);
        assert!(entry.label.contains("Offline"));
        entry.set_online(true);
        assert!(entry.label.contains("Online"));
        assert!(entry.online);
    }
}
