//! Client achievement catalog: `System/achievement_list.lub` with the English
//! `SystemEN/achievements.lub` overlay, as roBrowser loads `achievement_tbl`.
//! Also the attendance reward table from `System/CheckAttendance*.lub`.

use std::sync::{Arc, Mutex};

use hashbrown::HashMap;
use korangar_loaders::FileLoader;
use mlua::Lua;

use super::lua::{execute_chunk, game_string, value_u32};

const ACHIEVEMENT_FILES: &[&str] = &["System\\achievement_list.lub", "System/achievement_list.lub"];
const ACHIEVEMENT_OVERLAY_FILES: &[&str] = &["SystemEN\\achievements.lub", "SystemEN/achievements.lub"];
const ATTENDANCE_FILES: &[&str] = &[
    "System\\CheckAttendance.turoran1.lub",
    "System/CheckAttendance.turoran1.lub",
    "System\\CheckAttendance.lub",
    "System/CheckAttendance.lub",
];

/// What completing an achievement grants. Any of the three may be missing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AchievementReward {
    pub item_id: Option<u32>,
    pub title_id: Option<u32>,
    pub buff_id: Option<u32>,
}

/// One `achievement_tbl` entry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AchievementInfo {
    pub title: String,
    pub group: String,
    /// roBrowser major tab (1 General … 6 Feat). 0 is the summary page.
    pub major: u8,
    pub minor: u8,
    pub summary: String,
    pub details: String,
    /// `resource[n].text`, in key order.
    pub goals: Vec<String>,
    pub reward: AchievementReward,
    pub score: u32,
}

pub type AchievementCatalog = HashMap<u32, AchievementInfo>;

/// Base table first, then the English overlay. Each overlay entry replaces
/// the base entry, like roBrowser's `Object.assign`. Missing files leave the
/// catalog empty; they never fail startup.
pub fn load_achievement_catalog(loader: &impl FileLoader) -> AchievementCatalog {
    let mut catalog = AchievementCatalog::new();
    for group in [ACHIEVEMENT_FILES, ACHIEVEMENT_OVERLAY_FILES] {
        for file in group {
            let Ok(data) = loader.get(file) else { continue };
            let lua = Lua::new();
            if execute_chunk(&lua, file, &data).is_err() {
                continue;
            }
            merge_achievement_table(&lua, &mut catalog);
            break;
        }
    }
    catalog
}

pub fn merge_achievement_table(lua: &Lua, catalog: &mut AchievementCatalog) {
    let Ok(source) = lua.globals().get::<mlua::Table>("achievement_tbl") else {
        return;
    };
    for pair in source.pairs::<mlua::Value, mlua::Value>().flatten() {
        let Some(id) = value_u32(pair.0) else { continue };
        let mlua::Value::Table(entry) = pair.1 else { continue };
        catalog.insert(id, parse_entry(&entry));
    }
}

fn parse_entry(entry: &mlua::Table) -> AchievementInfo {
    let content = entry.get::<mlua::Table>("content").ok();
    let reward = entry.get::<mlua::Table>("reward").ok();
    AchievementInfo {
        title: text(entry, "title").unwrap_or_default(),
        group: text(entry, "group").unwrap_or_default(),
        major: number(entry, "major").and_then(|value| u8::try_from(value).ok()).unwrap_or(0),
        minor: number(entry, "minor").and_then(|value| u8::try_from(value).ok()).unwrap_or(0),
        summary: content.as_ref().and_then(|content| text(content, "summary")).unwrap_or_default(),
        details: content.as_ref().and_then(|content| text(content, "details")).unwrap_or_default(),
        goals: goals(entry),
        reward: AchievementReward {
            item_id: reward.as_ref().and_then(|reward| number(reward, "item")),
            title_id: reward.as_ref().and_then(|reward| number(reward, "title")),
            buff_id: reward.as_ref().and_then(|reward| number(reward, "buff")),
        },
        score: number(entry, "score").unwrap_or(0),
    }
}

