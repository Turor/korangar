use korangar_interface::element::StateElement;
use ragnarok_packets::{AchievementData, AttendanceReward};
use rust_state::RustState;

/// One achievement row for the list window.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct TrackedAchievement {
    pub achievement_id: u32,
    pub completed: bool,
    pub rewarded: bool,
    pub objective_hits: u8,
    pub summary: String,
}

/// Objective slots on `ZC_ACHIEVEMENT_LIST`. The bar uses how many are above 0.
pub const OBJECTIVE_SLOTS: u8 = 10;

/// Rough RE achievement id buckets (`id / 10000`). Official major/minor tabs
/// need the client achievement table; until that ships these buckets stand in.
fn category_bucket(achievement_id: u32) -> u32 {
    achievement_id / 10_000
}

fn category_label(bucket: u32) -> String {
    match bucket {
        0 => "All".to_owned(),
        11 => "Quests".to_owned(),
        12 => "Adventure".to_owned(),
        13 => "Special".to_owned(),
        17 => "Events".to_owned(),
        19 => "Memory".to_owned(),
        20 => "Battle".to_owned(),
        22 => "Gathering".to_owned(),
        23 => "Community".to_owned(),
        24 => "Crafting".to_owned(),
        other => format!("{other}xxxx"),
    }
}

fn achievement_summary(entry: &AchievementData) -> String {
    let progress = objective_hits(entry);
    let state = achievement_state(entry.is_completed != 0, entry.got_rewarded != 0);
    format!(
        "#{id:04} {state} ({progress}/{slots})",
        id = entry.acheivement_id,
        slots = OBJECTIVE_SLOTS
    )
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
    lines: Vec<String>,
    filter_label: String,
    category_label: String,
    #[hidden_element]
    filter: u8,
    /// `0` = all categories. Otherwise `id / 10000` for the active bucket.
    #[hidden_element]
    category: u32,
}

impl Achievements {
    pub fn replace_list(
        &mut self,
        total_score: u32,
        level: u16,
        experience: u32,
        experience_to_next_level: u32,
        entries: Vec<AchievementData>,
    ) {
        self.set_rank(total_score, level, experience, experience_to_next_level);
        self.entries = entries.into_iter().map(tracked_from).collect();
        self.entries.sort_by_key(|entry| entry.achievement_id);
        self.rebuild_lines();
    }

    pub fn upsert(&mut self, total_score: u32, level: u16, experience: u32, experience_to_next_level: u32, entry: AchievementData) {
        self.set_rank(total_score, level, experience, experience_to_next_level);
        let tracked = tracked_from(entry);
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
            entry.summary = format!(
                "#{achievement_id:04} rewarded ({}/{})",
                entry.objective_hits, OBJECTIVE_SLOTS
            );
        }
        self.rebuild_lines();
    }

    pub fn cycle_filter(&mut self) {
        let next = AchievementFilter::from_code(self.filter).cycle();
        self.filter = next as u8;
        self.rebuild_lines();
    }

    pub fn cycle_category(&mut self) {
        let mut buckets: Vec<u32> = self
            .entries
            .iter()
            .map(|entry| category_bucket(entry.achievement_id))
            .collect();
        buckets.sort_unstable();
        buckets.dedup();
        if buckets.is_empty() {
            self.category = 0;
            self.rebuild_lines();
            return;
        }
        if self.category == 0 {
            self.category = buckets[0];
        } else if let Some(index) = buckets.iter().position(|bucket| *bucket == self.category) {
            if index + 1 < buckets.len() {
                self.category = buckets[index + 1];
            } else {
                self.category = 0;
            }
        } else {
            self.category = 0;
        }
        self.rebuild_lines();
    }

    pub fn set_attendance(&mut self, start_time: u32, end_time: u32, count: u32, rewards: Vec<AttendanceReward>) {
        self.attendance_start = start_time;
        self.attendance_end = end_time;
        self.attendance_count = count;
        self.attendance_rewards = rewards;
        self.attendance_known = true;
        self.rebuild_lines();
    }

    /// `0x0AF0` result is the new day counter. It does not replace the calendar.
    pub fn apply_attendance_claim(&mut self, result: u32) {
        self.attendance_count = result;
        self.attendance_known = true;
        self.rebuild_lines();
    }

    pub fn claimable_id(&self) -> Option<u32> {
        let filter = AchievementFilter::from_code(self.filter);
        self.entries
            .iter()
            .filter(|entry| self.category_matches(entry))
            .filter(|entry| filter.matches(entry))
            .find(|entry| entry.completed && !entry.rewarded)
            .map(|entry| entry.achievement_id)
    }

    fn category_matches(&self, entry: &TrackedAchievement) -> bool {
        self.category == 0 || category_bucket(entry.achievement_id) == self.category
    }

    pub fn clear(&mut self) {
        *self = Self::default();
        self.rebuild_lines();
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
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
        self.category_label = category_label(self.category);
        self.lines = vec![
            format!(
                "Score {}  rank {}  exp {}/{}  filter {}  category {}",
                self.total_score,
                self.level,
                self.experience,
                self.experience_to_next_level,
                self.filter_label,
                self.category_label
            ),
            attendance,
        ];
        let filter = AchievementFilter::from_code(self.filter);
        let shown: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| self.category_matches(entry))
            .filter(|entry| filter.matches(entry))
            .map(|entry| entry.summary.clone())
            .collect();
        if shown.is_empty() {
            self.lines.push("No achievements".to_owned());
        } else {
            self.lines.extend(shown);
        }
    }

    fn set_rank(&mut self, total_score: u32, level: u16, experience: u32, experience_to_next_level: u32) {
        self.total_score = total_score;
        self.level = level;
        self.experience = experience;
        self.experience_to_next_level = experience_to_next_level;
    }
}

