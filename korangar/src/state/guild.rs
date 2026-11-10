//! Local guild roster, skills, relations, notice, and emblem cache.

use std::sync::Arc;

use korangar_interface::element::StateElement;
use korangar_networking::{GuildMemberSnapshot, GuildSkillSnapshot};
use ragnarok_packets::{AccountId, CharacterId, ClientTick};
use rust_state::RustState;

use super::guild_emblem::{self, DecodedEmblem};
use crate::loaders::Sprite;
use crate::world::{Actions, ITEM_SPRITE_PREFIX, SpriteAnimationState};

/// GRF path of a guild skill icon.
///
/// `0x0162` carries `skill->get_name`, the Aegis name. The bitmap is the skill
/// tree's resource, `아이템\{SkillListInformation.file_name}`, keyed by skill
/// id.
pub fn guild_skill_icon_path(file_name: &str) -> String {
    format!("{ITEM_SPRITE_PREFIX}{file_name}")
}

/// Invite permission (`GPERM_INVITE`).
pub const GPERM_INVITE: i32 = 0x001;
/// Expel permission (`GPERM_EXPEL`).
pub const GPERM_EXPEL: i32 = 0x010;
/// Storage permission (`GPERM_STORAGE`, PACKETVER >= 20140205).
pub const GPERM_STORAGE: i32 = 0x100;

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
    pub label: String,
}

/// A guild position: title from `0x0166` and rights from `0x0160`.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct GuildPosition {
    pub id: i32,
    pub name: String,
    pub mode: i32,
    pub ranking: i32,
    pub pay_rate: i32,
    pub label: String,
}

/// Ally (`opposition == 0`) or enemy (`opposition == 1`).
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct GuildRelation {
    pub guild_id: u32,
    pub opposition: i32,
    pub name: String,
    pub label: String,
}

/// One guild skill from `0x0162`.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct GuildSkill {
    pub skill_id: u16,
    pub level: u16,
    pub sp: u16,
    pub range: u16,
    pub name: String,
    pub upgradable: bool,
    pub label: String,
    /// Skill-tree sprite for this skill id. Hidden from the element tree.
    #[hidden_element]
    pub sprite: Option<Arc<Sprite>>,
    /// Matching `.act`, required to draw the sprite.
    #[hidden_element]
    pub actions: Option<Arc<Actions>>,
    #[hidden_element]
    pub animation_state: SpriteAnimationState,
}

/// Cached emblem pixels. Kept when the local character leaves a guild so other
/// players' emblems stay on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuildEmblem {
    pub guild_id: u32,
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
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
    pub tax_points: i32,
    pub honor: i32,
    pub virtue: i32,
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
    pub tax_points: i32,
    pub honor: i32,
    pub virtue: i32,
    pub territory: String,
    pub emblem_version: i32,
    pub is_master: bool,
    pub subject: String,
    pub notice: String,
    pub status: String,
    pub skill_points: i32,
    pub skill_points_label: String,
    pub members: Vec<GuildMember>,
    pub positions: Vec<GuildPosition>,
    pub relations: Vec<GuildRelation>,
    pub skills: Vec<GuildSkill>,
    /// Preformatted info rows so the window can borrow them for the frame.
    pub rows: Vec<String>,
    #[hidden_element]
    pub emblems: Vec<GuildEmblem>,
    #[hidden_element]
    emblem_assembly: Vec<u8>,
    #[hidden_element]
    emblem_assembly_guild: u32,
    #[hidden_element]
    emblem_assembly_version: u32,
    #[hidden_element]
    emblem_requests: Vec<(u32, u32)>,
}

impl GuildState {
    pub fn clear(&mut self) {
        let emblems = std::mem::take(&mut self.emblems);
        *self = Self::default();
        self.emblems = emblems;
    }

    pub fn set_status(&mut self, status: String) {
        self.status = status;
    }

