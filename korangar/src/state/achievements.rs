use std::sync::Arc;

use hashbrown::HashMap;
use korangar_interface::element::StateElement;
use ragnarok_packets::{AchievementData, AttendanceReward};
use rust_state::RustState;

use crate::graphics::Texture;
use crate::state::{ClientState, ClientStatePathExt, client_state};
use crate::world::{
    AchievementCatalog, AchievementInfo, AchievementReward, AttendanceTable, ItemName, ItemNameKey, Library, SkillListInformation, Table,
};

/// Share the library's achievement, attendance and navigation tables with
/// the window state once at startup. Logout keeps them.
pub fn attach_library_tables(state: &mut rust_state::State<ClientState>, library: &Library) {
    let catalog = library.achievement_catalog();
    let names = RewardNames::resolve(
        &catalog,
        |item_id| {
            ItemName::try_get(library, ItemNameKey {
                item_id: ragnarok_packets::ItemId(item_id),
                is_identified: true,
            })
            .map(|name| name.to_string())
        },
        |title_id| library.title_name(title_id).map(str::to_owned),
        |skill_id| {
            let skill_id = u16::try_from(skill_id).ok()?;
            SkillListInformation::try_get(library, ragnarok_packets::SkillId(skill_id)).map(|skill| skill.name.clone())
        },
    );
    let achievements = state.follow_mut(client_state().achievements());
    achievements.attach_reward_names(names);
    achievements.attach_tables(catalog, library.attendance_table());
    state
        .follow_mut(client_state().navigation())
        .attach_navigation_tables(library.navigation_tables());
}

/// Display names for the reward ids in the achievement table: item names,
/// `titletable.lub` titles and the skill names roBrowser shows for buffs.
/// Resolved once from the library; an id without a name shows the id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RewardNames {
    pub items: HashMap<u32, String>,
    pub titles: HashMap<u32, String>,
    pub buffs: HashMap<u32, String>,
}

impl RewardNames {
    pub fn resolve(
        catalog: &AchievementCatalog,
        item: impl Fn(u32) -> Option<String>,
        title: impl Fn(u32) -> Option<String>,
        buff: impl Fn(u32) -> Option<String>,
    ) -> Self {
        let mut names = Self::default();
        let named = |name: Option<String>| name.map(|name| name.trim().to_owned()).filter(|name| !name.is_empty());
        for info in catalog.values() {
            if let Some(id) = info.reward.item_id
                && let Some(name) = named(item(id))
            {
                names.items.insert(id, name);
            }
            if let Some(id) = info.reward.title_id
                && let Some(name) = named(title(id))
            {
                names.titles.insert(id, name);
            }
            if let Some(id) = info.reward.buff_id
                && let Some(name) = named(buff(id))
            {
                names.buffs.insert(id, name);
            }
        }
        names
    }
}

/// The detail panel's reward line.
pub fn reward_line(reward: &AchievementReward, names: &RewardNames) -> String {
    let mut parts = Vec::new();
    if let Some(id) = reward.item_id {
        parts.push(match names.items.get(&id) {
            Some(name) => format!("{name} (item)"),
            None => format!("item {id}"),
        });
    }
    if let Some(id) = reward.title_id {
        parts.push(match names.titles.get(&id) {
            Some(name) => format!("title \"{name}\""),
            None => format!("title {id}"),
        });
    }
    if let Some(id) = reward.buff_id {
        parts.push(match names.buffs.get(&id) {
            Some(name) => format!("buff {name}"),
            None => format!("buff {id}"),
        });
    }
    if parts.is_empty() {
        "Reward: none".to_owned()
    } else {
        format!("Reward: {}", parts.join(", "))
    }
}

/// One achievement row for the list window.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct TrackedAchievement {
    pub achievement_id: u32,
    pub completed: bool,
    pub rewarded: bool,
    pub objective_hits: u8,
    /// Catalog title, or empty when the client table has no entry.
    pub title: String,
    pub major: u8,
    pub minor: u8,
    pub score: u32,
    pub summary: String,
}

/// Objective slots on `ZC_ACHIEVEMENT_LIST`. The bar uses how many are above 0.
pub const OBJECTIVE_SLOTS: u8 = 10;

