mod accessory_name;
mod achievement_table;
mod baby_job;
mod item_info;
mod item_name;
mod item_resource;
mod job_identity;
mod lua;
mod map_display_name;
mod map_sky_data;
mod monster_name;
mod navigation_table;
mod quest_display;
mod robe_name;
mod signboard;
mod skill_description;
mod skill_information;
mod skill_requirements;
mod skill_tree;
mod status_icon;
mod status_icon_builtin;
mod title_table;

use std::hash::Hash;
use std::sync::Arc;

use hashbrown::HashMap;
use korangar_loaders::FileLoader;
use mlua::{ErrorContext, Lua};
use ragnarok_packets::{ItemId, JobId, SkillId};

pub use self::accessory_name::parse_accessory_names;
#[cfg(test)]
pub use self::achievement_table::AttendanceTableReward;
pub use self::achievement_table::{
    AchievementCatalog, AchievementInfo, AchievementReward, AttendanceTable, merge_achievement_table, parse_attendance_chunk,
};
pub use self::baby_job::IsBabyJob;
pub use self::item_info::ItemInfo;
pub(crate) use self::item_info::{CatalogItemIdentity, EnglishItemIdentity, catalog_item_identity, english_item_identity};
pub use self::item_name::{ItemName, ItemNameKey};
pub use self::item_resource::{ItemResource, ItemResourceKey, classify_item_icon_failure, inventory_icon_texture_path};
pub use self::job_identity::JobIdentity;
pub(crate) use self::lua::execute_chunk as execute_lua_chunk;
use self::lua::{decode_game_bytes, execute_chunk, game_string};
pub use self::map_display_name::{display_name_from_table, load_map_display_names};
pub use self::map_sky_data::MapSkyData;
use self::monster_name::load_monster_names;
pub use self::navigation_table::{NaviLink, NavigationTables, navigation_tables_from_source};
#[cfg(test)]
pub use self::navigation_table::{NaviMob, NaviNpc};
#[allow(unused_imports)]
pub use self::quest_display::QuestRewardItem;
use self::quest_display::load_quest_displays;
pub use self::quest_display::{QuestDisplay, QuestTextSource, format_quest_rewards, format_quest_timer, quest_is_cooldown};
pub use self::signboard::MapSignboards;
use self::skill_description::filter_job_prefixed_lines;
pub(crate) use self::skill_description::load_skill_text;
pub use self::skill_description::{JobParents, compose_skill_description, shown_skill_level, skill_hover_text};
pub use self::skill_tree::SkillTreeLayout;
pub use self::status_icon::{StatusIconInfo, parse_status_icons, with_builtin_icons};
pub use self::title_table::parse_title_table;
use crate::loaders::GameFileLoader;
pub use crate::world::library::skill_information::SkillListInformation;
pub use crate::world::library::skill_requirements::{SkillListKey, SkillListRequirements};

pub struct Library {
    job_identity_table: <JobIdentity as Table>::Storage,
    item_info_table: <ItemInfo as Table>::Storage,
    map_sky_data_table: <MapSkyData as Table>::Storage,
    skill_information_table: <SkillListInformation as Table>::Storage,
    skill_requirements_table: <SkillListRequirements as Table>::Storage,
    skill_tree_table: <SkillTreeLayout as Table>::Storage,
    baby_job_table: <IsBabyJob as Table>::Storage,
    signboard_table: <MapSignboards as Table>::Storage,
    map_display_names: HashMap<String, String>,
    quest_displays: HashMap<u32, QuestDisplay>,
    monster_names: HashMap<u32, String>,
    skill_descriptions: HashMap<u16, String>,
    job_parents: JobParents,
    accessory_names: HashMap<u16, String>,
    robe_names: HashMap<u16, String>,
    achievement_catalog: Arc<AchievementCatalog>,
    attendance_table: Arc<AttendanceTable>,
    navigation_tables: Arc<NavigationTables>,
    status_icons: HashMap<u16, StatusIconInfo>,
    title_names: HashMap<u32, String>,
}

