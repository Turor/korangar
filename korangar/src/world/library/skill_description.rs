//! Skill description text from `skilldescript.turoran.re2.lub` plus the step 6
//! override, matching roBrowser `DBManager` (`SKILL_DESCRIPT` then
//! `SKILL_DESCRIPT_TURORAN_S6`).

use std::collections::HashSet;

use hashbrown::HashMap;
use mlua::Lua;

use super::LuaExt;
use super::lua::{execute_chunk, game_string, value_u32};
use super::quest_display::read_first;
use crate::loaders::GameFileLoader;

#[derive(Clone, Debug, Default)]
pub struct JobParents {
    pub primary: HashMap<u16, u16>,
    pub secondary: HashMap<u16, u16>,
}

const SKILL_ID_CANDIDATES: &[&str] = &[
    "data\\luafiles514\\lua files\\skillinfoz\\skillid.turoran4.lub",
    "data/luafiles514/lua files/skillinfoz/skillid.turoran4.lub",
    "data\\luafiles514\\lua files\\skillinfoz\\skillid.lub",
    "data/luafiles514/lua files/skillinfoz/skillid.lub",
];

const DESCRIPT_CANDIDATES: &[&str] = &[
    "System\\skilldescript.turoran.re2.lub",
    "System/skilldescript.turoran.re2.lub",
    "System\\skilldescript.lub",
    "System/skilldescript.lub",
];

const STEP6_CANDIDATES: &[&str] = &["System\\skilldescript.turoran.step6.lub", "System/skilldescript.turoran.step6.lub"];

const JOB_INHERIT_CANDIDATES: &[&str] = &[
    "data\\luafiles514\\lua files\\skillinfoz\\jobinheritlist.lub",
    "data/luafiles514/lua files/skillinfoz/jobinheritlist.lub",
];

pub(super) fn load_skill_text(game_file_loader: &GameFileLoader) -> (HashMap<u16, String>, JobParents) {
    let parents = load_job_parents(game_file_loader);
    let Ok(state) = Lua::load_from_game_files_with_fallbacks(game_file_loader, &[SKILL_ID_CANDIDATES]) else {
        return (HashMap::new(), parents);
    };
    let Some(bytes) = read_first(game_file_loader, DESCRIPT_CANDIDATES) else {
        return (HashMap::new(), parents);
    };
    if execute_chunk(&state, "skilldescript", &bytes).is_err() {
        return (HashMap::new(), parents);
    }
    let mut descriptions = parse_skill_descript_table(&state, "SKILL_DESCRIPT");
    if let Some(step6) = read_first(game_file_loader, STEP6_CANDIDATES)
        && execute_chunk(&state, "skilldescript.step6", &step6).is_ok()
    {
        let overlay = parse_skill_descript_table(&state, "SKILL_DESCRIPT_TURORAN_S6");
        if overlay.is_empty() {
            // Some overrides rewrite `SKILL_DESCRIPT` instead of a side table.
            descriptions = parse_skill_descript_table(&state, "SKILL_DESCRIPT");
        } else {
            descriptions.extend(overlay);
        }
    }
    (descriptions, parents)
}

fn load_job_parents(game_file_loader: &GameFileLoader) -> JobParents {
    let Some(bytes) = read_first(game_file_loader, JOB_INHERIT_CANDIDATES) else {
        return JobParents::default();
    };
    let state = Lua::new();
    if execute_chunk(&state, "jobinheritlist.lub", &bytes).is_err() {
        return JobParents::default();
    }
    JobParents {
        primary: parent_map(&state, "JOB_INHERIT_LIST"),
        secondary: parent_map(&state, "JOB_INHERIT_LIST2"),
    }
}

fn parent_map(state: &Lua, name: &str) -> HashMap<u16, u16> {
    let Ok(table) = state.globals().get::<mlua::Table>(name) else {
        return HashMap::new();
    };
    let mut parents = HashMap::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
        let Some(job) = value_u32(pair.0).and_then(|id| u16::try_from(id).ok()) else {
            continue;
        };
        let Some(parent) = value_u32(pair.1).and_then(|id| u16::try_from(id).ok()) else {
            continue;
        };
        parents.insert(job, parent);
    }
    parents
}