    pub fn can(&self, player_name: &str, bit: i32) -> bool {
        if !self.joined {
            return false;
        }
        if self.is_master {
            return true;
        }
        let Some(member) = self.members.iter().find(|member| member.name == player_name) else {
            return false;
        };
        self.positions
            .iter()
            .find(|position| position.id == member.position_id)
            .is_some_and(|position| position.mode & bit != 0)
    }

    pub fn wants_emblem(&self, guild_id: u32, version: u32) -> bool {
        if guild_id == 0 || version == 0 {
            return false;
        }
        if self
            .emblems
            .iter()
            .any(|emblem| emblem.guild_id == guild_id && emblem.version == version && !emblem.pixels.is_empty())
        {
            return false;
        }
        !self
            .emblem_requests
            .iter()
            .any(|(id, requested)| *id == guild_id && *requested == version)
    }

    pub fn note_emblem_request(&mut self, guild_id: u32, version: u32) {
        if !self
            .emblem_requests
            .iter()
            .any(|(id, requested)| *id == guild_id && *requested == version)
        {
            self.emblem_requests.push((guild_id, version));
        }
    }

    pub fn emblem(&self, guild_id: u32) -> Option<&GuildEmblem> {
        self.emblems
            .iter()
            .find(|emblem| emblem.guild_id == guild_id && !emblem.pixels.is_empty())
    }

    /// `result` matches `ZC_GUILD_EMBLEM_TYPE`: 0 clear, 1 add, 2 complete.
    /// Returns true when a new image was stored.
    pub fn push_emblem_chunk(&mut self, result: u16, guild_id: u32, version: u32, bytes: &[u8]) -> bool {
        if guild_id == 0 {
            return false;
        }
        if self.emblem_assembly_guild != guild_id {
            self.emblem_assembly.clear();
            self.emblem_assembly_guild = guild_id;
        }
        self.emblem_assembly_version = version;
        match result {
            0 => {
                self.emblem_assembly.clear();
                false
            }
            1 => {
                self.emblem_assembly.extend_from_slice(bytes);
                false
            }
            _ => {
                if !bytes.is_empty() {
                    self.emblem_assembly.extend_from_slice(bytes);
                }
                let data = std::mem::take(&mut self.emblem_assembly);
                self.store_emblem_bytes(guild_id, version, &data)
            }
        }
    }

    fn store_emblem_bytes(&mut self, guild_id: u32, version: u32, data: &[u8]) -> bool {
        let Some(decoded) = guild_emblem::decode_emblem(data) else {
            return false;
        };
        self.store_emblem(guild_id, version, decoded);
        true
    }

    fn store_emblem(&mut self, guild_id: u32, version: u32, decoded: DecodedEmblem) {
        if let Some(existing) = self.emblems.iter_mut().find(|emblem| emblem.guild_id == guild_id) {
            existing.version = version;
            existing.width = decoded.width;
            existing.height = decoded.height;
            existing.pixels = decoded.pixels;
        } else {
            self.emblems.push(GuildEmblem {
                guild_id,
                version,
                width: decoded.width,
                height: decoded.height,
                pixels: decoded.pixels,
            });
        }
        self.emblem_requests
            .retain(|(id, requested)| !(*id == guild_id && *requested == version));
    }

    fn refresh_labels(&mut self) {
        let positions = self.positions.clone();
        for member in &mut self.members {
            member.label = member_label(member, &positions);
        }
        for position in &mut self.positions {
            let title = if position.name.is_empty() {
                format!("Position {}", position.id)
            } else {
                position.name.clone()
            };
            position.label = format!("{title}  tax {}%  {}", position.pay_rate, permission_summary(position.mode));
        }
        for relation in &mut self.relations {
            let kind = if relation.opposition == 0 { "Ally" } else { "Enemy" };
            relation.label = format!("{kind}: {}", relation.name);
        }
        for skill in &mut self.skills {
            let mark = if skill.upgradable { "  +" } else { "" };
            skill.label = format!(
                "{}  Lv{}  SP {}  range {}{mark}",
                skill.name, skill.level, skill.sp, skill.range
            );
        }
        self.skill_points_label = format!("Skill points: {}", self.skill_points);
        self.rows = info_rows(self);
    }

