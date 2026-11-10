//! Local party roster, options, and minimap positions.
//!
//! The minimap window reads [`PartyState::minimap_positions`]. `mvp_minimap.rs`
//! is left unchanged.

use korangar_interface::element::StateElement;
use korangar_networking::{PartyRosterEntry, PartyUpdate};
use ragnarok_packets::{AccountId, CharacterId, EntityId, PartyId};
use rust_state::RustState;

/// Text shown in chat for a party result.
pub struct PartyNotice {
    pub text: String,
    pub error: bool,
}

/// One party member. Spell points stay at 0: `0x080E` on 20250716 RE has no SP.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct PartyMember {
    pub account_id: AccountId,
    pub character_id: CharacterId,
    pub name: String,
    pub map_name: String,
    pub leader: bool,
    pub online: bool,
    pub dead: bool,
    pub job: i16,
    pub base_level: i16,
    pub health: i32,
    pub maximum_health: i32,
    pub spell_points: i32,
    pub maximum_spell_points: i32,
    pub x: i16,
    pub y: i16,
    pub has_position: bool,
    pub label: String,
}

/// Party the local character belongs to.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct PartyState {
    pub joined: bool,
    pub name: String,
    pub members: Vec<PartyMember>,
    /// 0 off, 1 on. A locked update (`2`) sets
    /// [`Self::experience_share_locked`].
    pub experience_share: u32,
    pub experience_share_locked: bool,
    pub item_pickup: bool,
    pub item_division: bool,
    pub invitations_allowed: bool,
    pub local_account_id: AccountId,
    pub self_is_leader: bool,
    /// Invite window that is still open. The autopilot accepts this id.
    pub pending_invite: Option<PartyId>,
}

impl Default for PartyState {
    fn default() -> Self {
        Self {
            joined: false,
            name: String::new(),
            members: Vec::new(),
            experience_share: 0,
            experience_share_locked: false,
            item_pickup: false,
            item_division: false,
            invitations_allowed: true,
            local_account_id: AccountId(0),
            self_is_leader: false,
            pending_invite: None,
        }
    }
}

impl PartyState {
    pub fn clear(&mut self) {
        let invitations_allowed = self.invitations_allowed;
        *self = Self::default();
        self.invitations_allowed = invitations_allowed;
    }

    pub fn set_pending_invite(&mut self, party_id: PartyId) {
        self.pending_invite = Some(party_id);
    }

    pub fn take_pending_invite(&mut self) -> Option<PartyId> {
        self.pending_invite.take()
    }

    pub fn set_invitations_allowed(&mut self, invitations_allowed: bool) {
        self.invitations_allowed = invitations_allowed;
    }

    /// Same-map party members for the minimap. Omits the local character and
    /// anyone whose last `0x0107` coordinate was negative.
    ///
    /// `mvp_minimap.rs` is intentionally unchanged. The minimap worker should
    /// read this accessor.
    #[allow(dead_code)]
    pub fn minimap_positions(&self) -> Vec<(AccountId, i16, i16)> {
        self.members
            .iter()
            .filter(|member| member.has_position && member.x >= 0 && member.y >= 0 && member.account_id != self.local_account_id)
            .map(|member| (member.account_id, member.x, member.y))
            .collect()
    }

    /// HP for overhead bars. Omits the local player, who already draws full
    /// bars.
    pub fn world_health_bars(&self) -> Vec<(EntityId, i32, i32)> {
        self.members
            .iter()
            .filter(|member| member.account_id != self.local_account_id && member.maximum_health > 0)
            .map(|member| (EntityId(member.account_id.0), member.health.max(0), member.maximum_health))
            .collect()
    }