/// roBrowser `Achievement.js` tabs (msgstringtable 2656-2678). Major 0 is the
/// summary page and lists everything.
pub const ACHIEVEMENT_MAJORS: &[(u8, &str, &[&str])] = &[
    (0, "Summary", &[]),
    (1, "General", &["Character", "Action", "Other"]),
    (2, "Adventure", &[
        "Rune Midgarts",
        "Schwartzvald",
        "Arunafeltz",
        "New World",
        "Localizing",
        "Dungeon",
    ]),
    (3, "Battle", &["Duel", "Training"]),
    (4, "Quest", &["Episode", "General"]),
    (5, "Memorial", &["Midgard", "New World", "Other"]),
    (6, "Feat", &[]),
];

pub fn major_name(major: u8) -> &'static str {
    ACHIEVEMENT_MAJORS
        .iter()
        .find(|(id, ..)| *id == major)
        .map(|(_, name, _)| *name)
        .unwrap_or("Other")
}

pub fn minor_name(major: u8, minor: u8) -> Option<&'static str> {
    ACHIEVEMENT_MAJORS
        .iter()
        .find(|(id, ..)| *id == major)
        .and_then(|(_, _, minors)| minors.get(minor as usize).copied())
}

/// Tab guess for ids the client table does not know, from the official id
/// ranges (`id / 10000`).
fn fallback_major(achievement_id: u32) -> u8 {
    match achievement_id / 10_000 {
        12 => 2,
        20 | 22 => 1,
        23 => 3,
        11 | 13 | 17 | 19 => 4,
        _ => 6,
    }
}

fn achievement_state(completed: bool, rewarded: bool) -> &'static str {
    if rewarded {
        "rewarded"
    } else if completed {
        "complete"
    } else {
        "in progress"
    }
}

fn objective_hits(entry: &AchievementData) -> u8 {
    entry.objectives.iter().filter(|value| **value > 0).count() as u8
}

fn summary_text(id: u32, title: &str, completed: bool, rewarded: bool, hits: u8) -> String {
    let state = achievement_state(completed, rewarded);
    if title.is_empty() {
        format!("#{id:04} {state} ({hits}/{OBJECTIVE_SLOTS})")
    } else {
        format!("{title} · {state} ({hits}/{OBJECTIVE_SLOTS})")
    }
}

/// Filters that match the achievement window radios: all, complete,
/// incomplete, and rewarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AchievementFilter {
    All = 0,
    Complete = 1,
    Incomplete = 2,
    Rewarded = 3,
}

impl AchievementFilter {
    fn cycle(self) -> Self {
        match self {
            Self::All => Self::Complete,
            Self::Complete => Self::Incomplete,
            Self::Incomplete => Self::Rewarded,
            Self::Rewarded => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Complete => "Complete",
            Self::Incomplete => "Incomplete",
            Self::Rewarded => "Rewarded",
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Complete,
            2 => Self::Incomplete,
            3 => Self::Rewarded,
            _ => Self::All,
        }
    }

    fn matches(self, entry: &TrackedAchievement) -> bool {
        match self {
            Self::All => true,
            Self::Complete => entry.completed && !entry.rewarded,
            Self::Incomplete => !entry.completed,
            Self::Rewarded => entry.rewarded,
        }
    }
}

/// One list row the window draws and clicks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AchievementRow {
    pub achievement_id: u32,
    pub text: String,
    pub score: u32,
    pub objective_hits: u8,
    pub claimable: bool,
    pub rewarded: bool,
    pub selected: bool,
}

/// Per-tab completion for the summary page gauges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryProgress {
    pub major: u8,
    pub name: &'static str,
    pub completed: u32,
    pub total: u32,
}

/// Hercules ships 20 attendance days by default. Used until the client table
/// or `0x0AE3` gives the real reward list.
pub const DEFAULT_ATTENDANCE_DAYS: usize = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AttendanceCellState {
    Claimed,
    Today,
    #[default]
    Upcoming,
}

/// One calendar cell. `label` is `Day N` plus the reward when known.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttendanceCell {
    pub label: String,
    pub state: AttendanceCellState,
    /// Reward item, for the icon. 0 when unknown.
    pub item_id: u32,
}

/// Hercules `ZC_UI_OPEN` attendance payload: `days` already includes today
/// once it is claimed. Unclaimed, today is the cell right after the last
/// claimed day.
pub fn attendance_cell_state(index: usize, days_attended: u32, claimed_today: bool) -> AttendanceCellState {
    let days = days_attended as usize;
    if index < days {
        AttendanceCellState::Claimed
    } else if index == days && !claimed_today {
        AttendanceCellState::Today
    } else {
        AttendanceCellState::Upcoming
    }
}