    pub fn apply_affiliation(&mut self, guild_id: u32, name: String, is_master: bool, master_character_id: u32, emblem_version: i32) {
        if guild_id == 0 {
            self.clear();
            return;
        }
        if self.guild_id != guild_id {
            self.drop_roster();
        }
        self.joined = true;
        self.guild_id = guild_id;
        self.name = name;
        self.is_master = is_master;
        self.master_character_id = master_character_id;
        if emblem_version != 0 {
            self.emblem_version = emblem_version;
        }
        self.refresh_labels();
    }

    pub fn apply_info(&mut self, info: GuildInfoUpdate) {
        if info.guild_id == 0 {
            self.clear();
            return;
        }
        if self.guild_id != info.guild_id {
            self.drop_roster();
        }
        self.joined = true;
        self.guild_id = info.guild_id;
        self.level = info.level;
        self.online_members = info.online_members;
        self.max_members = info.max_members;
        self.average_level = info.average_level;
        self.experience = info.experience;
        self.next_experience = info.next_experience;
        self.tax_points = info.tax_points;
        self.honor = info.honor;
        self.virtue = info.virtue;
        self.emblem_version = info.emblem_version;
        self.name = info.name;
        self.master_name = info.master_name;
        self.territory = info.territory;
        self.master_character_id = info.master_character_id;
        self.refresh_labels();
    }

    fn drop_roster(&mut self) {
        self.members.clear();
        self.positions.clear();
        self.relations.clear();
        self.skills.clear();
        self.skill_points = 0;
        self.subject.clear();
        self.notice.clear();
    }

    pub fn set_members(&mut self, members: Vec<GuildMemberSnapshot>) {
        self.members = members.into_iter().map(member_from_snapshot).collect();
        self.refresh_labels();
    }

    pub fn add_member(&mut self, member: GuildMemberSnapshot) {
        if let Some(existing) = self
            .members
            .iter_mut()
            .find(|existing| existing.character_id == member.character_id)
        {
            *existing = member_from_snapshot(member);
        } else {
            self.members.push(member_from_snapshot(member));
        }
        self.refresh_labels();
    }

    pub fn set_member_position(&mut self, account_id: AccountId, character_id: CharacterId, position_id: i32) {
        if let Some(member) = self
            .members
            .iter_mut()
            .find(|member| member.account_id == account_id && member.character_id == character_id)
        {
            member.position_id = position_id;
            self.refresh_labels();
        }
    }

    pub fn set_positions(&mut self, positions: Vec<(i32, String)>) {
        let old = std::mem::take(&mut self.positions);
        self.positions = positions
            .into_iter()
            .map(|(id, name)| {
                let previous = old.iter().find(|position| position.id == id);
                GuildPosition {
                    id,
                    name,
                    mode: previous.map(|position| position.mode).unwrap_or(0),
                    ranking: previous.map(|position| position.ranking).unwrap_or(id),
                    pay_rate: previous.map(|position| position.pay_rate).unwrap_or(0),
                    label: String::new(),
                }
            })
            .collect();
        self.refresh_labels();
    }

    pub fn set_position_rights(&mut self, rights: Vec<(i32, i32, i32, i32)>) {
        for (id, mode, ranking, pay_rate) in rights {
            if let Some(position) = self.positions.iter_mut().find(|position| position.id == id) {
                position.mode = mode;
                position.ranking = ranking;
                position.pay_rate = pay_rate;
            } else {
                self.positions.push(GuildPosition {
                    id,
                    name: String::new(),
                    mode,
                    ranking,
                    pay_rate,
                    label: String::new(),
                });
            }
        }
        self.refresh_labels();
    }

    pub fn apply_position_changed(&mut self, id: i32, mode: i32, ranking: i32, pay_rate: i32, name: String) {
        if let Some(position) = self.positions.iter_mut().find(|position| position.id == id) {
            position.mode = mode;
            position.ranking = ranking;
            position.pay_rate = pay_rate;
            if !name.is_empty() {
                position.name = name;
            }
        } else {
            self.positions.push(GuildPosition {
                id,
                name,
                mode,
                ranking,
                pay_rate,
                label: String::new(),
            });
        }
        self.refresh_labels();
    }

