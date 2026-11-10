//! Quest titles, summaries, descriptions, and rewards.
//!
//! Renewal clients load `System/OngoingQuestInfoList` the way roBrowser
//! `getSystemAliases` does, then merge `SystemEN/OngoingQuests.reeng.lub`.
//! `data/questid2display.txt` fills ids the Lua tables do not define.

use hashbrown::HashMap;
use korangar_loaders::FileLoader;
use mlua::Lua;

use super::LuaExt;
use super::item_info::strip_navi_tags;
use super::lua::{execute_chunk, game_string, value_u32};
use crate::loaders::GameFileLoader;

/// One reward stack from `RewardItemList`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestRewardItem {
    pub item_id: u32,
    pub count: u32,
}

/// Display row from `QuestInfoList` or `questid2display.txt`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestDisplay {
    pub title: String,
    pub summary: String,
    pub description: Vec<String>,
    pub reward_exp: u64,
    pub reward_job_exp: u64,
    pub reward_items: Vec<QuestRewardItem>,
}

/// Lookups the quest log uses while applying network events.
pub trait QuestTextSource {
    fn quest_display(&self, quest_id: u32) -> Option<&QuestDisplay>;
    fn monster_name(&self, mob_id: u32) -> Option<&str>;
    fn item_label(&self, item_id: u32) -> String;
}

const QUEST_LUA_CANDIDATES: &[&str] = &[
    "System\\OngoingQuestInfoList.lub",
    "System/OngoingQuestInfoList.lub",
    "System\\OngoingQuestInfoList.lua",
    "System/OngoingQuestInfoList.lua",
    "System\\OngoingQuestInfoList_true.lub",
    "System/OngoingQuestInfoList_true.lub",
    "System\\OngoingQuestInfoList_true.lua",
    "System/OngoingQuestInfoList_true.lua",
    "System\\OngoingQuestInfoList_sak.lub",
    "System/OngoingQuestInfoList_sak.lub",
    "System\\OngoingQuestInfoList_sak.lua",
    "System/OngoingQuestInfoList_sak.lua",
    "System\\OngoingQuestInfoList_Sakray.lub",
    "System/OngoingQuestInfoList_Sakray.lub",
    "System\\OngoingQuestInfoList_Sakray.lua",
    "System/OngoingQuestInfoList_Sakray.lua",
];

const QUEST_TRANSLATION_CANDIDATES: &[&str] = &[
    "SystemEN\\OngoingQuests.reeng.lub",
    "SystemEN/OngoingQuests.reeng.lub",
    "System\\OngoingQuests.reeng.lub",
    "System/OngoingQuests.reeng.lub",
];

const QUEST_TXT_CANDIDATES: &[&str] = &["data\\questid2display.txt", "data/questid2display.txt"];

pub(super) fn load_quest_displays(game_file_loader: &GameFileLoader) -> HashMap<u32, QuestDisplay> {
    let mut displays = HashMap::new();
    if let Ok(state) = Lua::load_from_game_files_with_fallbacks(game_file_loader, &[QUEST_LUA_CANDIDATES]) {
        displays = parse_quest_info_list(&state);
    }
    if let Some(bytes) = read_first(game_file_loader, QUEST_TRANSLATION_CANDIDATES) {
        let state = Lua::new();
        if execute_chunk(&state, "OngoingQuests.reeng.lub", &bytes).is_ok() {
            merge_quest_displays(&mut displays, parse_quest_info_list(&state));
        }
    }
    if let Some(bytes) = read_first(game_file_loader, QUEST_TXT_CANDIDATES) {
        fill_missing_quest_displays(&mut displays, parse_questid2display(&decode_bytes(&bytes)));
    }
    displays
}

pub(super) fn read_first(game_file_loader: &GameFileLoader, paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| game_file_loader.get(path).ok())
}

pub(super) fn decode_bytes(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }
    encoding_rs::EUC_KR
        .decode_without_bom_handling_and_without_replacement(bytes)
        .map(|text| text.into_owned())
        .unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
}

/// Later entries replace the same quest id. Used for the English overlay.
pub fn merge_quest_displays(base: &mut HashMap<u32, QuestDisplay>, overlay: HashMap<u32, QuestDisplay>) {
    for (quest_id, display) in overlay {
        base.insert(quest_id, display);
    }
}

/// `questid2display.txt` only supplies quests the Lua list did not define.
pub fn fill_missing_quest_displays(base: &mut HashMap<u32, QuestDisplay>, extra: HashMap<u32, QuestDisplay>) {
    for (quest_id, display) in extra {
        base.entry(quest_id).or_insert(display);
    }
}

pub fn parse_quest_info_list(state: &Lua) -> HashMap<u32, QuestDisplay> {
    let Ok(table) = state.globals().get::<mlua::Table>("QuestInfoList") else {
        return HashMap::new();
    };
    let mut displays = HashMap::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
        let Some(quest_id) = value_u32(pair.0) else { continue };
        let mlua::Value::Table(row) = pair.1 else { continue };
        displays.insert(quest_id, quest_display_from_row(&row));
    }
    displays
}