    pub fn apply(&mut self, update: PartyUpdate, local_account_id: AccountId) -> Option<PartyNotice> {
        self.local_account_id = local_account_id;
        let notice = match update {
            PartyUpdate::CreateResult(result) => self.apply_create(result),
            PartyUpdate::Roster { name, members } => {
                self.apply_roster(name, members);
                None
            }
            PartyUpdate::MemberAdded {
                party_name,
                share_pickup,
                share_loot,
                member,
            } => self.apply_member_added(party_name, share_pickup, share_loot, member),
            PartyUpdate::MemberRemoved { account_id, name, result } => self.apply_removed(account_id, name, result),
            PartyUpdate::Options {
                experience_share,
                item_pickup,
                item_division,
            } => self.apply_options(experience_share, item_pickup, item_division),
            PartyUpdate::ExperienceShare(experience_share) => self.apply_experience(experience_share),
            PartyUpdate::Leader { new_leader, .. } => self.apply_leader(new_leader),
            PartyUpdate::Health {
                account_id,
                health,
                maximum,
            } => {
                if let Some(member) = self.member_mut(account_id) {
                    member.health = health;
                    member.maximum_health = maximum;
                    refresh_label(member);
                }
                None
            }
            PartyUpdate::Position { account_id, x, y } => {
                if let Some(member) = self.member_mut(account_id) {
                    if x < 0 || y < 0 {
                        member.has_position = false;
                    } else {
                        member.x = x;
                        member.y = y;
                        member.has_position = true;
                    }
                }
                None
            }
            PartyUpdate::JobLevel { account_id, job, level } => {
                if let Some(member) = self.member_mut(account_id) {
                    member.job = job;
                    member.base_level = level;
                    refresh_label(member);
                }
                None
            }
            PartyUpdate::Alive { account_id, dead } => {
                if let Some(member) = self.member_mut(account_id) {
                    member.dead = dead;
                    if dead {
                        member.health = 0;
                    }
                    refresh_label(member);
                }
                None
            }
            PartyUpdate::InviteResult { name, result } => Some(invite_notice(&name, result)),
        };
        self.refresh_leader_flag();
        notice
    }

    fn apply_create(&mut self, result: u8) -> Option<PartyNotice> {
        let (text, error) = match result {
            0 => {
                self.joined = true;
                self.self_is_leader = true;
                ("Party created.".to_owned(), false)
            }
            1 => ("A party with that name already exists.".to_owned(), true),
            2 => ("You are already in a party.".to_owned(), true),
            3 => ("You cannot create a party here.".to_owned(), true),
            other => (format!("Party creation failed ({other})."), true),
        };
        Some(PartyNotice { text, error })
    }

    fn apply_roster(&mut self, name: String, members: Vec<PartyRosterEntry>) {
        self.joined = true;
        self.name = name;
        let previous = std::mem::take(&mut self.members);
        self.members = members
            .into_iter()
            .map(|entry| {
                let mut member = previous
                    .iter()
                    .find(|member| member.account_id == entry.account_id)
                    .cloned()
                    .unwrap_or_else(|| member_from_entry(&entry));
                let health = member.health;
                let maximum_health = member.maximum_health;
                let spell_points = member.spell_points;
                let maximum_spell_points = member.maximum_spell_points;
                let dead = member.dead;
                let had_position = member.has_position;
                let x = member.x;
                let y = member.y;
                apply_entry(&mut member, &entry);
                member.health = health;
                member.maximum_health = maximum_health;
                member.spell_points = spell_points;
                member.maximum_spell_points = maximum_spell_points;
                member.dead = dead;
                if entry.has_position {
                    member.x = entry.x;
                    member.y = entry.y;
                    member.has_position = entry.x >= 0 && entry.y >= 0;
                } else {
                    member.x = x;
                    member.y = y;
                    member.has_position = had_position;
                }
                refresh_label(&mut member);
                member
            })
            .collect();
    }

    fn apply_member_added(
        &mut self,
        party_name: String,
        share_pickup: bool,
        share_loot: bool,
        entry: PartyRosterEntry,
    ) -> Option<PartyNotice> {
        let was_joined = self.joined;
        let is_new = self.members.iter().all(|member| member.account_id != entry.account_id);
        let is_self = entry.account_id == self.local_account_id;
        self.joined = true;
        self.name = party_name;
        self.item_pickup = share_pickup;
        self.item_division = share_loot;
        if let Some(member) = self.members.iter_mut().find(|member| member.account_id == entry.account_id) {
            let health = member.health;
            let maximum_health = member.maximum_health;
            let spell_points = member.spell_points;
            let maximum_spell_points = member.maximum_spell_points;
            let dead = member.dead;
            apply_entry(member, &entry);
            member.health = health;
            member.maximum_health = maximum_health;
            member.spell_points = spell_points;
            member.maximum_spell_points = maximum_spell_points;
            member.dead = dead;
            if entry.has_position {
                member.x = entry.x;
                member.y = entry.y;
                member.has_position = true;
            }
            refresh_label(member);
        } else {
            let mut member = member_from_entry(&entry);
            refresh_label(&mut member);
            self.members.push(member);
        }
        if !is_new {
            return None;
        }
        if is_self {
            if was_joined {
                None
            } else {
                Some(PartyNotice {
                    text: format!("Joined party {}.", self.name),
                    error: false,
                })
            }
        } else {
            Some(PartyNotice {
                text: format!("{} joined the party.", entry.name),
                error: false,
            })
        }
    }