    pub fn set_relations(&mut self, relations: Vec<(i32, u32, String)>) {
        self.relations = relations
            .into_iter()
            .map(|(opposition, guild_id, name)| GuildRelation {
                guild_id,
                opposition,
                name,
                label: String::new(),
            })
            .collect();
        self.refresh_labels();
    }

    pub fn add_relation(&mut self, opposition: i32, guild_id: u32, name: String) {
        if let Some(existing) = self.relations.iter_mut().find(|relation| relation.guild_id == guild_id) {
            existing.opposition = opposition;
            existing.name = name;
        } else {
            self.relations.push(GuildRelation {
                guild_id,
                opposition,
                name,
                label: String::new(),
            });
        }
        self.refresh_labels();
    }

    pub fn remove_relation(&mut self, guild_id: u32, opposition: i32) {
        let before = self.relations.len();
        self.relations
            .retain(|relation| !(relation.guild_id == guild_id && relation.opposition == opposition));
        if self.relations.len() == before {
            self.relations.retain(|relation| relation.guild_id != guild_id);
        }
        self.refresh_labels();
    }

    pub fn set_skills(&mut self, skill_points: i32, skills: Vec<GuildSkillSnapshot>) {
        let previous = std::mem::take(&mut self.skills);
        self.skill_points = skill_points;
        self.skills = skills
            .into_iter()
            .map(|skill| {
                let previous = previous.iter().find(|old| old.skill_id == skill.skill_id);
                GuildSkill {
                    skill_id: skill.skill_id,
                    level: skill.level,
                    sp: skill.sp,
                    range: skill.range,
                    name: skill.name,
                    upgradable: skill.upgradable,
                    label: String::new(),
                    sprite: previous.and_then(|old| old.sprite.clone()),
                    actions: previous.and_then(|old| old.actions.clone()),
                    animation_state: previous
                        .map(|old| old.animation_state.clone())
                        .unwrap_or_else(|| SpriteAnimationState::new(ClientTick(0))),
                }
            })
            .collect();
        self.refresh_labels();
    }

    pub fn attach_skill_sprite(&mut self, skill_id: u16, sprite: Arc<Sprite>) {
        if let Some(skill) = self.skills.iter_mut().find(|skill| skill.skill_id == skill_id) {
            skill.sprite = Some(sprite);
        }
    }

    pub fn attach_skill_actions(&mut self, skill_id: u16, actions: Arc<Actions>) {
        if let Some(skill) = self.skills.iter_mut().find(|skill| skill.skill_id == skill_id) {
            skill.actions = Some(actions);
        }
    }

    pub fn apply_skill_update(&mut self, skill_id: u16, level: u16, sp: u16, upgradable: bool) {
        if skill_id < 10000 {
            return;
        }
        if let Some(skill) = self.skills.iter_mut().find(|skill| skill.skill_id == skill_id) {
            if level > skill.level && self.skill_points > 0 {
                self.skill_points -= 1;
            }
            skill.level = level;
            skill.sp = sp;
            skill.upgradable = upgradable;
            self.refresh_labels();
        }
    }

    pub fn set_notice(&mut self, subject: String, notice: String) {
        self.subject = subject;
        self.notice = notice;
        self.refresh_labels();
    }

    pub fn set_member_online(&mut self, account_id: AccountId, character_id: CharacterId, online: bool) {
        if let Some(member) = self
            .members
            .iter_mut()
            .find(|member| member.account_id == account_id && member.character_id == character_id)
        {
            if member.online != online {
                if online {
                    self.online_members = self.online_members.saturating_add(1);
                } else {
                    self.online_members = (self.online_members - 1).max(0);
                }
                member.online = online;
            }
            self.refresh_labels();
        }
    }

