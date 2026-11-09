use korangar_interface::element::StateElement;
use ragnarok_packets::{AchievementData, AttendanceReward};
use rust_state::RustState;

/// One achievement row for the list window.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct TrackedAchievement {
    pub achievement_id: u32,
    pub completed: bool,
    pub rewarded: bool,
    pub summary: String,
}

fn achievement_summary(entry: &AchievementData) -> String {
    let progress = entry.objectives.iter().filter(|value| **value > 0).count();
    let state = if entry.got_rewarded != 0 {
        "rewarded"
    } else if entry.is_completed != 0 {
        "complete"
    } else {
        "in progress"
    };
    format!("#{:04} {state} ({progress} objectives)", entry.acheivement_id)
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
            entry.summary = format!("#{achievement_id:04} rewarded");
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
        self.entries
            .iter()
            .find(|entry| entry.completed && !entry.rewarded)
            .map(|entry| entry.achievement_id)
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
        self.lines = vec![
            format!(
                "Score {}  rank {}  exp {}/{}",
                self.total_score, self.level, self.experience, self.experience_to_next_level
            ),
            attendance,
        ];
        if self.entries.is_empty() {
            self.lines.push("No achievements".to_owned());
        } else {
            self.lines.extend(self.entries.iter().map(|entry| entry.summary.clone()));
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
}