    fn apply_removed(&mut self, account_id: AccountId, name: String, result: u8) -> Option<PartyNotice> {
        match result {
            0 | 1 => {
                let is_self = account_id == self.local_account_id;
                self.members.retain(|member| member.account_id != account_id);
                let text = match (result, is_self) {
                    (1, true) => "You were expelled from the party.".to_owned(),
                    (1, false) => format!("{name} was expelled from the party."),
                    (_, true) => "You left the party.".to_owned(),
                    (_, false) => format!("{name} left the party."),
                };
                if is_self {
                    let invitations_allowed = self.invitations_allowed;
                    *self = Self::default();
                    self.invitations_allowed = invitations_allowed;
                    self.local_account_id = account_id;
                }
                Some(PartyNotice { text, error: false })
            }
            2 => Some(PartyNotice {
                text: "You cannot leave the party on this map.".to_owned(),
                error: true,
            }),
            3 => Some(PartyNotice {
                text: "You cannot expel a party member on this map.".to_owned(),
                error: true,
            }),
            other => Some(PartyNotice {
                text: format!("Party change failed ({other})."),
                error: true,
            }),
        }
    }

    fn apply_options(&mut self, experience_share: u32, item_pickup: bool, item_division: bool) -> Option<PartyNotice> {
        let changed = self.set_experience(experience_share) || self.item_pickup != item_pickup || self.item_division != item_division;
        self.item_pickup = item_pickup;
        self.item_division = item_division;
        changed.then(|| PartyNotice {
            text: "Party options updated.".to_owned(),
            error: false,
        })
    }

    fn apply_experience(&mut self, experience_share: u32) -> Option<PartyNotice> {
        self.set_experience(experience_share).then(|| PartyNotice {
            text: "Party options updated.".to_owned(),
            error: false,
        })
    }

    fn set_experience(&mut self, experience_share: u32) -> bool {
        if experience_share == 2 {
            let changed = !self.experience_share_locked;
            self.experience_share_locked = true;
            changed
        } else {
            let changed = self.experience_share != experience_share || self.experience_share_locked;
            self.experience_share = experience_share;
            self.experience_share_locked = false;
            changed
        }
    }

    fn apply_leader(&mut self, new_leader: AccountId) -> Option<PartyNotice> {
        let mut name = String::new();
        for member in &mut self.members {
            member.leader = member.account_id == new_leader;
            if member.leader {
                name.clone_from(&member.name);
            }
            refresh_label(member);
        }
        let text = if name.is_empty() {
            "Party leader changed.".to_owned()
        } else {
            format!("{name} is now the party leader.")
        };
        Some(PartyNotice { text, error: false })
    }

    fn member_mut(&mut self, account_id: AccountId) -> Option<&mut PartyMember> {
        self.members.iter_mut().find(|member| member.account_id == account_id)
    }

    fn refresh_leader_flag(&mut self) {
        if self.members.is_empty() {
            return;
        }
        self.self_is_leader = self
            .members
            .iter()
            .any(|member| member.account_id == self.local_account_id && member.leader);
    }
}

fn member_from_entry(entry: &PartyRosterEntry) -> PartyMember {
    let mut member = PartyMember {
        account_id: entry.account_id,
        character_id: entry.character_id,
        name: entry.name.clone(),
        map_name: entry.map_name.clone(),
        leader: entry.leader,
        online: entry.online,
        dead: false,
        job: entry.job,
        base_level: entry.base_level,
        health: 0,
        maximum_health: 0,
        spell_points: 0,
        maximum_spell_points: 0,
        x: entry.x,
        y: entry.y,
        has_position: entry.has_position && entry.x >= 0 && entry.y >= 0,
        label: String::new(),
    };
    refresh_label(&mut member);
    member
}

fn apply_entry(member: &mut PartyMember, entry: &PartyRosterEntry) {
    member.account_id = entry.account_id;
    member.character_id = entry.character_id;
    member.name.clone_from(&entry.name);
    member.map_name.clone_from(&entry.map_name);
    member.leader = entry.leader;
    member.online = entry.online;
    member.job = entry.job;
    member.base_level = entry.base_level;
}