impl Library {
    pub fn new(game_file_loader: &GameFileLoader) -> mlua::Result<Self> {
        let job_identity_table = JobIdentity::load(game_file_loader).context("loading JobIdentity")?;
        let item_info_table = ItemInfo::load(game_file_loader).context("loading ItemInfo")?;
        let map_sky_data_table = MapSkyData::load(game_file_loader).context("loading MapSkyData")?;
        let skill_information_table = SkillListInformation::load(game_file_loader).context("loading SkillListInformation")?;
        let skill_requirements_table = SkillListRequirements::load(game_file_loader).context("loading SkillListRequirements")?;
        let skill_tree_table = SkillTreeLayout::load(game_file_loader).context("loading SkillTreeLayout")?;
        let baby_job_table = IsBabyJob::load(game_file_loader).context("loading IsBabyJob")?;
        let signboard_table = MapSignboards::load(game_file_loader).context("loading MapSignboards")?;
        let map_display_names = load_map_display_names(game_file_loader);
        // Missing quest and skill text files leave these empty. They must not fail
        // startup.
        let quest_displays = load_quest_displays(game_file_loader);
        let monster_names = load_monster_names(game_file_loader);
        let (skill_descriptions, job_parents) = load_skill_text(game_file_loader);
        let accessory_names = accessory_name::load_accessory_names(game_file_loader);
        let robe_names = robe_name::load_robe_names(game_file_loader);
        // Achievement, attendance and navigation tables are optional client files.
        let achievement_catalog = Arc::new(achievement_table::load_achievement_catalog(game_file_loader));
        let attendance_table = Arc::new(achievement_table::load_attendance_table(game_file_loader));
        let mut navigation_tables = navigation_table::load_navigation_tables(game_file_loader);
        navigation_table::apply_monster_names(&mut navigation_tables, &monster_names);
        let navigation_tables = Arc::new(navigation_tables);
        let status_icons = status_icon::load_status_icons(game_file_loader);
        let title_names = title_table::load_title_table(game_file_loader);

        Ok(Self {
            job_identity_table,
            item_info_table,
            map_sky_data_table,
            skill_information_table,
            skill_requirements_table,
            skill_tree_table,
            baby_job_table,
            signboard_table,
            map_display_names,
            quest_displays,
            monster_names,
            skill_descriptions,
            job_parents,
            accessory_names,
            robe_names,
            achievement_catalog,
            attendance_table,
            navigation_tables,
            status_icons,
            title_names,
        })
    }

    /// Filtered skill description body for one job. Level and SP are added
    /// later.
    pub fn skill_description_body(&self, skill_id: SkillId, job_id: JobId) -> String {
        let raw = self.skill_descriptions.get(&skill_id.0).map(String::as_str).unwrap_or("");
        filter_job_prefixed_lines(raw, job_id.0, &self.job_parents)
    }

    /// `AccNameTable` suffix for a headgear view id, such as `_고글`.
    pub fn accessory_sprite_suffix(&self, view_id: u16) -> Option<&str> {
        self.accessory_names.get(&view_id).map(String::as_str)
    }

    /// `RobeNameTable` folder for a garment view id, such as `천사날개`.
    pub fn robe_sprite_name(&self, view_id: u16) -> Option<&str> {
        self.robe_names.get(&view_id).map(String::as_str)
    }

    /// `titletable.lub` name for an achievement title reward.
    pub fn title_name(&self, title_id: u32) -> Option<&str> {
        self.title_names.get(&title_id).map(String::as_str)
    }

    pub fn achievement_catalog(&self) -> Arc<AchievementCatalog> {
        Arc::clone(&self.achievement_catalog)
    }

    pub fn attendance_table(&self) -> Arc<AttendanceTable> {
        Arc::clone(&self.attendance_table)
    }

    pub fn navigation_tables(&self) -> Arc<NavigationTables> {
        Arc::clone(&self.navigation_tables)
    }

    /// Icon file and tooltip for an EFST id from `stateicon/*.lub`, with the
    /// built-in icon files for older statuses.
    pub fn status_icon(&self, status_index: u16) -> Option<&StatusIconInfo> {
        self.status_icons.get(&status_index)
    }

    pub fn map_display_name(&self, internal: &str) -> String {
        display_name_from_table(&self.map_display_names, internal)
    }

    pub fn item_description(&self, item_id: ragnarok_packets::ItemId, identified: bool) -> String {
        ItemInfo::try_get(self, item_id)
            .map(|info| {
                let lines = if identified {
                    info.identified_description()
                } else {
                    info.unidentified_description()
                };
                lines.join("\n")
            })
            .unwrap_or_default()
    }

    #[inline(always)]
    pub fn get<T: Table>(&self, key: T::Key<'_>) -> &T {
        T::get(self, key)
    }
}

impl QuestTextSource for Library {
    fn quest_display(&self, quest_id: u32) -> Option<&QuestDisplay> {
        self.quest_displays.get(&quest_id)
    }