fn tracked_from(entry: AchievementData) -> TrackedAchievement {
    TrackedAchievement {
        summary: achievement_summary(&entry),
        achievement_id: entry.acheivement_id,
        completed: entry.is_completed != 0,
        rewarded: entry.got_rewarded != 0,
        objective_hits: objective_hits(&entry),
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::AchievementData;

    use super::Achievements;

    fn sample(id: u32, completed: u8, rewarded: u8) -> AchievementData {
        AchievementData {
            acheivement_id: id,
            is_completed: completed,
            objectives: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            completion_timestamp: 1,
            got_rewarded: rewarded,
        }
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
        log.set_attendance(
            10,
            20,
            4,
            vec![ragnarok_packets::AttendanceReward { item_id: 501, amount: 1 }],
        );
        assert!(log.lines().iter().any(|line| line.contains("4 days") && line.contains("rewards 1")));
        log.apply_attendance_claim(5);
        assert!(log.lines().iter().any(|line| line.contains("5 days") && line.contains("rewards 1")));
        log.clear();
        assert!(log.lines().is_empty() || log.lines().iter().any(|line| line.contains("not loaded")));
    }

    #[test]
    fn filter_hides_incomplete_and_rank_bar_uses_experience() {
        let mut log = Achievements::default();
        log.replace_list(10, 2, 25, 100, vec![sample(1, 1, 0), sample(2, 0, 0)]);
        assert!(log.lines()[0].contains("exp 25/100"));
        assert!(log.lines().iter().any(|line| line.contains("(1/10)")));
        log.cycle_filter();
        assert_eq!(log.claimable_id(), Some(1));
        assert!(log.lines().iter().any(|line| line.starts_with("#0001")));
        assert!(!log.lines().iter().any(|line| line.starts_with("#0002")));
        log.cycle_filter();
        assert!(log.claimable_id().is_none());
        assert!(log.lines()[0].contains("Incomplete"));
    }

    #[test]
    fn category_cycles_id_buckets() {
        let mut log = Achievements::default();
        log.replace_list(
            1,
            1,
            0,
            10,
            vec![sample(120001, 1, 0), sample(200001, 0, 0), sample(120002, 0, 0)],
        );
        assert!(log.lines()[0].contains("category All"));
        log.cycle_category();
        assert!(log.lines()[0].contains("category Adventure"));
        assert!(log.lines().iter().any(|line| line.starts_with("#120001")));
        assert!(log.lines().iter().any(|line| line.starts_with("#120002")));
        assert!(!log.lines().iter().any(|line| line.starts_with("#200001")));
        log.cycle_category();
        assert!(log.lines()[0].contains("category Battle"));
        assert!(log.lines().iter().any(|line| line.starts_with("#200001")));
        assert!(!log.lines().iter().any(|line| line.starts_with("#120001")));
        log.cycle_category();
        assert!(log.lines()[0].contains("category All"));
    }
}
