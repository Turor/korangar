//! Turoran Item Finder catalog.
//!
//! The browser client reads `System/itemfinder.turoran.v8.json` and
//! `System/itemsources.turoran.v3.json`. Those files are produced with the
//! item tables owned by data worker A5 (`itemInfo.turoran.renewal11`). This
//! module loads them when they are already in the client data path. When they
//! are absent it keeps a typed empty catalog and says so. There is no map
//! packet for this window: search is local.

use std::collections::HashMap;

use serde_json::Value;

/// Catalog path the generator writes. A5 publishes the item table this file is
/// built from.
pub const CATALOG_PATH: &str = "System/itemfinder.turoran.v8.json";
/// Drop, spawn, and quest-source path. Same data-worker dependency as the
/// catalog.
pub const SOURCES_PATH: &str = "System/itemsources.turoran.v3.json";

pub const MISSING_CATALOG_STATUS: &str = "Item Finder catalog is not loaded. Data worker A5 owns itemInfo.turoran.renewal11 and has not \
                                          published System/itemfinder.turoran.v8.json in the client data path. This window stays empty \
                                          until that file is available. Drop lines also need System/itemsources.turoran.v3.json.";

const MISSING_SOURCES_STATUS: &str =
    "Drop and quest sources are not loaded. Waiting on System/itemsources.turoran.v3.json from data worker A5.";

const PAGE: usize = 40;

