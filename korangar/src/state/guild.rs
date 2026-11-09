//! Local guild roster, basic info, and notice.

use korangar_interface::element::StateElement;
use korangar_networking::GuildMemberSnapshot;
use ragnarok_packets::{AccountId, CharacterId};
use rust_state::RustState;

/// One guild member shown in the Guild window.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct GuildMember {
    pub account_id: AccountId,
    pub character_id: CharacterId,
    pub name: String,
    pub job: i16,
    pub level: i16,
    pub position_id: i32,
    pub online: bool,
    pub contribution: i32,
}

/// A guild position title from `ZC_POSITION_ID_NAME_INFO`.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct GuildPosition {
    pub id: i32,
    pub name: String,
}

/// Fields from `ZC_GUILD_INFO` (`0x0B7B`).
pub struct GuildInfoUpdate {
    pub guild_id: u32,
    pub level: i32,
    pub online_members: i32,
    pub max_members: i32,
    pub average_level: i32,
    pub experience: i32,
    pub next_experience: i32,
    pub emblem_version: i32,
    pub name: String,
    pub master_name: String,
    pub territory: String,
    pub master_character_id: u32,
}

/// Guild the local character belongs to. Empty when `joined` is false.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct GuildState {
    pub joined: bool,
    pub guild_id: u32,
    pub name: String,
    pub master_name: String,
    pub master_character_id: u32,
    pub level: i32,
    pub online_members: i32,
    pub max_members: i32,
    pub average_level: i32,
    pub experience: i32,
    pub next_experience: i32,
    pub territory: String,
    pub emblem_version: i32,
    pub is_master: bool,
    pub subject: String,
    pub notice: String,
    pub members: Vec<GuildMember>,
    pub positions: Vec<GuildPosition>,
    /// Preformatted rows so the window can borrow them for the frame.
    pub rows: Vec<String>,
}