fn quest_display_from_row(row: &mlua::Table) -> QuestDisplay {
    let title = row_string(row, "Title");
    let summary = row_string(row, "Summary");
    let description = row_lines(row, "Description");
    let reward_exp = row_u64(row, "RewardEXP");
    let reward_job_exp = row_u64(row, "RewardJEXP");
    let reward_items = reward_items(row);
    QuestDisplay {
        title,
        summary,
        description,
        reward_exp,
        reward_job_exp,
        reward_items,
    }
}

fn row_string(row: &mlua::Table, key: &str) -> String {
    match row.get::<mlua::Value>(key) {
        Ok(mlua::Value::String(text)) => game_string(text).map(|text| clean_quest_text(&text)).unwrap_or_default(),
        _ => String::new(),
    }
}

fn row_lines(row: &mlua::Table, key: &str) -> Vec<String> {
    match row.get::<mlua::Value>(key) {
        Ok(mlua::Value::Table(lines)) => {
            let mut collected = Vec::new();
            for value in lines.sequence_values::<mlua::Value>().flatten() {
                if let Some(line) = line_from_value(value) {
                    collected.push(line);
                }
            }
            if collected.is_empty() {
                let mut indexed = Vec::new();
                for pair in lines.pairs::<mlua::Value, mlua::Value>().flatten() {
                    let Some(line) = line_from_value(pair.1) else { continue };
                    indexed.push((value_u32(pair.0).unwrap_or(u32::MAX), line));
                }
                indexed.sort_by_key(|(index, _)| *index);
                collected = indexed.into_iter().map(|(_, line)| line).collect();
            }
            collected.into_iter().flat_map(|line| split_quest_lines(&line)).collect()
        }
        Ok(mlua::Value::String(text)) => game_string(text)
            .map(|text| split_quest_lines(&clean_quest_text(&text)))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn line_from_value(value: mlua::Value) -> Option<String> {
    match value {
        mlua::Value::String(text) => game_string(text).ok().map(|text| clean_quest_text(&text)),
        _ => None,
    }
}

fn row_u64(row: &mlua::Table, key: &str) -> u64 {
    match row.get::<mlua::Value>(key) {
        Ok(mlua::Value::Integer(number)) if number > 0 => number as u64,
        Ok(mlua::Value::Number(number)) if number > 0.0 => number as u64,
        Ok(mlua::Value::String(text)) => game_string(text).ok().and_then(|text| text.trim().parse::<u64>().ok()).unwrap_or(0),
        _ => 0,
    }
}

fn reward_items(row: &mlua::Table) -> Vec<QuestRewardItem> {
    let Ok(mlua::Value::Table(list)) = row.get::<mlua::Value>("RewardItemList") else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for value in list.sequence_values::<mlua::Value>().flatten() {
        let mlua::Value::Table(item) = value else { continue };
        let Some(item_id) = item.get::<mlua::Value>("ItemID").ok().and_then(value_u32) else {
            continue;
        };
        if item_id == 0 {
            continue;
        }
        let count = item.get::<mlua::Value>("ItemNum").ok().and_then(value_u32).unwrap_or(1).max(1);
        items.push(QuestRewardItem { item_id, count });
    }
    items
}

/// Gravity `#` table: `id#title#group#image#description#summary#`.
pub fn parse_questid2display(text: &str) -> HashMap<u32, QuestDisplay> {
    let without_comments = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let elements: Vec<&str> = without_comments.split('#').map(str::trim).collect();
    let mut displays = HashMap::new();
    for chunk in elements.chunks(6) {
        if chunk.len() < 6 {
            continue;
        }
        let Ok(quest_id) = chunk[0].parse::<u32>() else { continue };
        let description = split_quest_lines(&clean_quest_text(chunk[4]));
        displays.insert(quest_id, QuestDisplay {
            title: clean_quest_text(chunk[1]),
            summary: clean_quest_text(chunk[5]),
            description,
            ..QuestDisplay::default()
        });
    }
    displays
}

pub fn clean_quest_text(text: &str) -> String {
    let unescaped = text.replace("\\r\\n", "\n").replace("\\n", "\n").replace('\r', "");
    strip_item_tags(&strip_navi_tags(&unescaped)).trim().to_owned()
}

fn split_quest_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `<ITEM>Name<INFO>id</INFO></ITEM>` becomes the visible name.
pub fn strip_item_tags(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let open = "<item>";
    let info = "<info>";
    let close = "</item>";
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(rel_start) = lower[cursor..].find(open) {
        let start = cursor + rel_start;
        out.push_str(&input[cursor..start]);
        let after_open = start + open.len();
        let Some(rel_close) = lower[after_open..].find(close) else {
            out.push_str(&input[start..]);
            return out;
        };
        let close_at = after_open + rel_close;
        let inner = &input[after_open..close_at];
        let inner_lower = &lower[after_open..close_at];
        if let Some(info_at) = inner_lower.find(info) {
            out.push_str(inner[..info_at].trim());
        } else {
            out.push_str(inner);
        }
        cursor = close_at + close.len();
    }
    out.push_str(&input[cursor..]);
    out
}

pub fn format_quest_rewards(display: &QuestDisplay, mut item_label: impl FnMut(u32) -> String) -> String {
    let mut lines = Vec::new();
    if display.reward_exp > 0 {
        lines.push(format!("Base EXP: {}", display.reward_exp));
    }
    if display.reward_job_exp > 0 {
        lines.push(format!("Job EXP: {}", display.reward_job_exp));
    }
    for item in &display.reward_items {
        let name = item_label(item.item_id);
        if item.count > 1 {
            lines.push(format!("{name} x{}", item.count));
        } else {
            lines.push(name);
        }
    }
    lines.join("\n")
}

/// Unix deadline from `quest_endTime`. `0` means the quest has no timer.
pub fn quest_is_cooldown(expire_time: u32, now_unix: u32) -> bool {
    expire_time > 1_000_000_000 && expire_time > now_unix
}

pub fn format_quest_timer(expire_time: u32, now_unix: u32) -> String {
    if expire_time == 0 {
        return String::new();
    }
    if expire_time > 1_000_000_000 {
        if expire_time <= now_unix {
            return String::new();
        }
        return format!("Time left: {}", format_duration(expire_time - now_unix));
    }
    format!("Time left: {}", format_duration(expire_time))
}

fn format_duration(seconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::{
        clean_quest_text, fill_missing_quest_displays, format_quest_rewards, format_quest_timer, merge_quest_displays,
        parse_quest_info_list, parse_questid2display, quest_is_cooldown, strip_item_tags,
    };
    use crate::world::library::lua::execute_chunk;

    #[test]
    fn quest_info_list_loads_description_lines_and_rewards() {
        let lua = Lua::new();
        execute_chunk(
            &lua,
            "quests.lub",
            br#"
            QuestInfoList = {
              [9] = {
                Title = "Poring Hunt",
                Summary = "Hunt porings",
                Description = { "Kill them.", "Then return." },
                RewardEXP = 100,
                RewardJEXP = "40",
                RewardItemList = { { ItemID = 501, ItemNum = 3 } },
              }
            }
            "#,
        )
        .unwrap();
        let displays = parse_quest_info_list(&lua);
        let quest = &displays[&9];
        assert_eq!(quest.title, "Poring Hunt");
        assert_eq!(quest.summary, "Hunt porings");
        assert_eq!(quest.description, vec!["Kill them.".to_owned(), "Then return.".to_owned()]);
        assert_eq!(quest.reward_exp, 100);
        assert_eq!(quest.reward_job_exp, 40);
        assert_eq!(quest.reward_items[0].item_id, 501);
        assert_eq!(quest.reward_items[0].count, 3);
        let rewards = format_quest_rewards(quest, |item_id| format!("Item {item_id}"));
        assert!(rewards.contains("Base EXP: 100"));
        assert!(rewards.contains("Job EXP: 40"));
        assert!(rewards.contains("Item 501 x3"));
    }

    #[test]
    fn translation_overlay_replaces_one_quest_and_txt_fills_gaps() {
        let mut displays =
            parse_questid2display("9#Old Title#Field#ico#Old line#Old summary#\n10#Txt Quest#Field#ico#From txt#Txt summary#\n");
        let lua = Lua::new();
        execute_chunk(
            &lua,
            "overlay.lub",
            br#"QuestInfoList = { [9] = { Title = "New Title", Summary = "New summary", Description = { "Fresh." } } }"#,
        )
        .unwrap();
        merge_quest_displays(&mut displays, parse_quest_info_list(&lua));
        let txt = parse_questid2display("9#Should Not Win#Field#ico#no#no#\n11#Only Txt#Field#ico#txt body#txt sum#\n");
        fill_missing_quest_displays(&mut displays, txt);
        assert_eq!(displays[&9].title, "New Title");
        assert_eq!(displays[&9].description, vec!["Fresh."]);
        assert_eq!(displays[&10].title, "Txt Quest");
        assert_eq!(displays[&11].title, "Only Txt");
    }

    #[test]
    fn quest_text_strips_item_and_navi_markup_and_keeps_color_codes() {
        let cleaned = clean_quest_text(
            "See <ITEM>Red Potion<INFO>501</INFO></ITEM> at <NAVI>Prontera<INFO>prontera,1,2,0,000,0</INFO></NAVI>\\n^FF0000Now^000000",
        );
        assert_eq!(cleaned, "See Red Potion at Prontera\n^FF0000Now^000000");
        assert_eq!(strip_item_tags("plain"), "plain");
    }

    #[test]
    fn timers_use_the_unix_deadline() {
        assert!(quest_is_cooldown(1_700_000_100, 1_700_000_000));
        assert!(!quest_is_cooldown(1_700_000_000, 1_700_000_100));
        assert!(!quest_is_cooldown(0, 1_700_000_000));
        assert_eq!(format_quest_timer(0, 10), "");
        assert_eq!(format_quest_timer(1_700_000_000, 1_700_000_100), "");
        assert_eq!(format_quest_timer(1_700_000_090, 1_700_000_000), "Time left: 1m 30s");
        assert_eq!(format_quest_timer(45, 0), "Time left: 45s");
    }
}