/// Client tables shared from the library. `Debug` prints sizes only.
#[derive(Clone, Default)]
pub struct AchievementTables {
    catalog: Option<Arc<AchievementCatalog>>,
    attendance: Option<Arc<AttendanceTable>>,
}

impl std::fmt::Debug for AchievementTables {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AchievementTables")
            .field("catalog", &self.catalog.as_ref().map(|catalog| catalog.len()))
            .field("attendance_days", &self.attendance.as_ref().map(|table| table.rewards.len()))
            .finish()
    }
}

/// Achievement list and the latest attendance period.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct Achievements {
    total_score: u32,
    level: u16,
    experience: u32,
    experience_to_next_level: u32,
    entries: Vec<TrackedAchievement>,
    attendance_count: u32,
    attendance_start: u32,
    attendance_end: u32,
    attendance_known: bool,
    attendance_rewards: Vec<AttendanceReward>,
    pub attendance_status: String,
    #[hidden_element]
    attendance_claimed_today: bool,
    #[hidden_element]
    attendance_cells: Vec<AttendanceCell>,
    lines: Vec<String>,
    filter_label: String,
    category_label: String,
    #[hidden_element]
    filter: u8,
    /// Major tab. 0 is the summary page (every achievement).
    #[hidden_element]
    major: u8,
    #[hidden_element]
    minor: Option<u8>,
    #[hidden_element]
    selected: Option<u32>,
    #[hidden_element]
    rows: Vec<AchievementRow>,
    #[hidden_element]
    detail: Vec<String>,
    #[hidden_element]
    tables: AchievementTables,
    #[hidden_element]
    item_icons: HashMap<u32, Arc<Texture>>,
    #[hidden_element]
    reward_names: RewardNames,
}

impl Achievements {
    pub fn attach_reward_names(&mut self, names: RewardNames) {
        self.reward_names = names;
        self.rebuild_lines();
    }

    /// Item reward of the selected achievement, for the detail icon.
    pub fn selected_reward_item(&self) -> Option<u32> {
        self.info(self.selected?)?.reward.item_id.filter(|item_id| *item_id != 0)
    }

    pub fn attach_tables(&mut self, catalog: Arc<AchievementCatalog>, attendance: Arc<AttendanceTable>) {
        self.tables = AchievementTables {
            catalog: Some(catalog),
            attendance: Some(attendance),
        };
        let entries = std::mem::take(&mut self.entries);
        self.entries = entries.into_iter().map(|entry| self.annotate(entry)).collect();
        self.rebuild_lines();
    }

    fn info(&self, achievement_id: u32) -> Option<&AchievementInfo> {
        self.tables.catalog.as_ref()?.get(&achievement_id)
    }

    /// Fill title, tab and score from the client table.
    fn annotate(&self, mut entry: TrackedAchievement) -> TrackedAchievement {
        match self.info(entry.achievement_id) {
            Some(info) => {
                entry.title = info.title.clone();
                entry.major = info.major;
                entry.minor = info.minor;
                entry.score = info.score;
            }
            None => {
                entry.title.clear();
                entry.major = fallback_major(entry.achievement_id);
                entry.minor = 0;
                entry.score = 0;
            }
        }
        entry.summary = summary_text(
            entry.achievement_id,
            &entry.title,
            entry.completed,
            entry.rewarded,
            entry.objective_hits,
        );
        entry
    }

    fn track(&self, entry: AchievementData) -> TrackedAchievement {
        let tracked = TrackedAchievement {
            achievement_id: entry.acheivement_id,
            completed: entry.is_completed != 0,
            rewarded: entry.got_rewarded != 0,
            objective_hits: objective_hits(&entry),
            title: String::new(),
            major: 0,
            minor: 0,
            score: 0,
            summary: String::new(),
        };
        self.annotate(tracked)
    }

    pub fn replace_list(
        &mut self,
        total_score: u32,
        level: u16,
        experience: u32,
        experience_to_next_level: u32,
        entries: Vec<AchievementData>,
    ) {
        self.set_rank(total_score, level, experience, experience_to_next_level);
        self.entries = entries.into_iter().map(|entry| self.track(entry)).collect();
        self.entries.sort_by_key(|entry| entry.achievement_id);
        self.rebuild_lines();
    }