#[derive(Clone, Debug)]
pub struct FinderEffect {
    pub kind: String,
    pub target: String,
    pub text: String,
    pub conditional: bool,
    pub magnitude: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct FinderItem {
    pub id: u32,
    pub name: String,
    pub category: String,
    pub weapon_type: String,
    pub locations: Vec<String>,
    pub attack: i32,
    pub magic_attack: i32,
    pub defense: i32,
    pub flat_mdef: i32,
    pub weapon_level: i32,
    pub slots: i32,
    pub weight: i32,
    pub equip_level: i32,
    pub jobs: String,
    /// Slotted MVP item id, when the catalog marks one. Also used as a quest-id
    /// override for [`quest_files`].
    pub mvp_item_id: Option<u32>,
    pub effects: Vec<FinderEffect>,
    search_blob: String,
}

#[derive(Clone, Debug, Default)]
pub struct FinderCatalog {
    items: Vec<FinderItem>,
}

impl FinderCatalog {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get(&self, id: u32) -> Option<&FinderItem> {
        self.items.iter().find(|item| item.id == id)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct MobSpawn {
    pub map: String,
    pub name: String,
    pub count: i64,
    pub respawn_ms: i64,
}

#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct FinderMob {
    pub name: String,
    pub level: i32,
    pub boss: bool,
    pub mvp: bool,
    pub spawns: Vec<MobSpawn>,
}

#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
struct RawDrop {
    mob_id: u32,
    /// Server rate in 1/10000 units.
    rate: i64,
    mvp_reward: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ItemSources {
    mobs: HashMap<u32, FinderMob>,
    drops: HashMap<u32, Vec<RawDrop>>,
    quests: HashMap<u32, Vec<String>>,
    /// Level-difference penalty rows for bosses (`penalty["11"]`).
    penalty_boss: Vec<(i32, i32)>,
    /// Level-difference penalty rows for normal monsters (`penalty["12"]`).
    penalty_normal: Vec<(i32, i32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct DropLine {
    pub mob_name: String,
    pub mob_level: i32,
    pub boss: bool,
    pub mvp_monster: bool,
    pub mvp_reward: bool,
    pub tag: String,
    /// Adjusted rate in 1/10000 units, capped at 10000.
    pub rate: i64,
    pub spawns: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct FinderQuery {
    pub category: String,
    pub text: String,
    pub effect_kinds: Vec<String>,
    pub effect_target: String,
    pub job: String,
    pub max_equip_level: Option<i32>,
    pub unconditional_only: bool,
}

impl Default for FinderQuery {
    fn default() -> Self {
        Self {
            category: "weapon".to_owned(),
            text: String::new(),
            effect_kinds: Vec::new(),
            effect_target: String::new(),
            job: String::new(),
            max_equip_level: None,
            unconditional_only: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FinderHit {
    pub item_id: u32,
    pub name: String,
    pub summary: String,
}

/// Window-facing state. The catalog itself is not an interface element.
#[derive(Clone, Debug, korangar_interface::element::StateElement, rust_state::RustState)]
pub struct ItemFinderState {
    pub status: String,
    pub query: String,
    pub category: String,
    pub rows: Vec<String>,
    #[hidden_element]
    catalog: FinderCatalog,
    #[hidden_element]
    sources: Option<ItemSources>,
    #[hidden_element]
    sources_ready: bool,
}

impl Default for ItemFinderState {
    fn default() -> Self {
        Self {
            status: MISSING_CATALOG_STATUS.to_owned(),
            query: String::new(),
            category: "weapon".to_owned(),
            rows: Vec::new(),
            catalog: FinderCatalog::default(),
            sources: None,
            sources_ready: false,
        }
    }
}

impl ItemFinderState {
    /// Load whichever files the data path actually has. Missing or invalid
    /// bytes leave the typed catalog empty and record the A5 dependency.
    pub fn load_bytes(&mut self, catalog: Option<&[u8]>, sources: Option<&[u8]>) {
        match catalog.and_then(|bytes| parse_catalog(bytes).ok()) {
            Some(parsed) if !parsed.is_empty() => {
                self.catalog = parsed;
                match sources.and_then(|bytes| parse_sources(bytes).ok()) {
                    Some(parsed) => {
                        self.sources = Some(parsed);
                        self.sources_ready = true;
                        self.status = format!("{} items loaded.", self.catalog.len());
                    }
                    None => {
                        self.sources = None;
                        self.sources_ready = false;
                        self.status = format!("{} items loaded. {MISSING_SOURCES_STATUS}", self.catalog.len());
                    }
                }
            }
            _ => {
                self.catalog = FinderCatalog::default();
                self.sources = None;
                self.sources_ready = false;
                self.rows.clear();
                self.status = MISSING_CATALOG_STATUS.to_owned();
            }
        }
        if !self.catalog.is_empty() {
            self.search();
        }
    }

    pub fn set_category(&mut self, category: &str) {
        self.category = category.to_owned();
        if !self.catalog.is_empty() {
            self.search();
        }
    }

    pub fn catalog_is_empty(&self) -> bool {
        self.catalog.is_empty()
    }

    /// Name picker for shop search. A numeric query is an exact item id.
    pub fn name_matches(&self, query: &str, limit: usize) -> Vec<(u32, String)> {
        crate::state::player_shops::match_item_names(
            self.catalog.items.iter().map(|item| (item.id, item.name.as_str())),
            query,
            limit,
        )
    }

    pub fn search(&mut self) {
        if self.catalog.is_empty() {
            self.rows.clear();
            self.status = MISSING_CATALOG_STATUS.to_owned();
            return;
        }
        let hits = search_catalog(&self.catalog, &self.current_query());
        self.fill_rows(&hits);
    }

    fn current_query(&self) -> FinderQuery {
        FinderQuery {
            category: self.category.clone(),
            text: self.query.clone(),
            effect_kinds: Vec::new(),
            effect_target: String::new(),
            job: String::new(),
            max_equip_level: None,
            unconditional_only: false,
        }
    }

    fn fill_rows(&mut self, hits: &[FinderHit]) {
        let shown = hits.len().min(PAGE);
        let mut rows = Vec::new();
        for hit in hits.iter().take(PAGE) {
            rows.push(hit.summary.clone());
            if let Some(sources) = &self.sources {
                let mvp = self.catalog.get(hit.item_id).and_then(|item| item.mvp_item_id);
                push_source_rows(&mut rows, &self.catalog, sources, hit.item_id, mvp);
            }
        }
        self.rows = rows;
        let mut status = format!("{shown} of {} result(s).", hits.len());
        if !self.sources_ready {
            status.push(' ');
            status.push_str(MISSING_SOURCES_STATUS);
        }
        self.status = status;
    }
}

const DROP_PREVIEW: usize = 4;
const QUEST_PREVIEW: usize = 4;

/// One summary row plus drop and quest lines from [`drop_lines`] and [`quest_files`].
fn push_source_rows(rows: &mut Vec<String>, catalog: &FinderCatalog, sources: &ItemSources, item_id: u32, mvp_item_id: Option<u32>) {
    let lines = drop_lines(catalog, sources, item_id, 0);
    let shown = lines.len().min(DROP_PREVIEW);
    for line in lines.iter().take(shown) {
        let kind = if line.mvp_reward { "MVP drop" } else { "Drop" };
        let tag = if line.tag.is_empty() {
            String::new()
        } else {
            format!(" ({})", line.tag)
        };
        let spawn = line.spawns.first().map(String::as_str).unwrap_or("");
        rows.push(format!(
            "    {kind}: {} Lv{} {:.2}%{tag} · {spawn}",
            line.mob_name,
            line.mob_level,
            line.rate as f64 / 100.0
        ));
    }
    if lines.len() > shown {
        rows.push(format!("    … {} more drop(s)", lines.len() - shown));
    }
    let quests = quest_files(sources, item_id, mvp_item_id);
    let shown_quests = quests.len().min(QUEST_PREVIEW);
    for quest in quests.iter().take(shown_quests) {
        rows.push(format!("    Quest: {quest}"));
    }
    if quests.len() > shown_quests {
        rows.push(format!("    … {} more quest file(s)", quests.len() - shown_quests));
    }
}

pub fn parse_catalog(bytes: &[u8]) -> Result<FinderCatalog, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let Some(items) = value.get("items").and_then(Value::as_array) else {
        return Err("catalog has no items array".to_owned());
    };
    let mut parsed = Vec::with_capacity(items.len());
    for item in items {
        if let Some(item) = parse_item(item) {
            parsed.push(item);
        }
    }
    if parsed.is_empty() {
        return Err("catalog items were empty".to_owned());
    }
    Ok(FinderCatalog { items: parsed })
}

fn parse_item(value: &Value) -> Option<FinderItem> {
    let id = value.get("id").and_then(Value::as_u64)? as u32;
    let name = value.get("n").and_then(Value::as_str).unwrap_or("").to_owned();
    let category = value.get("c").and_then(Value::as_str).unwrap_or("").to_owned();
    let effects: Vec<FinderEffect> = value
        .get("eff")
        .and_then(Value::as_array)
        .map(|effects| effects.iter().filter_map(parse_effect).collect())
        .unwrap_or_default();
    let description_text = value
        .get("d")
        .and_then(Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .filter_map(|line| line.get("x").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let effect_text = effects.iter().map(|effect| effect.text.as_str()).collect::<Vec<_>>().join(" ");
    let search_blob = format!("{name} {description_text} {effect_text}").to_lowercase();
    let flat_mdef = effects
        .iter()
        .filter(|effect| effect.kind == "mdef" && effect.target == "MDEF")
        .filter_map(|effect| effect.magnitude)
        .map(|value| value as i32)
        .sum();
    Some(FinderItem {
        id,
        name,
        category,
        weapon_type: value.get("wt").and_then(Value::as_str).unwrap_or("").to_owned(),
        locations: string_list(value.get("loc")),
        attack: json_i32(value.get("atk")),
        magic_attack: json_i32(value.get("matk")),
        defense: json_i32(value.get("def")),
        flat_mdef,
        weapon_level: json_i32(value.get("wl")),
        slots: json_i32(value.get("s")),
        weight: json_i32(value.get("w")),
        equip_level: json_i32(value.get("lv")),
        jobs: value.get("j").and_then(Value::as_str).unwrap_or("").to_owned(),
        mvp_item_id: value.get("mvp").and_then(Value::as_u64).map(|id| id as u32),
        effects,
        search_blob,
    })
}

fn parse_effect(value: &Value) -> Option<FinderEffect> {
    Some(FinderEffect {
        kind: value.get("k").and_then(Value::as_str).unwrap_or("").to_owned(),
        target: value.get("t").and_then(Value::as_str).unwrap_or("").to_owned(),
        text: value.get("x").and_then(Value::as_str).unwrap_or("").to_owned(),
        conditional: value.get("c").and_then(Value::as_bool).unwrap_or(false) || value.get("c").and_then(Value::as_i64).unwrap_or(0) != 0,
        magnitude: value
            .get("m")
            .and_then(Value::as_f64)
            .or_else(|| value.get("v").and_then(Value::as_f64)),
    })
}

pub fn parse_sources(bytes: &[u8]) -> Result<ItemSources, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let mut sources = ItemSources::default();
    if let Some(mobs) = value.get("mobs").and_then(Value::as_object) {
        for (key, mob) in mobs {
            let Ok(id) = key.parse::<u32>() else { continue };
            sources.mobs.insert(id, parse_mob(mob));
        }
    }
    if let Some(drops) = value.get("drops").and_then(Value::as_object) {
        for (key, rows) in drops {
            let Ok(id) = key.parse::<u32>() else { continue };
            let Some(rows) = rows.as_array() else { continue };
            let parsed = rows.iter().filter_map(parse_drop).collect();
            sources.drops.insert(id, parsed);
        }
    }
    if let Some(quests) = value.get("quests").and_then(Value::as_object) {
        for (key, files) in quests {
            let Ok(id) = key.parse::<u32>() else { continue };
            sources.quests.insert(id, string_list(Some(files)));
        }
    }
    if let Some(penalty) = value.get("penalty").and_then(Value::as_object) {
        sources.penalty_boss = penalty_rows(penalty.get("11"));
        sources.penalty_normal = penalty_rows(penalty.get("12"));
    }
    Ok(sources)
}

fn parse_mob(value: &Value) -> FinderMob {
    let spawns = value
        .get("sp")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let row = row.as_array()?;
                    Some(MobSpawn {
                        map: row.first().and_then(Value::as_str).unwrap_or("").to_owned(),
                        name: row.get(1).and_then(Value::as_str).unwrap_or("").to_owned(),
                        count: row.get(2).and_then(Value::as_i64).unwrap_or(0),
                        respawn_ms: row.get(3).and_then(Value::as_i64).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    FinderMob {
        name: value.get("n").and_then(Value::as_str).unwrap_or("").to_owned(),
        level: json_i32(value.get("lv")),
        boss: value.get("b").and_then(Value::as_bool).unwrap_or(false) || json_i32(value.get("b")) != 0,
        mvp: value.get("mvp").and_then(Value::as_bool).unwrap_or(false) || json_i32(value.get("mvp")) != 0,
        spawns,
    }
}

fn parse_drop(value: &Value) -> Option<RawDrop> {
    let row = value.as_array()?;
    Some(RawDrop {
        mob_id: row.first().and_then(Value::as_u64)? as u32,
        rate: row.get(1).and_then(Value::as_i64).unwrap_or(0),
        mvp_reward: row.get(2).and_then(Value::as_bool).unwrap_or(false) || row.get(2).and_then(Value::as_i64).unwrap_or(0) != 0,
    })
}

fn penalty_rows(value: Option<&Value>) -> Vec<(i32, i32)> {
    value
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let row = row.as_array()?;
                    Some((json_i32(row.first()), json_i32(row.get(1))))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn search_catalog(catalog: &FinderCatalog, query: &FinderQuery) -> Vec<FinderHit> {
    let text = query.text.trim().to_lowercase();
    let target = query.effect_target.trim().to_lowercase();
    let job = query.job.trim().to_lowercase();
    let kinds: std::collections::HashSet<&str> = query.effect_kinds.iter().map(String::as_str).collect();
    let effect_filter = !kinds.is_empty() || !target.is_empty();
    let mut hits = Vec::new();
    for item in &catalog.items {
        if item.category != query.category {
            continue;
        }
        if !text.is_empty() && !item.search_blob.contains(&text) {
            continue;
        }
        if let Some(level) = query.max_equip_level
            && item.equip_level > level
        {
            continue;
        }
        if !job.is_empty() {
            let jobs = item.jobs.to_lowercase();
            if !jobs.starts_with("every job") && !jobs.contains(&job) {
                continue;
            }
            if jobs.contains(&format!("except {job}")) {
                continue;
            }
        }
        if effect_filter {
            let matched = item
                .effects
                .iter()
                .any(|effect| effect_matches(effect, &kinds, &target, query.unconditional_only));
            if !matched {
                continue;
            }
        }
        hits.push(FinderHit {
            item_id: item.id,
            name: item.name.clone(),
            summary: summarize(item),
        });
    }
    hits.sort_by(|left, right| left.name.cmp(&right.name).then(left.item_id.cmp(&right.item_id)));
    hits
}

fn effect_matches(effect: &FinderEffect, kinds: &std::collections::HashSet<&str>, target: &str, unconditional_only: bool) -> bool {
    if unconditional_only && effect.conditional {
        return false;
    }
    if !kinds.is_empty() && !kinds.contains(effect.kind.as_str()) {
        return false;
    }
    if !target.is_empty() && !effect.target.to_lowercase().contains(target) && !effect.text.to_lowercase().contains(target) {
        return false;
    }
    !kinds.is_empty() || !target.is_empty()
}

fn summarize(item: &FinderItem) -> String {
    let mut parts = Vec::new();
    match item.category.as_str() {
        "weapon" => {
            parts.push(item.weapon_type.clone());
            let mut attack = format!("ATK {}", item.attack);
            if item.magic_attack != 0 {
                attack.push_str(&format!(" MATK {}", item.magic_attack));
            }
            parts.push(attack);
            parts.push(format!("Wpn Lv {}", item.weapon_level));
        }
        "armor" => {
            parts.push(item.locations.join("/"));
            let mut defense = format!("DEF {}", item.defense);
            if item.flat_mdef != 0 {
                defense.push_str(&format!(" MDEF {}", item.flat_mdef));
            }
            parts.push(defense);
        }
        "card" | "enchant" => parts.push(if item.locations.is_empty() {
            "Card".to_owned()
        } else {
            item.locations.join("/")
        }),
        _ => {}
    }
    if item.category != "card" && item.category != "enchant" {
        parts.push(format!("Slots {}", item.slots));
    }
    parts.push(format!("Wt {}", item.weight));
    if item.equip_level != 0 {
        parts.push(format!("Lv {}", item.equip_level));
    }
    let slots = if item.slots > 0 && item.category != "card" && item.category != "enchant" {
        format!(" [{}]", item.slots)
    } else {
        String::new()
    };
    format!("{}{} #{} · {}", item.name, slots, item.id, parts.join(" · "))
}

/// Monster drop lines for one item. An explicit catalog `mvp` id is the slotted
/// sibling. When that field is absent, the sibling is `id + 40000` if that id
/// is in the catalog. The Item Finder window calls this with player level 0,
/// which keeps the server rate (no level-difference penalty).
pub fn drop_lines(catalog: &FinderCatalog, sources: &ItemSources, item_id: u32, player_level: i32) -> Vec<DropLine> {
    let mut raw = Vec::new();
    push_drops(sources, item_id, "", &mut raw);
    let mvp_sibling = catalog.get(item_id).and_then(|item| {
        item.mvp_item_id.or_else(|| {
            let sibling = item_id.checked_add(40000)?;
            catalog.get(sibling).map(|_| sibling)
        })
    });
    if let Some(mvp_id) = mvp_sibling {
        push_drops(sources, mvp_id, "slotted MVP version", &mut raw);
    }
    let mut lines: Vec<DropLine> = raw
        .into_iter()
        .map(|(drop, tag)| {
            let mob = sources.mobs.get(&drop.mob_id);
            let name = mob.map(|mob| mob.name.clone()).unwrap_or_else(|| format!("#{}", drop.mob_id));
            let level = mob.map(|mob| mob.level).unwrap_or(0);
            let boss = mob.map(|mob| mob.boss).unwrap_or(false);
            let mvp_monster = mob.map(|mob| mob.mvp).unwrap_or(false);
            let penalty = penalty_percent(boss, level, player_level, sources);
            let rate = if drop.mvp_reward {
                drop.rate
            } else {
                ((drop.rate * i64::from(penalty)) / 100).max(1)
            }
            .min(10_000);
            let spawns = mob
                .map(|mob| {
                    if mob.spawns.is_empty() {
                        vec!["No natural spawn (event / summon / instance only)".to_owned()]
                    } else {
                        mob.spawns
                            .iter()
                            .map(|spawn| format!("{} ({}) x{} respawn {}", spawn.name, spawn.map, spawn.count, spawn.respawn_ms))
                            .collect()
                    }
                })
                .unwrap_or_else(|| vec!["Unknown monster".to_owned()]);
            DropLine {
                mob_name: name,
                mob_level: level,
                boss,
                mvp_monster,
                mvp_reward: drop.mvp_reward,
                tag: tag.to_owned(),
                rate,
                spawns,
            }
        })
        .collect();
    lines.sort_by(|left, right| right.rate.cmp(&left.rate).then(left.mob_name.cmp(&right.mob_name)));
    lines
}

#[cfg_attr(not(test), allow(dead_code))]
fn push_drops<'a>(sources: &'a ItemSources, item_id: u32, tag: &'a str, out: &mut Vec<(&'a RawDrop, &'a str)>) {
    if let Some(drops) = sources.drops.get(&item_id) {
        for drop in drops {
            out.push((drop, tag));
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn penalty_percent(boss: bool, mob_level: i32, player_level: i32, sources: &ItemSources) -> i32 {
    if player_level == 0 {
        return 100;
    }
    let rows = if boss { &sources.penalty_boss } else { &sources.penalty_normal };
    let difference = mob_level - player_level;
    let mut rate = 100;
    if difference >= 0 {
        for (threshold, row_rate) in rows {
            if *threshold >= 0 && difference >= *threshold {
                rate = *row_rate;
            }
        }
    } else {
        for (threshold, row_rate) in rows.iter().rev() {
            if *threshold <= 0 && difference <= *threshold {
                rate = *row_rate;
            }
        }
    }
    rate
}

pub fn quest_files(sources: &ItemSources, item_id: u32, mvp_item_id: Option<u32>) -> Vec<String> {
    sources
        .quests
        .get(&item_id)
        .or_else(|| mvp_item_id.and_then(|id| sources.quests.get(&id)))
        .cloned()
        .unwrap_or_default()
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(|value| value.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

fn json_i32(value: Option<&Value>) -> i32 {
    value.and_then(Value::as_i64).unwrap_or(0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = r#"{
        "items": [
            {"id": 1201, "n": "Knife", "c": "weapon", "wt": "Dagger", "atk": 17, "wl": 1, "s": 3, "w": 40, "lv": 1, "j": "Every Job",
             "eff": [{"k": "crit", "t": "Crit", "x": "Crit +10", "v": 10}]},
            {"id": 2301, "n": "Cotton Shirt", "c": "armor", "loc": ["Armor"], "def": 10, "s": 1, "w": 10, "lv": 50, "j": "Every Job except Novice",
             "eff": [{"k": "mdef", "t": "MDEF", "x": "MDEF +5", "v": 5}, {"k": "str", "t": "STR", "x": "STR +1 if base level > 99", "v": 1, "c": true}]},
            {"id": 4001, "n": "Poring Card", "c": "card", "loc": ["Armor"], "j": "Every Job",
             "eff": [{"k": "luk", "t": "LUK", "x": "LUK +2", "m": 2}]}
        ]
    }"#;

    const SOURCES: &str = r#"{
        "mobs": {
            "1002": {"n": "Poring", "lv": 1, "sp": [["prt_fild01", "Poring", 10, 5000]]},
            "1039": {"n": "Baphomet", "lv": 81, "b": 1, "mvp": 1, "sp": []}
        },
        "drops": {
            "1201": [[1002, 500, 0]],
            "41201": [[1039, 100, 1]]
        },
        "quests": {"1201": ["npc/quests/knife.txt"]},
        "penalty": {"11": [[0, 100], [10, 50]], "12": [[-10, 80], [0, 100]]}
    }"#;

    #[test]
    fn missing_bytes_keep_the_a5_stub() {
        let mut state = ItemFinderState::default();
        state.load_bytes(None, None);
        assert!(state.catalog.is_empty());
        assert!(state.status.contains("A5"));
        assert!(state.status.contains(CATALOG_PATH));
        assert!(state.rows.is_empty());
    }

    #[test]
    fn catalog_search_filters_category_level_job_and_effect() {
        let catalog = parse_catalog(CATALOG.as_bytes()).unwrap();
        let weapons = search_catalog(&catalog, &FinderQuery {
            text: "knife".to_owned(),
            effect_kinds: vec!["crit".to_owned()],
            ..FinderQuery::default()
        });
        assert_eq!(weapons.len(), 1);
        assert_eq!(weapons[0].item_id, 1201);
        assert!(weapons[0].summary.contains("ATK 17"));

        let shirts = search_catalog(&catalog, &FinderQuery {
            category: "armor".to_owned(),
            job: "novice".to_owned(),
            ..FinderQuery::default()
        });
        assert!(shirts.is_empty());

        let high_level = search_catalog(&catalog, &FinderQuery {
            category: "armor".to_owned(),
            max_equip_level: Some(40),
            ..FinderQuery::default()
        });
        assert!(high_level.is_empty());

        let conditional = search_catalog(&catalog, &FinderQuery {
            category: "armor".to_owned(),
            effect_kinds: vec!["str".to_owned()],
            unconditional_only: true,
            ..FinderQuery::default()
        });
        assert!(conditional.is_empty());

        let cards = search_catalog(&catalog, &FinderQuery {
            category: "card".to_owned(),
            effect_target: "luk".to_owned(),
            ..FinderQuery::default()
        });
        assert_eq!(cards[0].item_id, 4001);
    }

    #[test]
    fn sources_apply_penalty_and_slotted_sibling() {
        let catalog_json = r#"{"items":[
            {"id":1201,"n":"Knife","c":"weapon","j":"Every Job","eff":[]},
            {"id":41201,"n":"Knife","c":"weapon","j":"Every Job","mvp":1,"eff":[]}
        ]}"#;
        let catalog = parse_catalog(catalog_json.as_bytes()).unwrap();
        let sources = parse_sources(SOURCES.as_bytes()).unwrap();
        assert_eq!(penalty_percent(false, 1, 20, &sources), 80);
        let lines = drop_lines(&catalog, &sources, 1201, 20);
        assert_eq!(lines[0].mob_name, "Poring");
        assert_eq!(lines[0].rate, 400);
        assert!(
            lines
                .iter()
                .any(|line| line.tag == "slotted MVP version" && line.mvp_reward && line.rate == 100)
        );
        let quests = quest_files(&sources, 1201, None);
        assert_eq!(quests, vec!["npc/quests/knife.txt".to_owned()]);
    }

    #[test]
    fn explicit_mvp_id_is_used_instead_of_plus_40000() {
        let catalog_json = r#"{"items":[
            {"id":1201,"n":"Knife","c":"weapon","j":"Every Job","mvp":50001,"eff":[]},
            {"id":41201,"n":"Other Knife","c":"weapon","j":"Every Job","eff":[]},
            {"id":4294967195,"n":"Wide Id","c":"weapon","j":"Every Job","eff":[]},
            {"id":4294967295,"n":"Max Id","c":"weapon","j":"Every Job","eff":[]}
        ]}"#;
        let sources_json = r#"{
            "mobs": {"1039": {"n": "Baphomet", "lv": 81, "b": 1, "mvp": 1, "sp": []}},
            "drops": {
                "41201": [[1039, 50, 0]],
                "50001": [[1039, 100, 1]]
            },
            "quests": {},
            "penalty": {"11": [[0, 100]], "12": [[0, 100]]}
        }"#;
        let catalog = parse_catalog(catalog_json.as_bytes()).unwrap();
        let sources = parse_sources(sources_json.as_bytes()).unwrap();
        let lines = drop_lines(&catalog, &sources, 1201, 0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].tag, "slotted MVP version");
        assert_eq!(lines[0].rate, 100);
        // 4294967195 + 40000 overflows u32. The +40000 heuristic must not wrap.
        assert!(drop_lines(&catalog, &sources, 4_294_967_195, 0).is_empty());
    }

    #[test]
    fn loaded_catalog_without_sources_still_lists_rows() {
        let mut state = ItemFinderState {
            query: "knife".to_owned(),
            ..ItemFinderState::default()
        };
        state.load_bytes(Some(CATALOG.as_bytes()), None);
        assert!(state.status.contains(SOURCES_PATH));
        assert!(state.status.contains("A5"));
        assert_eq!(state.rows.len(), 1);
        assert!(state.rows[0].contains("#1201"));
    }

    #[test]
    fn search_rows_include_drop_and_quest_lines() {
        let mut state = ItemFinderState {
            query: "knife".to_owned(),
            ..ItemFinderState::default()
        };
        state.load_bytes(Some(CATALOG.as_bytes()), Some(SOURCES.as_bytes()));
        assert!(state.rows.iter().any(|row| row.contains("Drop:") && row.contains("Poring") && row.contains("5.00%")));
        assert!(state.rows.iter().any(|row| row.contains("Quest: npc/quests/knife.txt")));
    }

    #[test]
    fn shipped_itemfinder_json_matches_the_loaders() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("archive/System");
        let catalog_bytes = std::fs::read(root.join("itemfinder.turoran.v8.json")).expect("catalog");
        let sources_bytes = std::fs::read(root.join("itemsources.turoran.v3.json")).expect("sources");
        let catalog = parse_catalog(&catalog_bytes).expect("catalog schema");
        let sources = parse_sources(&sources_bytes).expect("sources schema");
        assert!(catalog.len() > 1000);
        assert!(!sources.drops.is_empty());
        assert!(!sources.quests.is_empty());
        assert!(!sources.penalty_normal.is_empty());
        let mut state = ItemFinderState::default();
        state.load_bytes(Some(&catalog_bytes), Some(&sources_bytes));
        assert!(state.sources_ready);
        assert!(!state.rows.is_empty());
    }
}