fn text(table: &mlua::Table, key: &str) -> Option<String> {
    match table.get::<mlua::Value>(key).ok()? {
        mlua::Value::String(value) => game_string(value).ok().map(|value| value.trim().to_owned()),
        _ => None,
    }
}

fn number(table: &mlua::Table, key: &str) -> Option<u32> {
    value_u32(table.get::<mlua::Value>(key).ok()?)
}

fn goals(entry: &mlua::Table) -> Vec<String> {
    let Ok(resource) = entry.get::<mlua::Table>("resource") else {
        return Vec::new();
    };
    let mut goals: Vec<(u32, String)> = resource
        .pairs::<mlua::Value, mlua::Value>()
        .flatten()
        .filter_map(|(key, value)| {
            let mlua::Value::Table(goal) = value else { return None };
            let text = text(&goal, "text")?;
            (!text.is_empty()).then(|| (value_u32(key).unwrap_or(u32::MAX), text))
        })
        .collect();
    goals.sort_by_key(|(key, _)| *key);
    goals.into_iter().map(|(_, text)| text).collect()
}

/// One day of the attendance cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttendanceTableReward {
    pub day: u32,
    pub item_id: u32,
    pub amount: u32,
}

/// `CheckAttendance` config and rewards, sorted by day.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttendanceTable {
    pub start_date: u32,
    pub end_date: u32,
    pub rewards: Vec<AttendanceTableReward>,
}

pub fn load_attendance_table(loader: &impl FileLoader) -> AttendanceTable {
    for file in ATTENDANCE_FILES {
        let Ok(data) = loader.get(file) else { continue };
        let table = parse_attendance_chunk(file, &data);
        if !table.rewards.is_empty() {
            return table;
        }
    }
    AttendanceTable::default()
}

/// Runs the file's `main()` with roBrowser's `InsertCheckAttendanceConfig` /
/// `InsertCheckAttendanceReward` callbacks. A file without `main` falls back
/// to its `Config` and `Reward` globals.
pub fn parse_attendance_chunk(name: &str, data: &[u8]) -> AttendanceTable {
    let lua = Lua::new();
    let collected = Arc::new(Mutex::new(AttendanceTable::default()));
    let config_sink = Arc::clone(&collected);
    let reward_sink = Arc::clone(&collected);
    let registered = (|| -> mlua::Result<()> {
        let insert_config = lua.create_function(move |_, (_on_off, start, end): (mlua::Value, mlua::Value, mlua::Value)| {
            let mut table = config_sink.lock().unwrap();
            table.start_date = value_u32(start).unwrap_or(0);
            table.end_date = value_u32(end).unwrap_or(0);
            Ok(true)
        })?;
        let insert_reward = lua.create_function(move |_, (day, item, amount): (mlua::Value, mlua::Value, mlua::Value)| {
            if let (Some(day), Some(item_id)) = (value_u32(day), value_u32(item)) {
                reward_sink.lock().unwrap().rewards.push(AttendanceTableReward {
                    day,
                    item_id,
                    amount: value_u32(amount).unwrap_or(1),
                });
            }
            Ok(true)
        })?;
        lua.globals().set("InsertCheckAttendanceConfig", insert_config)?;
        lua.globals().set("InsertCheckAttendanceReward", insert_reward)?;
        Ok(())
    })();
    if registered.is_err() || execute_chunk(&lua, name, data).is_err() {
        return AttendanceTable::default();
    }
    if let Ok(main) = lua.globals().get::<mlua::Function>("main") {
        let _ = main.call::<mlua::MultiValue>(());
    }
    let mut table = collected.lock().unwrap().clone();
    if table.rewards.is_empty() {
        table = attendance_from_globals(&lua);
    }
    table.rewards.sort_by_key(|reward| reward.day);
    table.rewards.dedup_by_key(|reward| reward.day);
    table
}