    pub fn upsert(&mut self, total_score: u32, level: u16, experience: u32, experience_to_next_level: u32, entry: AchievementData) {
        self.set_rank(total_score, level, experience, experience_to_next_level);
        let tracked = self.track(entry);
        if let Some(existing) = self.entries.iter_mut().find(|row| row.achievement_id == tracked.achievement_id) {
            *existing = tracked;
        } else {
            self.entries.push(tracked);
            self.entries.sort_by_key(|row| row.achievement_id);
        }
        self.rebuild_lines();
    }

    pub fn apply_reward(&mut self, achievement_id: u32, failed: bool) {
        if failed {
            return;
        }
        if let Some(entry) = self.entries.iter_mut().find(|row| row.achievement_id == achievement_id) {
            entry.rewarded = true;
            entry.completed = true;
            entry.summary = summary_text(achievement_id, &entry.title, true, true, entry.objective_hits);
        }
        self.rebuild_lines();
    }

    pub fn cycle_filter(&mut self) {
        let next = AchievementFilter::from_code(self.filter).cycle();
        self.filter = next as u8;
        self.rebuild_lines();
    }

    /// Next major tab, wrapping back to the summary.
    pub fn cycle_category(&mut self) {
        let index = ACHIEVEMENT_MAJORS.iter().position(|(id, ..)| *id == self.major).unwrap_or(0);
        let next = ACHIEVEMENT_MAJORS[(index + 1) % ACHIEVEMENT_MAJORS.len()].0;
        self.select_category(next, None);
    }

    /// A major tab click clears the minor tab; a minor click keeps the major.
    pub fn select_category(&mut self, major: u8, minor: Option<u8>) {
        self.major = major;
        self.minor = minor.filter(|minor| minor_name(major, *minor).is_some());
        self.rebuild_lines();
    }

    pub fn select_achievement(&mut self, achievement_id: u32) {
        self.selected = Some(achievement_id);
        self.rebuild_lines();
    }

    pub fn major(&self) -> u8 {
        self.major
    }

    pub fn minor(&self) -> Option<u8> {
        self.minor
    }

    pub fn rows(&self) -> &[AchievementRow] {
        &self.rows
    }

    /// Title, description, goals and rewards of the selected achievement.
    pub fn detail(&self) -> &[String] {
        &self.detail
    }

    pub fn experience_fraction(&self) -> f32 {
        if self.experience_to_next_level == 0 {
            0.0
        } else {
            (self.experience as f32 / self.experience_to_next_level as f32).clamp(0.0, 1.0)
        }
    }

    /// Completed/total per major tab, roBrowser's summary gauges.
    pub fn category_progress(&self) -> Vec<CategoryProgress> {
        ACHIEVEMENT_MAJORS
            .iter()
            .filter(|(id, ..)| *id != 0)
            .map(|(id, name, _)| {
                let in_tab = self.entries.iter().filter(|entry| entry.major == *id);
                let total = in_tab.clone().count() as u32;
                let completed = in_tab.filter(|entry| entry.completed).count() as u32;
                CategoryProgress {
                    major: *id,
                    name,
                    completed,
                    total,
                }
            })
            .collect()
    }

    pub fn set_attendance(&mut self, start_time: u32, end_time: u32, count: u32, rewards: Vec<AttendanceReward>) {
        self.attendance_start = start_time;
        self.attendance_end = end_time;
        self.attendance_count = count;
        self.attendance_rewards = rewards;
        self.attendance_known = true;
        self.rebuild_lines();
    }

    /// `0x0AF0` result is the new day counter. It does not replace the
    /// calendar.
    pub fn apply_attendance_claim(&mut self, result: u32) {
        self.attendance_count = result;
        self.attendance_known = true;
        self.rebuild_lines();
    }

    /// `ZC_UI_OPEN` type 7. `item_name` names a reward item id.
    pub fn open_attendance(&mut self, days_attended: u32, claimed_today: bool, item_name: &dyn Fn(u32) -> String) {
        self.attendance_count = days_attended;
        self.attendance_claimed_today = claimed_today;
        self.attendance_known = true;
        self.rebuild_attendance_cells(item_name);
        self.rebuild_lines();
    }

