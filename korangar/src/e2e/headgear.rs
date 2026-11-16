//! World headgear in logic mode.
//!
//! [`HeadgearScene`] keeps player looks the way the client does: an
//! `AddEntity` for a player-type job stores its hair and the three headgear
//! view ids ([`PlayerLook::from_entity_data`]), and `ChangeHair` /
//! `ChangeHead{Top,Mid,Bottom}` go through [`PlayerLook::apply`], the same
//! call `Entity::apply_player_look` makes before the client reloads the
//! sprite. Part files come from [`player_look_part_files`], the function
//! `Common::get_entity_part_files` uses, and [`resolve_part_files`] is the
//! animation loader's optional-part step. Logic mode has no renderer, so the
//! result is the list of sprite paths that would be composed.

use hashbrown::HashMap;
use korangar_networking::{EntityData, NetworkEvent};
use ragnarok_packets::{CharacterInformation, JobId, Sex, SpriteChangeType};

use crate::world::{EntityType, PlayerLook, parse_accessory_names, player_look_part_files, resolve_part_files};

/// One player-type entity as the scene tracks it.
#[derive(Clone, Debug, PartialEq)]
pub struct LookedPlayer {
    pub entity_id: u32,
    pub job_id: JobId,
    pub sex: Sex,
    pub look: PlayerLook,
}

#[derive(Default)]
pub struct HeadgearScene {
    accessory_names: HashMap<u16, String>,
    players: Vec<LookedPlayer>,
    characters: Vec<LookedPlayer>,
    reloads: Vec<u32>,
}

impl HeadgearScene {
    /// Scene backed by `accessoryid.lub` / `accname.lub` chunks, the table
    /// the client library loads.
    pub fn with_tables(accessory_id: &[u8], accname: &[u8]) -> Result<Self, String> {
        let accessory_names = parse_accessory_names(accessory_id, accname).map_err(|error| format!("accname tables: {error}"))?;
        Ok(Self {
            accessory_names,
            ..Self::default()
        })
    }

    pub fn accessory_names(&self) -> &HashMap<u16, String> {
        &self.accessory_names
    }

    /// Mirror of the client's `AddEntity` and look-change match arms.
    pub fn observe(&mut self, event: &NetworkEvent) {
        match event {
            NetworkEvent::AddEntity { entity_data } => self.on_appear(entity_data),
            NetworkEvent::CharacterList { characters } => self.on_character_list(characters),
            NetworkEvent::ChangeHair { account_id, hair_id } => self.on_look(account_id.0, &SpriteChangeType::Hair, *hair_id),
            NetworkEvent::ChangeHeadTop { account_id, view_id } => self.on_look(account_id.0, &SpriteChangeType::HeadTop, *view_id),
            NetworkEvent::ChangeHeadMid { account_id, view_id } => self.on_look(account_id.0, &SpriteChangeType::HeadMiddle, *view_id),
            NetworkEvent::ChangeHeadBottom { account_id, view_id } => self.on_look(account_id.0, &SpriteChangeType::HeadBottom, *view_id),
            _ => {}
        }
    }

    fn on_appear(&mut self, entity_data: &EntityData) {
        if EntityType::from(entity_data.job_id) != EntityType::Player {
            return;
        }
        let entity_id = entity_data.entity_id.0;
        // The client replaces an entity that appears twice.
        self.players.retain(|player| player.entity_id != entity_id);
        self.players.push(LookedPlayer {
            entity_id,
            job_id: entity_data.job_id,
            sex: entity_data.sex,
            look: PlayerLook::from_entity_data(entity_data),
        });
    }

    fn on_character_list(&mut self, characters: &[CharacterInformation]) {
        self.characters = characters
            .iter()
            .map(|character| {
                let data = EntityData::from_character(
                    ragnarok_packets::AccountId(character.character_id.0),
                    character,
                    ragnarok_packets::WorldPosition::origin(),
                );
                LookedPlayer {
                    entity_id: character.character_id.0,
                    job_id: character.job_id,
                    sex: character.sex,
                    look: PlayerLook::from_entity_data(&data),
                }
            })
            .collect();
    }

    fn on_look(&mut self, account_id: u32, look: &SpriteChangeType, value: u32) {
        // A look for an account that is not on screen is ignored, like the
        // client does.
        let Some(player) = self.players.iter_mut().find(|player| player.entity_id == account_id) else {
            return;
        };
        if player.look.apply(look, value) {
            self.reloads.push(account_id);
        }
    }

    pub fn players(&self) -> &[LookedPlayer] {
        &self.players
    }

    /// Characters from the last character list (own account).
    pub fn characters(&self) -> &[LookedPlayer] {
        &self.characters
    }

    /// Entity ids whose sprite would reload, in order.
    pub fn reloads(&self) -> &[u32] {
        &self.reloads
    }

    pub fn part_files_for(&self, player: &LookedPlayer) -> Vec<String> {
        player_look_part_files(player.job_id, player.sex, &player.look, |view_id| {
            self.accessory_names.get(&view_id).cloned()
        })
    }

    /// Part files the animation loader would request for `entity_id`.
    pub fn part_files(&self, entity_id: u32) -> Option<Vec<String>> {
        self.players
            .iter()
            .find(|player| player.entity_id == entity_id)
            .map(|player| self.part_files_for(player))
    }

    /// Part files after the loader drops headgear without sprite files.
    /// `exists` answers for a path without its extension.
    pub fn composed_parts(&self, entity_id: u32, exists: impl Fn(&str) -> bool) -> Option<Vec<String>> {
        self.part_files(entity_id).map(|files| resolve_part_files(&files, exists))
    }
}
