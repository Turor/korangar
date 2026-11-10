//! Right-click menu entries for a player.
//!
//! Guild invite and block/ignore are omitted: there is no merged guild-invite
//! sender, and roBrowser leaves block commented out.

use korangar_interface::element::StateElement;
use rust_state::RustState;

/// What a menu row does when it is clicked. The input event carries the row
/// index so this type stays out of `InputEvent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, RustState, StateElement)]
pub enum ContextMenuAction {
    ViewEquipment,
    Trade,
    Whisper,
    AddFriend,
    PartyInvite,
    Sit,
    Stand,
}

#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct ContextMenuEntry {
    pub label: String,
    pub action: ContextMenuAction,
}

/// Facts the menu is built from. Name-based rows are omitted when the name is
/// empty. Guild membership does not add a row.
pub struct ContextMenuFacts<'a> {
    pub self_target: bool,
    pub sitting: bool,
    pub target_name: &'a str,
    pub friend_names: &'a [String],
    pub in_party: bool,
    pub party_leader: bool,
}

#[derive(Clone, Default, RustState, StateElement)]
pub struct ContextMenuState {
    pub target_entity_id: u32,
    pub self_target: bool,
    /// Character name used for whisper, friend, and party. Empty when unknown.
    pub character_name: String,
    pub title: String,
    /// Local player is sitting. Sit and stand packets update this.
    pub self_sitting: bool,
    pub entries: Vec<ContextMenuEntry>,
}

impl ContextMenuState {
    pub fn open(&mut self, entity_id: u32, facts: &ContextMenuFacts<'_>) {
        self.target_entity_id = entity_id;
        self.self_target = facts.self_target;
        self.character_name = facts.target_name.to_owned();
        self.title = if facts.self_target {
            "You".to_owned()
        } else if facts.target_name.is_empty() {
            "Player".to_owned()
        } else {
            facts.target_name.to_owned()
        };
        self.entries = build_context_menu(facts);
    }
}

pub fn build_context_menu(facts: &ContextMenuFacts<'_>) -> Vec<ContextMenuEntry> {
    if facts.self_target {
        return vec![if facts.sitting {
            entry("Stand", ContextMenuAction::Stand)
        } else {
            entry("Sit", ContextMenuAction::Sit)
        }];
    }

    let mut entries = vec![
        entry("View equipment", ContextMenuAction::ViewEquipment),
        entry("Trade", ContextMenuAction::Trade),
    ];
    let named = !facts.target_name.is_empty();
    if named {
        entries.push(entry("Whisper", ContextMenuAction::Whisper));
        if !is_friend(facts.target_name, facts.friend_names) {
            entries.push(entry("Add friend", ContextMenuAction::AddFriend));
        }
        if facts.in_party && facts.party_leader {
            entries.push(entry("Party invite", ContextMenuAction::PartyInvite));
        }
    }
    entries
}

fn is_friend(name: &str, friends: &[String]) -> bool {
    friends.iter().any(|friend| friend.eq_ignore_ascii_case(name))
}

fn entry(label: &str, action: ContextMenuAction) -> ContextMenuEntry {
    ContextMenuEntry {
        label: label.to_owned(),
        action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts<'a>(
        self_target: bool,
        sitting: bool,
        target_name: &'a str,
        friend_names: &'a [String],
        in_party: bool,
        party_leader: bool,
    ) -> ContextMenuFacts<'a> {
        ContextMenuFacts {
            self_target,
            sitting,
            target_name,
            friend_names,
            in_party,
            party_leader,
        }
    }

    fn labels(entries: &[ContextMenuEntry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.label.as_str()).collect()
    }

    #[test]
    fn other_player_menu_matches_the_robrowser_order_without_guild_or_block() {
        let friends = vec!["Bob".to_owned()];
        let entries = build_context_menu(&facts(false, false, "Alice", &friends, true, true));
        assert_eq!(labels(&entries), vec![
            "View equipment",
            "Trade",
            "Whisper",
            "Add friend",
            "Party invite"
        ]);
        assert!(
            labels(&entries)
                .iter()
                .all(|label| *label != "Guild invite" && *label != "Block" && *label != "Ignore")
        );
    }

    #[test]
    fn add_friend_is_hidden_when_the_name_matches_ignoring_case() {
        let friends = vec!["ALICE".to_owned()];
        let entries = build_context_menu(&facts(false, false, "Alice", &friends, false, false));
        assert_eq!(labels(&entries), vec!["View equipment", "Trade", "Whisper"]);
    }

    #[test]
    fn party_invite_requires_leadership_and_a_name() {
        let entries = build_context_menu(&facts(false, false, "Alice", &[], true, false));
        assert!(!labels(&entries).contains(&"Party invite"));
        let entries = build_context_menu(&facts(false, false, "Alice", &[], false, true));
        assert!(!labels(&entries).contains(&"Party invite"));
        let entries = build_context_menu(&facts(false, false, "", &[], true, true));
        assert_eq!(labels(&entries), vec!["View equipment", "Trade"]);
    }

    #[test]
    fn self_menu_is_sit_or_stand() {
        assert_eq!(
            build_context_menu(&facts(true, false, "Hero", &[], true, true))[0].action,
            ContextMenuAction::Sit
        );
        assert_eq!(
            build_context_menu(&facts(true, true, "Hero", &[], true, true))[0].action,
            ContextMenuAction::Stand
        );
        assert_eq!(build_context_menu(&facts(true, true, "Hero", &[], true, true)).len(), 1);
    }
}
