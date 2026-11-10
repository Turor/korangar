mod baby_job;
mod item_info;
mod item_name;
mod item_resource;
mod job_identity;
mod lua;
mod map_display_name;
mod map_sky_data;
mod signboard;
mod skill_information;
mod skill_requirements;
mod skill_tree;

use std::hash::Hash;

use hashbrown::HashMap;
use korangar_loaders::FileLoader;
use mlua::{ErrorContext, Lua};

pub use self::baby_job::IsBabyJob;
pub use self::item_info::ItemInfo;
pub use self::item_name::{ItemName, ItemNameKey};
pub use self::item_resource::{ItemResource, ItemResourceKey, classify_item_icon_failure, inventory_icon_texture_path};
pub use self::job_identity::JobIdentity;
use self::lua::{execute_chunk, game_string};
pub use self::map_display_name::{display_name_from_table, load_map_display_names};
pub use self::map_sky_data::MapSkyData;
pub use self::signboard::MapSignboards;
pub use self::skill_tree::SkillTreeLayout;
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
        })
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