    /// Server rewards first (`0x0AE3`), else the client's CheckAttendance
    /// table, as roBrowser's CheckAttendance window does.
    fn reward_days(&self) -> Vec<(u32, u32)> {
        if !self.attendance_rewards.is_empty() {
            return self
                .attendance_rewards
                .iter()
                .map(|reward| (reward.item_id, reward.amount))
                .collect();
        }
        self.tables
            .attendance
            .as_ref()
            .map(|table| table.rewards.iter().map(|reward| (reward.item_id, reward.amount)).collect())
            .unwrap_or_default()
    }

    /// Rebuild the calendar after a claim or new reward data.
    pub fn rebuild_attendance_cells(&mut self, item_name: &dyn Fn(u32) -> String) {
        let rewards = self.reward_days();
        let total = if rewards.is_empty() {
            DEFAULT_ATTENDANCE_DAYS.max(self.attendance_count as usize + 1)
        } else {
            rewards.len()
        };
        self.attendance_cells = (0..total)
            .map(|index| {
                let (label, item_id) = match rewards.get(index) {
                    Some((item_id, amount)) => (format!("Day {}\n{} x{}", index + 1, item_name(*item_id), amount), *item_id),
                    None => (format!("Day {}", index + 1), 0),
                };
                AttendanceCell {
                    label,
                    state: attendance_cell_state(index, self.attendance_count, self.attendance_claimed_today),
                    item_id,
                }
            })
            .collect();
        self.attendance_status = if self.attendance_claimed_today {
            format!(
                "{} days attended. Today's reward is claimed; come back tomorrow.",
                self.attendance_count
            )
        } else {
            format!("{} days attended. Today's reward is ready.", self.attendance_count)
        };
    }

    /// A successful claim moves today into the claimed days.
    pub fn mark_attendance_claimed(&mut self, item_name: &dyn Fn(u32) -> String) {
        self.attendance_claimed_today = true;
        self.rebuild_attendance_cells(item_name);
    }

    pub fn attendance_cells(&self) -> &[AttendanceCell] {
        &self.attendance_cells
    }

    /// Reward items whose icons the calendar or the selected achievement's
    /// detail panel still needs.
    pub fn missing_item_icons(&self) -> Vec<u32> {
        let mut missing: Vec<u32> = self
            .attendance_cells
            .iter()
            .map(|cell| cell.item_id)
            .chain(self.selected_reward_item())
            .filter(|item_id| *item_id != 0 && !self.item_icons.contains_key(item_id))
            .collect();
        missing.sort_unstable();
        missing.dedup();
        missing
    }

    pub fn note_item_texture(&mut self, item_id: u32, texture: Arc<Texture>) {
        if self.attendance_cells.iter().any(|cell| cell.item_id == item_id) || self.selected_reward_item() == Some(item_id) {
            self.item_icons.insert(item_id, texture);
        }
    }

    pub fn item_icon(&self, item_id: u32) -> Option<Arc<Texture>> {
        self.item_icons.get(&item_id).cloned()
    }

    /// The selected achievement when it can be claimed, else the first
    /// claimable one in the current view.
    pub fn claimable_id(&self) -> Option<u32> {
        if let Some(selected) = self.selected
            && self
                .entries
                .iter()
                .any(|entry| entry.achievement_id == selected && entry.completed && !entry.rewarded)
        {
            return Some(selected);
        }
        let filter = AchievementFilter::from_code(self.filter);
        self.entries
            .iter()
            .filter(|entry| self.category_matches(entry))
            .filter(|entry| filter.matches(entry))
            .find(|entry| entry.completed && !entry.rewarded)
            .map(|entry| entry.achievement_id)
    }

    fn category_matches(&self, entry: &TrackedAchievement) -> bool {
        if self.major == 0 {
            return true;
        }
        entry.major == self.major && self.minor.is_none_or(|minor| entry.minor == minor)
    }