pub fn parse_skill_descript_table(state: &Lua, table_name: &str) -> HashMap<u16, String> {
    let Ok(table) = state.globals().get::<mlua::Table>(table_name) else {
        return HashMap::new();
    };
    let mut descriptions = HashMap::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
        let Some(skill_id) = skill_key(state, pair.0) else { continue };
        let Some(text) = skill_text(pair.1) else { continue };
        if text.is_empty() {
            continue;
        }
        descriptions.insert(skill_id, text);
    }
    descriptions
}

fn skill_key(state: &Lua, key: mlua::Value) -> Option<u16> {
    if let Some(id) = value_u32(key.clone()).and_then(|id| u16::try_from(id).ok()) {
        return Some(id);
    }
    let mlua::Value::String(text) = key else {
        return None;
    };
    let name = game_string(text).ok()?;
    let skid = state.globals().get::<mlua::Table>("SKID").ok()?;
    let id = skid.get::<i64>(name).ok()?;
    u16::try_from(id).ok()
}

fn skill_text(value: mlua::Value) -> Option<String> {
    match value {
        mlua::Value::String(text) => game_string(text).ok().map(|text| text.replace('\r', "")),
        mlua::Value::Table(table) => {
            let mut lines = Vec::new();
            for value in table.sequence_values::<mlua::Value>().flatten() {
                if let Some(line) = skill_line(value) {
                    lines.push(line);
                }
            }
            if lines.is_empty() {
                let mut indexed = Vec::new();
                for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
                    let Some(line) = skill_line(pair.1) else { continue };
                    indexed.push((value_u32(pair.0).unwrap_or(u32::MAX), line));
                }
                indexed.sort_by_key(|(index, _)| *index);
                lines = indexed.into_iter().map(|(_, line)| line).collect();
            }
            let text = lines.join("\n");
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn skill_line(value: mlua::Value) -> Option<String> {
    let mlua::Value::String(text) = value else {
        return None;
    };
    game_string(text).ok().map(|text| text.trim_end_matches('\r').to_owned())
}

pub fn job_ancestry(job: u16, parents: &JobParents) -> HashSet<u16> {
    let mut seen = HashSet::new();
    let mut pending = vec![job];
    let mut guard = 0;
    while let Some(current) = pending.pop() {
        if !seen.insert(current) || guard > 32 {
            continue;
        }
        guard += 1;
        if let Some(parent) = parents.primary.get(&current) {
            pending.push(*parent);
        }
        if let Some(parent) = parents.secondary.get(&current) {
            pending.push(*parent);
        }
    }
    seen
}

/// Drop `jobId^` lines that belong to another job, and remove the prefix from
/// lines that stay. This is roBrowser `filterJobPrefixedSkillDesc`.
pub fn filter_job_prefixed_lines(description: &str, job: u16, parents: &JobParents) -> String {
    let ancestry = job_ancestry(job, parents);
    description
        .lines()
        .filter_map(|line| match job_prefix(line) {
            Some(prefix) if ancestry.contains(&prefix) => Some(&line[prefix.to_string().len()..]),
            Some(_) => None,
            None => Some(line),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn job_prefix(line: &str) -> Option<u16> {
    let bytes = line.as_bytes();
    if bytes.first().is_none_or(|byte| !byte.is_ascii_digit()) {
        return None;
    }
    let mut end = 0;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if bytes.get(end) != Some(&b'^') {
        return None;
    }
    line[..end].parse().ok()
}

/// Level shown on the slot and in the description. Pending points win over a
/// chosen cast level. Nothing learned, pending, or chosen is level 0.
pub fn shown_skill_level(learned: Option<u16>, pending_points: u16, chosen: Option<u16>) -> u16 {
    if pending_points >= 1 {
        learned.unwrap_or(0).saturating_add(pending_points)
    } else {
        chosen.or(learned).unwrap_or(0)
    }
}

/// Level line plus the roBrowser SP-cost line, then the description body.
pub fn compose_skill_description(raw: &str, level: u16, maximum_level: u16, spell_points: &[u32]) -> String {
    let maximum = maximum_level.max(level);
    let mut lines = vec![format!("Level: {level} / {maximum}")];
    if !spell_points.is_empty() {
        let cost_level = level.max(1);
        let index = (cost_level as usize).min(spell_points.len()).saturating_sub(1);
        let cost = spell_points[index];
        if cost > 0 {
            lines.push(format!("SP Cost (Lv {cost_level}): ^777777{cost}^000000"));
        }
    }
    let body = raw.trim();
    if !body.is_empty() && body != "..." {
        lines.push(body.to_owned());
    }
    lines.join("\n")
}

/// `data/leveluseskillspamount.txt`: `SKILL_NAME#lv1#lv2#...#@`.
pub fn parse_sp_amount_table(text: &str) -> Vec<(String, Vec<u32>)> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        for chunk in line.split('@') {
            let chunk = chunk.trim();
            if chunk.is_empty() || chunk.starts_with("//") {
                continue;
            }
            let mut parts = chunk.split('#').map(str::trim).filter(|part| !part.is_empty());
            let Some(name) = parts.next() else { continue };
            let amounts: Vec<u32> = parts.filter_map(|part| part.parse().ok()).collect();
            if amounts.is_empty() {
                continue;
            }
            rows.push((name.to_owned(), amounts));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    use super::{
        JobParents, compose_skill_description, filter_job_prefixed_lines, parse_skill_descript_table, parse_sp_amount_table,
        shown_skill_level,
    };
    use crate::world::library::lua::execute_chunk;

    #[test]
    fn skill_descript_joins_lines_and_step6_replaces_one_skill() {
        let lua = Lua::new();
        execute_chunk(
            &lua,
            "skilldescript.lub",
            br#"
            SKID = { NV_BASIC = 1, SM_BASH = 5 }
            SKILL_DESCRIPT = {
              [SKID.NV_BASIC] = { "Basic Skill", "Max Level:^777777 9^000000" },
              [SKID.SM_BASH] = "Bash once",
            }
            SKILL_DESCRIPT_TURORAN_S6 = {
              [SKID.SM_BASH] = { "Bash", "Turoran note" },
            }
            "#,
        )
        .unwrap();
        let mut descriptions = parse_skill_descript_table(&lua, "SKILL_DESCRIPT");
        descriptions.extend(parse_skill_descript_table(&lua, "SKILL_DESCRIPT_TURORAN_S6"));
        assert_eq!(
            descriptions.get(&1).map(String::as_str),
            Some("Basic Skill\nMax Level:^777777 9^000000")
        );
        assert_eq!(descriptions.get(&5).map(String::as_str), Some("Bash\nTuroran note"));
    }

    #[test]
    fn job_prefix_keeps_the_current_job_and_its_parent() {
        let mut parents = JobParents::default();
        parents.primary.insert(7, 1);
        let filtered = filter_job_prefixed_lines("Always\n1^Novice line\n7^Swordsman line\n4008^Other job", 7, &parents);
        assert_eq!(filtered, "Always\n^Novice line\n^Swordsman line");
        assert!(filtered.contains("^Novice"));
    }

    #[test]
    fn compose_adds_level_and_sp_and_keeps_color_codes() {
        let text = compose_skill_description("Max Level:^777777 10^000000", 3, 10, &[8, 8, 12]);
        assert!(text.starts_with("Level: 3 / 10\n"));
        assert!(text.contains("SP Cost (Lv 3): ^77777712^000000"));
        assert!(text.contains("Max Level:^777777 10^000000"));
        let unlearned = compose_skill_description("", 0, 5, &[0]);
        assert_eq!(unlearned, "Level: 0 / 5");
    }

    #[test]
    fn shown_level_prefers_pending_points_over_the_chosen_cast_level() {
        assert_eq!(shown_skill_level(Some(2), 1, Some(1)), 3);
        assert_eq!(shown_skill_level(None, 2, Some(4)), 2);
        assert_eq!(shown_skill_level(Some(5), 0, Some(3)), 3);
        assert_eq!(shown_skill_level(Some(5), 0, None), 5);
        assert_eq!(shown_skill_level(None, 0, None), 0);
    }

    #[test]
    fn sp_amount_file_parses_hash_rows() {
        let rows = parse_sp_amount_table("// comment\nSM_BASH#8#8#10#@\nNV_BASIC#0#@\n");
        assert_eq!(rows[0].0, "SM_BASH");
        assert_eq!(rows[0].1, vec![8, 8, 10]);
        assert_eq!(rows[1], ("NV_BASIC".to_owned(), vec![0]));
    }
}