fn refresh_label(member: &mut PartyMember) {
    let crown = if member.leader { "♔ " } else { "" };
    let presence = if member.dead {
        "Dead"
    } else if member.online {
        "Online"
    } else {
        "Offline"
    };
    let map = member.map_name.trim_end_matches(".gat");
    let health = if member.maximum_health > 0 {
        format!("  HP {}/{}", member.health.max(0), member.maximum_health)
    } else {
        String::new()
    };
    let spell = if member.maximum_spell_points > 0 {
        format!("  SP {}/{}", member.spell_points.max(0), member.maximum_spell_points)
    } else {
        String::new()
    };
    member.label = format!(
        "{crown}{}  {}  Lv{}  {map}  {presence}{health}{spell}",
        member.name,
        job_label(member.job),
        member.base_level
    );
}

pub fn job_label(job: i16) -> String {
    let name = match job {
        0 => "Novice",
        1 => "Swordman",
        2 => "Mage",
        3 => "Archer",
        4 => "Acolyte",
        5 => "Merchant",
        6 => "Thief",
        7 => "Knight",
        8 => "Priest",
        9 => "Wizard",
        10 => "Blacksmith",
        11 => "Hunter",
        12 => "Assassin",
        13 => "Knight",
        14 => "Crusader",
        15 => "Monk",
        16 => "Sage",
        17 => "Rogue",
        18 => "Alchemist",
        19 => "Bard",
        20 => "Dancer",
        21 => "Crusader",
        22 => "Wedding",
        23 => "Super Novice",
        24 => "Gunslinger",
        25 => "Ninja",
        26 => "Novice High",
        27 => "Swordman High",
        28 => "Mage High",
        29 => "Archer High",
        30 => "Acolyte High",
        _ => return format!("Job {job}"),
    };
    name.to_owned()
}