    fn monster_name(&self, mob_id: u32) -> Option<&str> {
        self.monster_names.get(&mob_id).map(String::as_str)
    }

    fn item_label(&self, item_id: u32) -> String {
        let name = self
            .get::<ItemName>(ItemNameKey {
                item_id: ItemId(item_id),
                is_identified: true,
            })
            .to_string();
        if name == "NOTFOUND" { format!("Item {item_id}") } else { name }
    }
}

/// Trait for compacting a hash map after it is completely populated.
trait HashMapExt {
    /// Compact the hash map, possibly by creating a second one.
    fn compact(self) -> Self;
}

impl<K, V> HashMapExt for HashMap<K, V>
where
    K: Eq + Hash,
{
    fn compact(self) -> Self {
        HashMap::from_iter(self)
    }
}

trait LuaExt: Sized {
    fn load_from_game_files(game_file_loader: &impl FileLoader, files: &[&str]) -> mlua::Result<Self>;

    fn load_from_game_files_with_fallbacks(game_file_loader: &impl FileLoader, file_groups: &[&[&str]]) -> mlua::Result<Self>;
}

impl LuaExt for Lua {
    fn load_from_game_files(game_file_loader: &impl FileLoader, files: &[&str]) -> mlua::Result<Self> {
        // Each path is its own single-candidate group.
        let groups: Vec<[&str; 1]> = files.iter().map(|file| [*file]).collect();
        let group_refs: Vec<&[&str]> = groups.iter().map(|group| group.as_slice()).collect();
        Self::load_from_game_files_with_fallbacks(game_file_loader, &group_refs)
    }

    /// For each group, use the first candidate that both reads and executes.
    /// A Turoran overlay that exists but errors (missing `SKID`/`JOBID`, bad
    /// syntax) is discarded and the next name in the group is tried. Probing
    /// happens on a fresh state rebuilt from earlier successful groups, so a
    /// failed chunk cannot leave partial globals behind.
    fn load_from_game_files_with_fallbacks(game_file_loader: &impl FileLoader, file_groups: &[&[&str]]) -> mlua::Result<Self> {
        let mut state = Lua::new();
        // Owned copies of chunks that executed. Replayed into each probe so a
        // later candidate still sees `SKID` / `JOBID` from earlier groups.
        let mut committed: Vec<(String, Vec<u8>)> = Vec::new();

        for group in file_groups {
            let mut last_error: Option<String> = None;
            let mut chosen: Option<(String, Vec<u8>, Lua)> = None;

            for file in *group {
                let data = match game_file_loader.get(file) {
                    Ok(data) => data,
                    Err(error) => {
                        last_error = Some(format!("{error:?}"));
                        continue;
                    }
                };

                match replay_and_execute(&committed, file, &data) {
                    Ok(probe) => {
                        chosen = Some(((*file).to_owned(), data, probe));
                        break;
                    }
                    Err(error) => {
                        last_error = Some(error.to_string());
                    }
                }
            }

            let Some((name, data, probe)) = chosen else {
                let file = group.last().copied().unwrap_or("<empty group>");
                let error = last_error.unwrap_or_else(|| "no candidates".to_owned());
                return Err(mlua::Error::RuntimeError(format!(
                    "Cannot read Lua asset {file} (also tried {} fallbacks): {error}",
                    group.len().saturating_sub(1)
                )));
            };
            state = probe;
            committed.push((name, data));
        }

        Ok(state)
    }
}

/// Build a new state from chunks that already succeeded, then run `data`.
/// `Err` means this candidate is unusable; `committed` is unchanged.
fn replay_and_execute(committed: &[(String, Vec<u8>)], name: &str, data: &[u8]) -> mlua::Result<Lua> {
    let state = Lua::new();
    for (committed_name, committed_data) in committed {
        execute_chunk(&state, committed_name, committed_data)?;
    }
    execute_chunk(&state, name, data)?;
    Ok(state)
}

/// Trait for data that can be stored in a table and retrieved using a key.
pub trait Table {
    type Key<'a>;
    type Storage;

    fn load(game_file_loader: &GameFileLoader) -> mlua::Result<Self::Storage>;

    fn try_get<'a, 'b>(library: &'a Library, key: Self::Key<'b>) -> Option<&'a Self>
    where
        Self: Sized;

    fn get<'a, 'b>(library: &'a Library, key: Self::Key<'b>) -> &'a Self
    where
        Self: Sized;
}