fn attendance_from_globals(lua: &Lua) -> AttendanceTable {
    let mut table = AttendanceTable::default();
    if let Ok(config) = lua.globals().get::<mlua::Table>("Config") {
        table.start_date = number(&config, "StartDate").unwrap_or(0);
        table.end_date = number(&config, "EndDate").unwrap_or(0);
    }
    if let Ok(rewards) = lua.globals().get::<mlua::Table>("Reward") {
        for row in rewards.sequence_values::<mlua::Table>().flatten() {
            let day = row.get::<mlua::Value>(1).ok().and_then(value_u32);
            let item = row.get::<mlua::Value>(2).ok().and_then(value_u32);
            let amount = row.get::<mlua::Value>(3).ok().and_then(value_u32).unwrap_or(1);
            if let (Some(day), Some(item_id)) = (day, item) {
                table.rewards.push(AttendanceTableReward { day, item_id, amount });
            }
        }
    }
    table
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::{AchievementCatalog, merge_achievement_table, parse_attendance_chunk};
    use crate::world::library::lua::execute_chunk;

    const BASE: &str = r#"
achievement_tbl = {
    [120001] = {
        UI_Type = 0, group = "ADVENTURE", major = 2, minor = 0,
        title = "korean title",
        content = { summary = "s", details = "d" },
        resource = { [1] = { text = "goal" } },
        reward = { item = 22876 },
        score = 10
    },
    [200000] = {
        group = "BATTLE", major = 3, minor = 1, title = "Duelist",
        content = { summary = "Win", details = "Win duels" },
        resource = { [2] = { text = "second" }, [1] = { text = "first" } },
        reward = { title = 1001, buff = 7 },
        score = 25
    },
}
"#;

    const OVERLAY: &str = r#"
achievement_tbl = {
    [120001] = {
        group = "ADVENTURE", major = 2, minor = 0,
        title = "North Prontera Field Exploration(1)",
        content = { summary = "Discover hidden treasure", details = "Discover treasure in north Prontera field" },
        resource = { [1] = { text = "Discover treasure in north Prontera field" } },
        reward = { item = 22876 },
        score = 10
    },
}
"#;

    #[test]
    fn overlay_replaces_base_entries() {
        let mut catalog = AchievementCatalog::new();
        for chunk in [BASE, OVERLAY] {
            let lua = Lua::new();
            execute_chunk(&lua, "achievements.lub", chunk.as_bytes()).unwrap();
            merge_achievement_table(&lua, &mut catalog);
        }
        let field = &catalog[&120001];
        assert_eq!(field.title, "North Prontera Field Exploration(1)");
        assert_eq!((field.major, field.minor, field.score), (2, 0, 10));
        assert_eq!(field.reward.item_id, Some(22876));
        assert_eq!(field.details, "Discover treasure in north Prontera field");

        let duel = &catalog[&200000];
        assert_eq!(duel.goals, vec!["first".to_owned(), "second".to_owned()]);
        assert_eq!(duel.reward.title_id, Some(1001));
        assert_eq!(duel.reward.buff_id, Some(7));
        assert_eq!(duel.reward.item_id, None);
    }

    #[test]
    fn attendance_main_and_globals() {
        let source = br#"
Config = { EvendOnOff = 1, StartDate = 20261021, EndDate = 20991231 }
Reward = { { 2, 608, 3 }, { 1, 12211, 5 } }
function main()
    result, msg = InsertCheckAttendanceConfig(Config.EvendOnOff, Config.StartDate, Config.EndDate)
    for k, r in pairs(Reward) do InsertCheckAttendanceReward(r[1], r[2], r[3]) end
    return true, "success"
end
"#;
        let table = parse_attendance_chunk("CheckAttendance.lub", source);
        assert_eq!((table.start_date, table.end_date), (20261021, 20991231));
        assert_eq!(table.rewards.len(), 2);
        assert_eq!(
            (table.rewards[0].day, table.rewards[0].item_id, table.rewards[0].amount),
            (1, 12211, 5)
        );

        let no_main = parse_attendance_chunk("CheckAttendance.lub", b"Reward = { { 1, 501, 2 } }");
        assert_eq!(no_main.rewards[0].item_id, 501);
        assert!(parse_attendance_chunk("broken.lub", b"Reward = {").rewards.is_empty());
    }
}