impl GuildState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn refresh_rows(&mut self) {
        let mut rows = Vec::new();
        rows.push(format!("Name: {}", self.name));
        rows.push(format!("Master: {}", self.master_name));
        rows.push(format!("Level: {}", self.level));
        rows.push(format!("Online: {} / {}", self.online_members, self.max_members));
        rows.push(format!("Average level: {}", self.average_level));
        rows.push(format!("Experience: {} / {}", self.experience, self.next_experience));
        rows.push(format!("Territory: {}", self.territory));
        rows.push(format!("Emblem: {}", self.emblem_version));
        rows.push("Members".to_owned());
        if self.members.is_empty() {
            rows.push("No members loaded.".to_owned());
        } else {
            for member in &self.members {
                let presence = if member.online { "Online" } else { "Offline" };
                let position = self
                    .positions
                    .iter()
                    .find(|position| position.id == member.position_id)
                    .map(|position| position.name.clone())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| format!("Position {}", member.position_id));
                rows.push(format!(
                    "{}  Lv{}  {}  {}  contrib {}",
                    member.name, member.level, position, presence, member.contribution
                ));
            }
        }
        rows.push(format!("Subject: {}", self.subject));
        rows.push(format!("Notice: {}", self.notice));
        self.rows = rows;
    }

    pub fn apply_affiliation(&mut self, guild_id: u32, name: String, is_master: bool, master_character_id: u32) {
        if guild_id == 0 {
            self.clear();
            return;
        }
        if self.guild_id != guild_id {
            self.members.clear();
            self.positions.clear();
            self.subject.clear();
            self.notice.clear();
        }
        self.joined = true;
        self.guild_id = guild_id;
        self.name = name;
        self.is_master = is_master;
        self.master_character_id = master_character_id;
        self.refresh_rows();
    }

    pub fn apply_info(&mut self, info: GuildInfoUpdate) {
        if info.guild_id == 0 {
            self.clear();
            return;
        }
        if self.guild_id != info.guild_id {
            self.members.clear();
            self.positions.clear();
        }
        self.joined = true;
        self.guild_id = info.guild_id;
        self.level = info.level;
        self.online_members = info.online_members;
        self.max_members = info.max_members;
        self.average_level = info.average_level;
        self.experience = info.experience;
        self.next_experience = info.next_experience;
        self.emblem_version = info.emblem_version;
        self.name = info.name;
        self.master_name = info.master_name;
        self.territory = info.territory;
        self.master_character_id = info.master_character_id;
        self.refresh_rows();
    }

    pub fn set_members(&mut self, members: Vec<GuildMemberSnapshot>) {
        self.members = members
            .into_iter()
            .map(|member| GuildMember {
                account_id: member.account_id,
                character_id: member.character_id,
                name: member.name,
                job: member.job,
                level: member.level,
                position_id: member.position_id,
                online: member.online,
                contribution: member.contribution,
            })
            .collect();
        self.refresh_rows();
    }

    pub fn set_positions(&mut self, positions: Vec<(i32, String)>) {
        self.positions = positions.into_iter().map(|(id, name)| GuildPosition { id, name }).collect();
        self.refresh_rows();
    }

    pub fn set_notice(&mut self, subject: String, notice: String) {
        self.subject = subject;
        self.notice = notice;
        self.refresh_rows();
    }

    pub fn set_member_online(&mut self, account_id: AccountId, character_id: CharacterId, online: bool) {
        if let Some(member) = self
            .members
            .iter_mut()
            .find(|member| member.account_id == account_id && member.character_id == character_id)
        {
            member.online = online;
            self.refresh_rows();
        }
    }

    /// Removes a member and returns their name when the roster had that
    /// character.
    pub fn remove_member(&mut self, character_id: CharacterId) -> Option<String> {
        let index = self.members.iter().position(|member| member.character_id == character_id)?;
        let name = self.members.remove(index).name;
        self.refresh_rows();
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(id: u32, name: &str) -> GuildMemberSnapshot {
        GuildMemberSnapshot {
            account_id: AccountId(id),
            character_id: CharacterId(id),
            name: name.to_owned(),
            job: 0,
            level: 1,
            position_id: 0,
            online: true,
            contribution: 0,
        }
    }

    #[test]
    fn affiliation_zero_clears_the_roster() {
        let mut guild = GuildState::default();
        guild.apply_affiliation(4, "Wolves".to_owned(), true, 9);
        guild.set_members(vec![member(9, "Alpha")]);
        guild.apply_affiliation(0, String::new(), false, 0);
        assert!(!guild.joined);
        assert!(guild.members.is_empty());
        assert!(guild.name.is_empty());
    }

    #[test]
    fn online_flag_and_removal_follow_character_id() {
        let mut guild = GuildState::default();
        guild.apply_info(GuildInfoUpdate {
            guild_id: 4,
            level: 2,
            online_members: 1,
            max_members: 10,
            average_level: 20,
            experience: 0,
            next_experience: 1,
            emblem_version: 0,
            name: "Wolves".to_owned(),
            master_name: "Alpha".to_owned(),
            territory: "Prontera".to_owned(),
            master_character_id: 9,
        });
        guild.set_members(vec![member(9, "Alpha"), member(8, "Beta")]);
        guild.set_member_online(AccountId(8), CharacterId(8), false);
        assert!(!guild.members[1].online);
        assert_eq!(guild.remove_member(CharacterId(9)).as_deref(), Some("Alpha"));
        assert_eq!(guild.members.len(), 1);
        assert!(guild.remove_member(CharacterId(9)).is_none());
    }

    #[test]
    fn switching_guild_drops_the_previous_roster() {
        let mut guild = GuildState::default();
        guild.apply_affiliation(1, "Old".to_owned(), false, 1);
        guild.set_members(vec![member(1, "Old")]);
        guild.apply_info(GuildInfoUpdate {
            guild_id: 2,
            level: 1,
            online_members: 1,
            max_members: 16,
            average_level: 10,
            experience: 0,
            next_experience: 1,
            emblem_version: 3,
            name: "New".to_owned(),
            master_name: "Lead".to_owned(),
            territory: String::new(),
            master_character_id: 5,
        });
        assert!(guild.members.is_empty());
        assert_eq!(guild.name, "New");
        assert!(!guild.is_master);
    }
}