    /// Removes a member and returns their name when the roster had that
    /// character.
    pub fn remove_member(&mut self, character_id: CharacterId) -> Option<String> {
        let index = self.members.iter().position(|member| member.character_id == character_id)?;
        let removed = self.members.remove(index);
        if removed.online {
            self.online_members = (self.online_members - 1).max(0);
        }
        self.refresh_labels();
        Some(removed.name)
    }

    /// Applies a leave or expulsion notice (`0x0A83` / `0x0A82`).
    ///
    /// Those packets carry a character id and no name. The member list is sent
    /// only after menu 1, so the character may be missing from the roster.
    /// Hercules then drops `sd->guild` without a zero affiliation. Returns true
    /// when `character_id` is the local character and the guild was cleared.
    pub fn apply_member_removed(&mut self, character_id: CharacterId, self_character_id: CharacterId) -> bool {
        let _ = self.remove_member(character_id);
        if character_id == self_character_id {
            self.clear();
            true
        } else {
            false
        }
    }
}

fn member_from_snapshot(member: GuildMemberSnapshot) -> GuildMember {
    GuildMember {
        account_id: member.account_id,
        character_id: member.character_id,
        name: member.name,
        job: member.job,
        level: member.level,
        position_id: member.position_id,
        online: member.online,
        contribution: member.contribution,
        label: String::new(),
    }
}