fn invite_notice(name: &str, result: i32) -> PartyNotice {
    let (text, error) = match result {
        0 => (format!("{name} is already in a party."), true),
        1 => (format!("{name} rejected the party invite."), true),
        2 => (format!("{name} joined the party."), false),
        3 => ("The party is full.".to_owned(), true),
        4 => ("A character from that account is already in the party.".to_owned(), true),
        5 => (format!("{name} is blocking party invites."), true),
        7 => (format!("{name} is offline."), true),
        8 => (format!("{name} is on a map that does not allow parties."), true),
        9 => ("You cannot join a party on this map.".to_owned(), true),
        10 => ("You cannot invite while inside a memorial dungeon.".to_owned(), true),
        11 => (format!("{name} cannot join the party at their level."), true),
        other => (format!("Party invite failed ({other})."), true),
    };
    PartyNotice { text, error }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(state: &mut PartyState, update: PartyUpdate) -> Option<PartyNotice> {
        state.apply(update, AccountId(1))
    }

    fn entry(account_id: u32, name: &str, leader: bool) -> PartyRosterEntry {
        PartyRosterEntry {
            account_id: AccountId(account_id),
            character_id: CharacterId(account_id + 100),
            name: name.to_owned(),
            map_name: "prontera.gat".to_owned(),
            leader,
            online: true,
            job: 7,
            base_level: 50,
            x: 0,
            y: 0,
            has_position: false,
        }
    }

    #[test]
    fn roster_preserves_health_and_position() {
        let mut state = PartyState::default();
        apply(&mut state, PartyUpdate::MemberAdded {
            party_name: "Wolves".to_owned(),
            share_pickup: true,
            share_loot: false,
            member: PartyRosterEntry {
                x: 10,
                y: 20,
                has_position: true,
                ..entry(2, "Nova", false)
            },
        });
        apply(&mut state, PartyUpdate::Health {
            account_id: AccountId(2),
            health: 40,
            maximum: 100,
        });
        apply(&mut state, PartyUpdate::Roster {
            name: "Wolves".to_owned(),
            members: vec![entry(2, "Nova", false), entry(1, "Hero", true)],
        });
        let nova = state.members.iter().find(|member| member.account_id == AccountId(2)).unwrap();
        assert_eq!(nova.health, 40);
        assert_eq!(nova.maximum_health, 100);
        assert_eq!(nova.spell_points, 0);
        assert_eq!(nova.maximum_spell_points, 0);
        assert!(nova.has_position);
        assert_eq!((nova.x, nova.y), (10, 20));
        assert!(state.item_pickup);
        assert!(!state.item_division);
        assert!(state.self_is_leader);
        assert_eq!(job_label(nova.job), "Knight");
        assert!(nova.label.contains("prontera"));
        assert!(!nova.label.contains(".gat"));
    }

    #[test]
    fn negative_position_clears_minimap_mark() {
        let mut state = PartyState::default();
        apply(&mut state, PartyUpdate::Roster {
            name: "Wolves".to_owned(),
            members: vec![entry(1, "Hero", true), entry(2, "Nova", false)],
        });
        apply(&mut state, PartyUpdate::Position {
            account_id: AccountId(2),
            x: 30,
            y: 40,
        });
        assert_eq!(state.minimap_positions(), vec![(AccountId(2), 30, 40)]);
        apply(&mut state, PartyUpdate::Position {
            account_id: AccountId(2),
            x: -1,
            y: 40,
        });
        assert!(state.minimap_positions().is_empty());
        assert!(state.world_health_bars().is_empty());
    }

    #[test]
    fn leave_and_failed_expel_keep_the_right_members() {
        let mut state = PartyState::default();
        apply(&mut state, PartyUpdate::Roster {
            name: "Wolves".to_owned(),
            members: vec![entry(1, "Hero", true), entry(2, "Nova", false)],
        });
        let blocked = apply(&mut state, PartyUpdate::MemberRemoved {
            account_id: AccountId(2),
            name: "Nova".to_owned(),
            result: 3,
        })
        .unwrap();
        assert!(blocked.error);
        assert_eq!(state.members.len(), 2);
        apply(&mut state, PartyUpdate::MemberRemoved {
            account_id: AccountId(2),
            name: "Nova".to_owned(),
            result: 0,
        });
        assert_eq!(state.members.len(), 1);
        apply(&mut state, PartyUpdate::MemberRemoved {
            account_id: AccountId(1),
            name: "Hero".to_owned(),
            result: 1,
        });
        assert!(!state.joined);
        assert!(state.members.is_empty());
    }

    #[test]
    fn failed_create_does_not_join_and_invite_keeps_roster() {
        let mut state = PartyState::default();
        let notice = apply(&mut state, PartyUpdate::CreateResult(1)).unwrap();
        assert!(notice.error);
        assert!(!state.joined);
        let notice = apply(&mut state, PartyUpdate::CreateResult(0)).unwrap();
        assert!(!notice.error);
        assert!(state.joined);
        assert!(state.self_is_leader);
        apply(&mut state, PartyUpdate::Roster {
            name: "Wolves".to_owned(),
            members: vec![entry(1, "Hero", true)],
        });
        let before = state.members.len();
        let invite = apply(&mut state, PartyUpdate::InviteResult {
            name: "Nova".to_owned(),
            result: 7,
        })
        .unwrap();
        assert!(invite.error);
        assert_eq!(state.members.len(), before);
    }

    #[test]
    fn leader_options_and_death() {
        let mut state = PartyState::default();
        apply(&mut state, PartyUpdate::Roster {
            name: "Wolves".to_owned(),
            members: vec![entry(1, "Hero", true), entry(2, "Nova", false)],
        });
        apply(&mut state, PartyUpdate::Health {
            account_id: AccountId(2),
            health: 80,
            maximum: 80,
        });
        apply(&mut state, PartyUpdate::Leader {
            previous: AccountId(1),
            new_leader: AccountId(2),
        });
        assert!(!state.self_is_leader);
        assert!(
            state
                .members
                .iter()
                .any(|member| member.account_id == AccountId(2) && member.leader)
        );
        let options = apply(&mut state, PartyUpdate::Options {
            experience_share: 2,
            item_pickup: true,
            item_division: true,
        })
        .unwrap();
        assert!(!options.error);
        assert!(state.experience_share_locked);
        assert_eq!(state.experience_share, 0);
        apply(&mut state, PartyUpdate::ExperienceShare(1));
        assert!(!state.experience_share_locked);
        assert_eq!(state.experience_share, 1);
        apply(&mut state, PartyUpdate::Alive {
            account_id: AccountId(2),
            dead: true,
        });
        let nova = state.members.iter().find(|member| member.account_id == AccountId(2)).unwrap();
        assert!(nova.dead);
        assert_eq!(nova.health, 0);
        assert_eq!(nova.maximum_health, 80);
        assert_eq!(state.world_health_bars(), vec![(EntityId(2), 0, 80)]);
    }

    #[test]
    fn repeated_member_add_does_not_notice() {
        let mut state = PartyState::default();
        let first = apply(&mut state, PartyUpdate::MemberAdded {
            party_name: "Wolves".to_owned(),
            share_pickup: false,
            share_loot: true,
            member: entry(2, "Nova", false),
        });
        assert!(first.is_some());
        let second = apply(&mut state, PartyUpdate::MemberAdded {
            party_name: "Wolves".to_owned(),
            share_pickup: false,
            share_loot: true,
            member: entry(2, "Nova", false),
        });
        assert!(second.is_none());
        assert_eq!(state.members.len(), 1);
        assert!(state.item_division);
    }
}