    /// Keeps the client tables; everything from the session resets.
    pub fn clear(&mut self) {
        let tables = self.tables.clone();
        *self = Self::default();
        self.tables = tables;
        self.rebuild_lines();
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    fn current_category_label(&self) -> String {
        match self.minor.and_then(|minor| minor_name(self.major, minor)) {
            Some(minor) => format!("{} / {minor}", major_name(self.major)),
            None => major_name(self.major).to_owned(),
        }
    }

    fn rebuild_lines(&mut self) {
        let attendance = if self.attendance_known {
            format!(
                "Attendance {} days ({}-{}) rewards {}",
                self.attendance_count,
                self.attendance_start,
                self.attendance_end,
                self.attendance_rewards.len()
            )
        } else {
            "Attendance not loaded".to_owned()
        };
        self.filter_label = AchievementFilter::from_code(self.filter).label().to_owned();
        self.category_label = self.current_category_label();
        self.lines = vec![
            format!(
                "Score {}  rank {}  exp {}/{}  filter {}  category {}",
                self.total_score, self.level, self.experience, self.experience_to_next_level, self.filter_label, self.category_label
            ),
            attendance,
        ];
        let filter = AchievementFilter::from_code(self.filter);
        let selected = self.selected;
        self.rows = self
            .entries
            .iter()
            .filter(|entry| self.category_matches(entry))
            .filter(|entry| filter.matches(entry))
            .map(|entry| AchievementRow {
                achievement_id: entry.achievement_id,
                text: entry.summary.clone(),
                score: entry.score,
                objective_hits: entry.objective_hits,
                claimable: entry.completed && !entry.rewarded,
                rewarded: entry.rewarded,
                selected: selected == Some(entry.achievement_id),
            })
            .collect();
        if self.rows.is_empty() {
            self.lines.push("No achievements".to_owned());
        } else {
            self.lines.extend(self.rows.iter().map(|row| row.text.clone()));
        }
        self.detail = self.build_detail();
    }

    fn build_detail(&self) -> Vec<String> {
        let Some(id) = self.selected else {
            return vec!["Select an achievement".to_owned()];
        };
        let entry = self.entries.iter().find(|entry| entry.achievement_id == id);
        let mut lines = Vec::new();
        match self.info(id) {
            Some(info) => {
                lines.push(info.title.clone());
                let mut tab = major_name(info.major).to_owned();
                if let Some(minor) = minor_name(info.major, info.minor) {
                    tab = format!("{tab} / {minor}");
                }
                lines.push(format!("{tab} · {} points", info.score));
                if !info.details.is_empty() {
                    lines.push(info.details.clone());
                } else if !info.summary.is_empty() {
                    lines.push(info.summary.clone());
                }
                lines.extend(info.goals.iter().map(|goal| format!("• {goal}")));
                lines.push(reward_line(&info.reward, &self.reward_names));
            }
            None => lines.push(format!("Achievement #{id}")),
        }
        if let Some(entry) = entry {
            lines.push(format!(
                "Status: {} ({}/{OBJECTIVE_SLOTS})",
                achievement_state(entry.completed, entry.rewarded),
                entry.objective_hits
            ));
        }
        lines
    }

    fn set_rank(&mut self, total_score: u32, level: u16, experience: u32, experience_to_next_level: u32) {
        self.total_score = total_score;
        self.level = level;
        self.experience = experience;
        self.experience_to_next_level = experience_to_next_level;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ragnarok_packets::AchievementData;

    use super::{Achievements, AttendanceCellState, DEFAULT_ATTENDANCE_DAYS, attendance_cell_state};
    use crate::world::{AchievementCatalog, AchievementInfo, AchievementReward, AttendanceTable, AttendanceTableReward};

    fn sample(id: u32, completed: u8, rewarded: u8) -> AchievementData {
        AchievementData {
            acheivement_id: id,
            is_completed: completed,
            objectives: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            completion_timestamp: 1,
            got_rewarded: rewarded,
        }
    }

    fn catalog() -> Arc<AchievementCatalog> {
        let mut catalog = AchievementCatalog::new();
        catalog.insert(120001, AchievementInfo {
            title: "North Prontera Field Exploration(1)".into(),
            group: "ADVENTURE".into(),
            major: 2,
            minor: 0,
            summary: "Discover hidden treasure".into(),
            details: "Discover treasure in north Prontera field".into(),
            goals: vec!["Find the chest".into()],
            reward: AchievementReward {
                item_id: Some(22876),
                ..AchievementReward::default()
            },
            score: 10,
        });
        catalog.insert(200001, AchievementInfo {
            title: "Duelist".into(),
            major: 3,
            minor: 1,
            score: 25,
            ..AchievementInfo::default()
        });
        Arc::new(catalog)
    }

    #[test]
    fn list_update_and_reward() {
        let mut log = Achievements::default();
        log.replace_list(10, 1, 0, 50, vec![sample(2, 0, 0), sample(1, 1, 0)]);
        assert!(log.lines()[2].starts_with("#0001"));
        assert_eq!(log.claimable_id(), Some(1));
        log.upsert(11, 1, 5, 50, sample(2, 1, 0));
        assert!(log.lines()[3].contains("complete"));
        log.apply_reward(1, false);
        assert!(log.lines()[2].contains("rewarded"));
        assert_eq!(log.claimable_id(), Some(2));
        log.set_attendance(10, 20, 4, vec![ragnarok_packets::AttendanceReward { item_id: 501, amount: 1 }]);
        assert!(log.lines().iter().any(|line| line.contains("4 days") && line.contains("rewards 1")));
        log.apply_attendance_claim(5);
        assert!(log.lines().iter().any(|line| line.contains("5 days") && line.contains("rewards 1")));
        log.clear();
        assert!(log.lines().iter().any(|line| line.contains("not loaded")));
    }

    #[test]
    fn filter_hides_incomplete_and_rank_bar_uses_experience() {
        let mut log = Achievements::default();
        log.replace_list(10, 2, 25, 100, vec![sample(1, 1, 0), sample(2, 0, 0)]);
        assert!(log.lines()[0].contains("exp 25/100"));
        assert!((log.experience_fraction() - 0.25).abs() < f32::EPSILON);
        assert!(log.lines().iter().any(|line| line.contains("(1/10)")));
        log.cycle_filter();
        assert_eq!(log.claimable_id(), Some(1));
        assert_eq!(log.rows().len(), 1);
        log.cycle_filter();
        assert!(log.claimable_id().is_none());
        assert!(log.lines()[0].contains("Incomplete"));
    }

    #[test]
    fn catalog_titles_tabs_and_detail() {
        let mut log = Achievements::default();
        log.replace_list(1, 1, 0, 10, vec![
            sample(120001, 1, 0),
            sample(200001, 0, 0),
            sample(220001, 0, 0),
        ]);
        // Before the table: id buckets stand in.
        assert!(log.rows()[0].text.starts_with("#120001"));
        log.attach_tables(catalog(), Arc::new(AttendanceTable::default()));
        assert!(log.rows()[0].text.starts_with("North Prontera Field Exploration(1) · complete"));
        assert_eq!(log.rows()[0].score, 10);

        log.select_category(2, None);
        assert_eq!(log.rows().len(), 1);
        assert!(log.lines()[0].contains("category Adventure"));
        log.select_category(3, Some(1));
        assert_eq!(log.rows()[0].achievement_id, 200001);
        assert!(log.lines()[0].contains("category Battle / Training"));
        log.select_category(3, Some(0));
        assert!(log.rows().is_empty());
        // Unknown id 220001 falls back to General.
        log.select_category(1, None);
        assert_eq!(log.rows()[0].achievement_id, 220001);
        log.cycle_category();
        assert_eq!(log.major(), 2);

        log.select_category(0, None);
        log.select_achievement(120001);
        assert!(log.rows().iter().any(|row| row.selected && row.achievement_id == 120001));
        let detail = log.detail();
        assert_eq!(detail[0], "North Prontera Field Exploration(1)");
        assert!(detail.iter().any(|line| line == "Adventure / Rune Midgarts · 10 points"));
        assert!(detail.iter().any(|line| line == "• Find the chest"));
        assert!(detail.iter().any(|line| line == "Reward: item 22876"));
        assert_eq!(log.claimable_id(), Some(120001));

        let progress = log.category_progress();
        let adventure = progress.iter().find(|tab| tab.major == 2).unwrap();
        assert_eq!((adventure.completed, adventure.total), (1, 1));

        log.clear();
        log.replace_list(1, 1, 0, 10, vec![sample(200001, 0, 0)]);
        assert!(log.rows()[0].text.starts_with("Duelist"), "tables survive clear");
    }

    #[test]
    fn attendance_cells_mark_claimed_today_and_upcoming() {
        assert_eq!(attendance_cell_state(0, 3, false), AttendanceCellState::Claimed);
        assert_eq!(attendance_cell_state(2, 3, false), AttendanceCellState::Claimed);
        assert_eq!(attendance_cell_state(3, 3, false), AttendanceCellState::Today);
        assert_eq!(attendance_cell_state(4, 3, false), AttendanceCellState::Upcoming);
        // Claimed today: the count already includes today, nothing is "today".
        assert_eq!(attendance_cell_state(3, 3, true), AttendanceCellState::Upcoming);
        assert_eq!(attendance_cell_state(2, 3, true), AttendanceCellState::Claimed);
    }

    #[test]
    fn attendance_calendar_uses_rewards_when_known() {
        let name = |id: u32| format!("Item {id}");
        let mut log = Achievements::default();
        log.open_attendance(2, false, &name);
        assert_eq!(log.attendance_cells().len(), DEFAULT_ATTENDANCE_DAYS);
        assert_eq!(log.attendance_cells()[0].label, "Day 1");
        assert_eq!(log.attendance_cells()[2].state, AttendanceCellState::Today);
        assert!(log.attendance_status.as_str().contains("ready"));

        log.set_attendance(1, 2, 2, vec![
            ragnarok_packets::AttendanceReward { item_id: 501, amount: 3 },
            ragnarok_packets::AttendanceReward { item_id: 502, amount: 1 },
            ragnarok_packets::AttendanceReward { item_id: 503, amount: 1 },
        ]);
        log.rebuild_attendance_cells(&name);
        assert_eq!(log.attendance_cells().len(), 3);
        assert_eq!(log.attendance_cells()[0].label, "Day 1\nItem 501 x3");
        log.mark_attendance_claimed(&name);
        assert_eq!(log.attendance_cells()[2].state, AttendanceCellState::Upcoming);
        assert!(log.attendance_status.as_str().contains("claimed"));
    }

    #[test]
    fn attendance_calendar_falls_back_to_client_table() {
        let name = |id: u32| format!("Item {id}");
        let mut log = Achievements::default();
        let table = AttendanceTable {
            start_date: 20261021,
            end_date: 20991231,
            rewards: (1..=20)
                .map(|day| AttendanceTableReward {
                    day,
                    item_id: 600 + day,
                    amount: 1,
                })
                .collect(),
        };
        log.attach_tables(catalog(), Arc::new(table));
        log.open_attendance(0, false, &name);
        assert_eq!(log.attendance_cells().len(), 20);
        assert_eq!(log.attendance_cells()[0].label, "Day 1\nItem 601 x1");
        assert_eq!(log.attendance_cells()[0].item_id, 601);
        assert_eq!(log.attendance_cells()[0].state, AttendanceCellState::Today);
        assert_eq!(log.missing_item_icons().len(), 20);
    }

    #[test]
    fn reward_line_names_items_titles_and_buffs_or_falls_back_to_ids() {
        use super::{RewardNames, reward_line};

        let mut catalog = AchievementCatalog::new();
        catalog.insert(1, AchievementInfo {
            reward: AchievementReward {
                item_id: Some(501),
                title_id: Some(1000),
                buff_id: Some(34),
            },
            ..AchievementInfo::default()
        });
        catalog.insert(2, AchievementInfo {
            reward: AchievementReward {
                item_id: Some(9999),
                ..AchievementReward::default()
            },
            ..AchievementInfo::default()
        });
        let names = RewardNames::resolve(
            &catalog,
            |id| match id {
                501 => Some("Red Potion".to_owned()),
                9999 => Some("  ".to_owned()),
                _ => None,
            },
            |id| (id == 1000).then(|| "Novice Adventurer".to_owned()),
            |id| (id == 34).then(|| "Blessing".to_owned()),
        );
        assert_eq!(
            reward_line(&catalog[&1].reward, &names),
            "Reward: Red Potion (item), title \"Novice Adventurer\", buff Blessing"
        );
        // A blank name counts as missing.
        assert_eq!(reward_line(&catalog[&2].reward, &names), "Reward: item 9999");
        assert_eq!(reward_line(&AchievementReward::default(), &names), "Reward: none");
    }

    #[test]
    fn selected_reward_item_asks_for_its_icon_once() {
        let mut catalog = AchievementCatalog::new();
        catalog.insert(7, AchievementInfo {
            title: "Seven".into(),
            reward: AchievementReward {
                item_id: Some(22876),
                ..AchievementReward::default()
            },
            ..AchievementInfo::default()
        });
        let mut log = Achievements::default();
        log.attach_tables(Arc::new(catalog), Arc::new(AttendanceTable::default()));
        assert!(log.missing_item_icons().is_empty());
        log.select_achievement(7);
        assert_eq!(log.selected_reward_item(), Some(22876));
        assert_eq!(log.missing_item_icons(), vec![22876]);
        assert_eq!(log.detail().iter().filter(|line| line.starts_with("Reward:")).count(), 1);
    }
}