fn member_label(member: &GuildMember, positions: &[GuildPosition]) -> String {
    let position = positions
        .iter()
        .find(|position| position.id == member.position_id)
        .map(|position| position.name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Member");
    let presence = if member.online { "Online" } else { "Offline" };
    format!(
        "{}  {}  {}  Lv{}  {}  devotion {}",
        member.name,
        position,
        job_label(member.job),
        member.level,
        presence,
        member.contribution
    )
}

fn permission_summary(mode: i32) -> String {
    let mut parts = Vec::new();
    if mode & GPERM_INVITE != 0 {
        parts.push("invite");
    }
    if mode & GPERM_EXPEL != 0 {
        parts.push("expel");
    }
    if mode & GPERM_STORAGE != 0 {
        parts.push("storage");
    }
    if parts.is_empty() {
        "no extra rights".to_owned()
    } else {
        parts.join(", ")
    }
}

fn info_rows(guild: &GuildState) -> Vec<String> {
    let allies = guild.relations.iter().filter(|relation| relation.opposition == 0).count();
    let enemies = guild.relations.len() - allies;
    vec![
        format!("Name: {}", guild.name),
        format!("Master: {}", guild.master_name),
        format!("Level: {}", guild.level),
        format!("Online: {} / {}", guild.online_members, guild.max_members),
        format!("Average level: {}", guild.average_level),
        format!("Experience: {} / {}", guild.experience, guild.next_experience),
        format!("Tax points: {}", guild.tax_points),
        format!("Honor: {}", guild.honor),
        format!("Virtue: {}", guild.virtue),
        format!("Territory: {}", guild.territory),
        format!("Emblem: {}", guild.emblem_version),
        format!("Alliances: {allies}    Enemies: {enemies}"),
        "Guild storage opens in the storage window when the server sends it.".to_owned(),
    ]
}

pub fn job_label(job: i16) -> String {
    let known = match job {
        0 => Some("Novice"),
        1 => Some("Swordman"),
        2 => Some("Mage"),
        3 => Some("Archer"),
        4 => Some("Acolyte"),
        5 => Some("Merchant"),
        6 => Some("Thief"),
        7 => Some("Knight"),
        8 => Some("Priest"),
        9 => Some("Wizard"),
        10 => Some("Blacksmith"),
        11 => Some("Hunter"),
        12 => Some("Assassin"),
        14 => Some("Crusader"),
        15 => Some("Monk"),
        16 => Some("Sage"),
        17 => Some("Rogue"),
        18 => Some("Alchemist"),
        19 => Some("Bard"),
        20 => Some("Dancer"),
        23 => Some("Super Novice"),
        24 => Some("Gunslinger"),
        25 => Some("Ninja"),
        4008 => Some("Lord Knight"),
        4009 => Some("High Priest"),
        4010 => Some("High Wizard"),
        4011 => Some("Whitesmith"),
        4012 => Some("Sniper"),
        4013 => Some("Assassin Cross"),
        4054 => Some("Rune Knight"),
        4055 => Some("Warlock"),
        4056 => Some("Ranger"),
        4057 => Some("Archbishop"),
        4058 => Some("Mechanic"),
        4059 => Some("Guillotine Cross"),
        4066 => Some("Royal Guard"),
        4067 => Some("Sorcerer"),
        4068 => Some("Minstrel"),
        4069 => Some("Wanderer"),
        4070 => Some("Sura"),
        4071 => Some("Genetic"),
        4072 => Some("Shadow Chaser"),
        _ => None,
    };
    match known {
        Some(name) => name.to_owned(),
        None => format!("Job {job}"),
    }
}

pub fn make_guild_result_text(result: u8) -> String {
    match result {
        0 => "Guild created.".to_owned(),
        1 => "You are already in a guild.".to_owned(),
        2 => "That guild name already exists.".to_owned(),
        3 => "You need an Emperium to create a guild.".to_owned(),
        4 => "You cannot create a guild on this map.".to_owned(),
        other => format!("Guild creation result {other}."),
    }
}

pub fn invite_result_text(result: u8) -> String {
    match result {
        0 => "That player is already in a guild.".to_owned(),
        1 => "The guild offer was rejected.".to_owned(),
        2 => "The guild offer was accepted.".to_owned(),
        3 => "The guild is full.".to_owned(),
        4 => "That player is offline.".to_owned(),
        other => format!("Guild invite result {other}."),
    }
}

pub fn with_permission_bits(mode: i32, invite: bool, expel: bool, storage: bool) -> i32 {
    let mode = if invite { mode | GPERM_INVITE } else { mode & !GPERM_INVITE };
    let mode = if expel { mode | GPERM_EXPEL } else { mode & !GPERM_EXPEL };
    if storage { mode | GPERM_STORAGE } else { mode & !GPERM_STORAGE }
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

    fn info(guild_id: u32, name: &str) -> GuildInfoUpdate {
        GuildInfoUpdate {
            guild_id,
            level: 1,
            online_members: 1,
            max_members: 16,
            average_level: 10,
            experience: 0,
            next_experience: 1,
            tax_points: 0,
            honor: 0,
            virtue: 0,
            emblem_version: 0,
            name: name.to_owned(),
            master_name: "Lead".to_owned(),
            territory: String::new(),
            master_character_id: 1,
        }
    }

    #[test]
    fn affiliation_zero_clears_the_roster_and_keeps_emblems() {
        let mut guild = GuildState::default();
        guild.apply_affiliation(4, "Wolves".to_owned(), true, 9, 2);
        guild.set_members(vec![member(9, "Alpha")]);
        guild.store_emblem(8, 1, DecodedEmblem {
            width: 1,
            height: 1,
            pixels: vec![1, 2, 3, 255],
        });
        guild.apply_affiliation(0, String::new(), false, 0, 0);
        assert!(!guild.joined);
        assert!(guild.members.is_empty());
        assert!(guild.name.is_empty());
        assert_eq!(guild.emblem(8).map(|emblem| emblem.version), Some(1));
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
            tax_points: 7,
            honor: 1,
            virtue: -2,
            emblem_version: 0,
            name: "Wolves".to_owned(),
            master_name: "Alpha".to_owned(),
            territory: "Prontera".to_owned(),
            master_character_id: 9,
        });
        guild.set_members(vec![member(9, "Alpha"), member(8, "Beta")]);
        guild.online_members = 2;
        guild.set_member_online(AccountId(8), CharacterId(8), false);
        assert!(!guild.members[1].online);
        assert_eq!(guild.online_members, 1);
        assert!(guild.members[1].label.contains("Offline"));
        assert!(guild.rows.iter().any(|row| row.contains("Tax points: 7")));
        guild.set_member_online(AccountId(8), CharacterId(8), false);
        assert_eq!(guild.online_members, 1);
        assert_eq!(guild.remove_member(CharacterId(9)).as_deref(), Some("Alpha"));
        assert_eq!(guild.members.len(), 1);
        assert_eq!(guild.online_members, 0);
        assert!(guild.remove_member(CharacterId(9)).is_none());
        assert_eq!(guild.online_members, 0);
    }

    #[test]
    fn leaving_or_expulsion_clears_the_local_guild_without_a_roster_hit() {
        let mut guild = GuildState::default();
        guild.apply_affiliation(4, "Wolves".to_owned(), true, 9, 2);
        guild.store_emblem(8, 1, DecodedEmblem {
            width: 1,
            height: 1,
            pixels: vec![1, 2, 3, 255],
        });
        guild.set_members(vec![member(8, "Beta")]);

        assert!(!guild.apply_member_removed(CharacterId(8), CharacterId(9)));
        assert!(guild.joined);
        assert!(guild.members.is_empty());
        assert_eq!(guild.name, "Wolves");

        // The notice names the local character, who was never in the roster.
        assert!(guild.apply_member_removed(CharacterId(9), CharacterId(9)));
        assert!(!guild.joined);
        assert!(guild.name.is_empty());
        assert_eq!(guild.guild_id, 0);
        assert_eq!(guild.emblem(8).map(|emblem| emblem.version), Some(1));

        guild.apply_affiliation(4, "Wolves".to_owned(), true, 9, 2);
        guild.set_members(vec![member(9, "Alpha"), member(8, "Beta")]);
        assert!(guild.apply_member_removed(CharacterId(9), CharacterId(9)));
        assert!(!guild.joined);
        assert!(guild.members.is_empty());
        assert_eq!(guild.emblem(8).map(|emblem| emblem.version), Some(1));
    }

    #[test]
    fn guild_skill_icon_path_uses_the_resource_file() {
        assert_eq!(guild_skill_icon_path("NV_BASIC"), "아이템\\NV_BASIC");
    }

    #[test]
    fn info_for_a_new_guild_drops_the_previous_notice() {
        let mut guild = GuildState::default();
        guild.apply_info(info(1, "Old"));
        guild.set_notice("Old subject".to_owned(), "Old notice".to_owned());
        let mut next = info(2, "New");
        next.emblem_version = 3;
        next.master_character_id = 5;
        guild.apply_info(next);
        assert!(guild.subject.is_empty());
        assert!(guild.notice.is_empty());
        guild.apply_affiliation(2, "New".to_owned(), true, 5, 3);
        assert!(guild.subject.is_empty());
        assert!(guild.notice.is_empty());
    }

    #[test]
    fn switching_guild_drops_the_previous_roster() {
        let mut guild = GuildState::default();
        guild.apply_affiliation(1, "Old".to_owned(), false, 1, 0);
        guild.set_members(vec![member(1, "Old")]);
        let mut next = info(2, "New");
        next.emblem_version = 3;
        next.master_character_id = 5;
        guild.apply_info(next);
        assert!(guild.members.is_empty());
        assert_eq!(guild.name, "New");
        assert!(!guild.is_master);
    }

    #[test]
    fn position_names_and_rights_merge() {
        let mut guild = GuildState::default();
        guild.set_positions(vec![(0, "Master".to_owned()), (1, "Raider".to_owned())]);
        guild.set_position_rights(vec![(1, GPERM_INVITE | GPERM_EXPEL, 1, 12)]);
        assert_eq!(guild.positions[1].name, "Raider");
        assert_eq!(guild.positions[1].pay_rate, 12);
        assert_eq!(guild.positions[0].name, "Master");
        guild.set_positions(vec![(0, "Leader".to_owned()), (1, "Raider".to_owned())]);
        assert_eq!(guild.positions[1].mode, GPERM_INVITE | GPERM_EXPEL);
        assert_eq!(guild.positions[0].name, "Leader");
        guild.apply_position_changed(1, GPERM_STORAGE, 1, 20, "Scout".to_owned());
        assert_eq!(guild.positions[1].name, "Scout");
        assert_eq!(guild.positions[1].pay_rate, 20);
    }

    #[test]
    fn permissions_follow_the_master_flag_or_the_position_mode() {
        let mut guild = GuildState::default();
        guild.apply_affiliation(1, "Wolves".to_owned(), false, 1, 0);
        let mut beta = member(8, "Beta");
        beta.position_id = 1;
        guild.set_members(vec![member(1, "Alpha"), beta]);
        guild.set_position_rights(vec![(1, GPERM_INVITE, 1, 0)]);
        assert!(guild.can("Beta", GPERM_INVITE));
        assert!(!guild.can("Beta", GPERM_EXPEL));
        assert!(!guild.can("Missing", GPERM_INVITE));
        guild.is_master = true;
        assert!(guild.can("Beta", GPERM_EXPEL));
    }

    #[test]
    fn relations_split_allies_and_enemies_and_skills_replace() {
        let mut guild = GuildState::default();
        guild.set_relations(vec![(0, 4, "Archers".to_owned()), (1, 5, "Axes".to_owned())]);
        assert!(guild.relations[0].label.starts_with("Ally"));
        assert!(guild.relations[1].label.starts_with("Enemy"));
        guild.remove_relation(5, 1);
        assert_eq!(guild.relations.len(), 1);
        guild.add_relation(0, 6, "Mages".to_owned());
        assert_eq!(guild.relations.len(), 2);
        guild.set_skills(2, vec![GuildSkillSnapshot {
            skill_id: 10000,
            level: 1,
            sp: 0,
            range: 1,
            name: "Emergency Call".to_owned(),
            upgradable: true,
        }]);
        assert_eq!(guild.skill_points_label, "Skill points: 2");
        guild.apply_skill_update(10000, 2, 10, false);
        assert_eq!(guild.skills[0].level, 2);
        assert!(!guild.skills[0].upgradable);
        assert_eq!(guild.skill_points, 1);
        guild.apply_skill_update(50, 9, 9, true);
        assert_eq!(guild.skills[0].level, 2);
    }

    #[test]
    fn member_position_and_job_label() {
        let mut guild = GuildState::default();
        let mut knight = member(3, "Cara");
        knight.job = 7;
        knight.level = 55;
        guild.set_members(vec![knight]);
        assert!(guild.members[0].label.contains("Knight"));
        assert!(guild.members[0].label.contains("Lv55"));
        guild.set_member_position(AccountId(3), CharacterId(3), 2);
        assert_eq!(guild.members[0].position_id, 2);
        assert_eq!(job_label(999), "Job 999");
        assert_eq!(make_guild_result_text(3), "You need an Emperium to create a guild.");
        assert_eq!(invite_result_text(4), "That player is offline.");
        assert_eq!(with_permission_bits(0xFFFF, false, true, false) & GPERM_INVITE, 0);
        assert_ne!(with_permission_bits(0, true, false, true) & GPERM_STORAGE, 0);
    }

    #[test]
    fn emblem_chunks_assemble_and_a_complete_packet_decodes() {
        let mut guild = GuildState::default();
        let bmp = guild_emblem::encode_bmp_24(1, 1, &[255, 0, 0, 255]);
        let compressed = guild_emblem::zlib_compress(&bmp);
        let mid = compressed.len() / 2;
        assert!(!guild.push_emblem_chunk(0, 9, 2, &[]));
        assert!(!guild.push_emblem_chunk(1, 9, 2, &compressed[..mid]));
        assert!(!guild.push_emblem_chunk(1, 9, 2, &compressed[mid..]));
        assert!(guild.push_emblem_chunk(2, 9, 2, &[]));
        let emblem = guild.emblem(9).unwrap();
        assert_eq!(emblem.version, 2);
        assert_eq!(&emblem.pixels[0..4], &[255, 0, 0, 255]);
        assert!(!guild.wants_emblem(9, 2));
        assert!(guild.wants_emblem(9, 3));
        guild.note_emblem_request(9, 3);
        assert!(!guild.wants_emblem(9, 3));
    }
}
