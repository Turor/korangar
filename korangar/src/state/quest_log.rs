use korangar_interface::element::StateElement;
use ragnarok_packets::{HuntingObjective, ObjectiveDetails1, Quest, QuestDetails};
use rust_state::RustState;

use crate::world::{QuestTextSource, format_quest_rewards, format_quest_timer, quest_is_cooldown};

/// One tracked quest for the journal UI.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct TrackedQuest {
    pub quest_id: u32,
    pub active: bool,
    pub start_time: u32,
    pub expire_time: u32,
    pub objectives: Vec<QuestObjective>,
    pub title: String,
    pub summary: String,
    pub description: String,
    pub rewards: String,
}

#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct QuestObjective {
    pub mob_id: u32,
    pub mob_name: String,
    pub current_count: u16,
    pub total_count: u16,
    pub summary: String,
}

/// Renewal journal tabs. Cooldown wins over the active flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub enum QuestFilter {
    #[default]
    Active,
    Inactive,
    Cooldown,
}

impl QuestFilter {
    fn matches(self, quest: &TrackedQuest, now_unix: u32) -> bool {
        let cooldown = quest_is_cooldown(quest.expire_time, now_unix);
        match self {
            Self::Cooldown => cooldown,
            Self::Active => quest.active && !cooldown,
            Self::Inactive => !quest.active && !cooldown,
        }
    }

    fn empty_message(self) -> &'static str {
        match self {
            Self::Active => "No active quests",
            Self::Inactive => "No inactive quests",
            Self::Cooldown => "No quests on cooldown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalLineKind {
    Title,
    Body,
    Toggle,
    Empty,
}

/// One row of the quest journal. Built at display time from
/// [`QuestLog::journal_lines`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalLine {
    pub text: String,
    pub kind: JournalLineKind,
    pub quest_id: u32,
    /// Value to send with `CZ_ACTIVE_QUEST` when this row is the toggle.
    pub make_active: bool,
    pub selected: bool,
}

fn monster_label(packet_name: &str, mob_id: u32, text: &impl QuestTextSource) -> String {
    let trimmed = packet_name.trim_matches(|character: char| character == '\0' || character.is_whitespace());
    if !trimmed.is_empty() {
        return trimmed.to_owned();
    }
    text.monster_name(mob_id)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Monster {mob_id}"))
}

fn objective_summary(name: &str, current: u16, total: u16) -> String {
    format!("{name}: {current}/{total}")
}

fn tracked_quest(
    quest_id: u32,
    active: bool,
    start_time: u32,
    expire_time: u32,
    objectives: Vec<QuestObjective>,
    text: &impl QuestTextSource,
) -> TrackedQuest {
    let display = text.quest_display(quest_id);
    let title = display
        .map(|row| row.title.clone())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| format!("Quest {quest_id}"));
    let summary = display.map(|row| row.summary.clone()).unwrap_or_default();
    let description = display.map(|row| row.description.join("\n")).unwrap_or_default();
    let rewards = display
        .map(|row| format_quest_rewards(row, |item_id| text.item_label(item_id)))
        .unwrap_or_default();
    TrackedQuest {
        quest_id,
        active,
        start_time,
        expire_time,
        objectives,
        title,
        summary,
        description,
        rewards,
    }
}

fn objectives_from_details(details: impl IntoIterator<Item = QuestDetails>, text: &impl QuestTextSource) -> Vec<QuestObjective> {
    details
        .into_iter()
        .map(|objective| {
            let mob_name = monster_label(&objective.mob_name, objective.mob_id, text);
            let summary = objective_summary(&mob_name, objective.kill_count, objective.total_count);
            QuestObjective {
                mob_id: objective.mob_id,
                mob_name,
                current_count: objective.kill_count,
                total_count: objective.total_count,
                summary,
            }
        })
        .collect()
}

fn objectives_from_notification(details: Vec<ObjectiveDetails1>, text: &impl QuestTextSource) -> Vec<QuestObjective> {
    details
        .into_iter()
        .map(|objective| {
            let mob_name = monster_label(&objective.mob_name, objective.mob_id, text);
            let summary = objective_summary(&mob_name, 0, objective.mob_count);
            QuestObjective {
                mob_id: objective.mob_id,
                mob_name,
                current_count: 0,
                total_count: objective.mob_count,
                summary,
            }
        })
        .collect()
}

/// Quest journal state driven by the typed quest network events.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct QuestLog {
    quests: Vec<TrackedQuest>,
    filter: QuestFilter,
    selected_quest: Option<u32>,
}

impl QuestLog {
    pub fn replace_all(&mut self, quests: Vec<Quest>, text: &impl QuestTextSource) {
        let selected = self.selected_quest;
        let filter = self.filter;
        self.quests = quests
            .into_iter()
            .map(|quest| {
                let objectives = objectives_from_details(quest.objective_details, text);
                tracked_quest(quest.quest_id, quest.active != 0, 0, quest.expire_time, objectives, text)
            })
            .collect();
        self.quests.sort_by_key(|quest| quest.quest_id);
        self.filter = filter;
        self.selected_quest = selected.filter(|quest_id| self.quests.iter().any(|quest| quest.quest_id == *quest_id));
    }

    pub fn upsert_notification(
        &mut self,
        quest_id: u32,
        active: bool,
        start_time: u32,
        expire_time: u32,
        objectives: Vec<ObjectiveDetails1>,
        text: &impl QuestTextSource,
    ) {
        let mapped = objectives_from_notification(objectives, text);
        let tracked = tracked_quest(quest_id, active, start_time, expire_time, mapped, text);
        if let Some(existing) = self.quests.iter_mut().find(|quest| quest.quest_id == quest_id) {
            *existing = tracked;
        } else {
            self.quests.push(tracked);
            self.quests.sort_by_key(|quest| quest.quest_id);
        }
    }

    pub fn apply_hunting_objectives(&mut self, objectives: &[HuntingObjective], text: &impl QuestTextSource) {
        for objective in objectives {
            let Some(quest) = self.quests.iter_mut().find(|quest| quest.quest_id == objective.quest_id) else {
                continue;
            };
            if let Some(slot) = quest.objectives.iter_mut().find(|slot| slot.mob_id == objective.mob_id) {
                slot.current_count = objective.current_count;
                slot.total_count = objective.total_count;
                slot.summary = objective_summary(&slot.mob_name, slot.current_count, slot.total_count);
            } else {
                let mob_name = monster_label("", objective.mob_id, text);
                let summary = objective_summary(&mob_name, objective.current_count, objective.total_count);
                quest.objectives.push(QuestObjective {
                    mob_id: objective.mob_id,
                    mob_name,
                    current_count: objective.current_count,
                    total_count: objective.total_count,
                    summary,
                });
            }
        }
    }

    pub fn remove(&mut self, quest_id: u32) {
        self.quests.retain(|quest| quest.quest_id != quest_id);
        if self.selected_quest == Some(quest_id) {
            self.selected_quest = None;
        }
    }

    pub fn clear(&mut self) {
        self.quests.clear();
        self.selected_quest = None;
    }

    pub fn set_active_local(&mut self, quest_id: u32, active: bool) {
        if let Some(quest) = self.quests.iter_mut().find(|quest| quest.quest_id == quest_id) {
            quest.active = active;
        }
    }

    pub fn toggle_selected(&mut self, quest_id: u32) {
        self.selected_quest = if self.selected_quest == Some(quest_id) {
            None
        } else {
            Some(quest_id)
        };
    }

    #[allow(dead_code)]
    pub fn quests(&self) -> &[TrackedQuest] {
        &self.quests
    }

    pub fn journal_lines(&self, now_unix: u32) -> Vec<JournalLine> {
        let visible: Vec<&TrackedQuest> = self.quests.iter().filter(|quest| self.filter.matches(quest, now_unix)).collect();
        if visible.is_empty() {
            return vec![JournalLine {
                text: self.filter.empty_message().to_owned(),
                kind: JournalLineKind::Empty,
                quest_id: 0,
                make_active: false,
                selected: false,
            }];
        }

        let mut lines = Vec::new();
        for quest in visible {
            let selected = self.selected_quest == Some(quest.quest_id);
            lines.push(JournalLine {
                text: quest.title.clone(),
                kind: JournalLineKind::Title,
                quest_id: quest.quest_id,
                make_active: false,
                selected,
            });
            if !quest.summary.is_empty() && quest.summary != quest.title {
                lines.push(body_line(quest.quest_id, quest.summary.clone(), selected));
            }
            for objective in &quest.objectives {
                lines.push(body_line(quest.quest_id, objective.summary.clone(), selected));
            }
            let timer = format_quest_timer(quest.expire_time, now_unix);
            if !timer.is_empty() {
                lines.push(body_line(quest.quest_id, timer, selected));
            }
            lines.push(JournalLine {
                text: if quest.active { "Mark inactive" } else { "Mark active" }.to_owned(),
                kind: JournalLineKind::Toggle,
                quest_id: quest.quest_id,
                make_active: !quest.active,
                selected,
            });
            if selected {
                for line in quest.description.lines().filter(|line| !line.is_empty()) {
                    lines.push(body_line(quest.quest_id, line.to_owned(), true));
                }
                for line in quest.rewards.lines().filter(|line| !line.is_empty()) {
                    lines.push(body_line(quest.quest_id, line.to_owned(), true));
                }
            }
        }
        lines
    }
}

fn body_line(quest_id: u32, text: String, selected: bool) -> JournalLine {
    JournalLine {
        text,
        kind: JournalLineKind::Body,
        quest_id,
        make_active: false,
        selected,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ragnarok_bytes::ByteWriter;
    use ragnarok_packets::{HuntingObjective, ObjectiveDetails1, PacketExt, Quest, QuestDetails, SetQuestActivePacket};

    use super::{QuestFilter, QuestLog};
    use crate::world::{QuestDisplay, QuestRewardItem, QuestTextSource};

    struct Stub {
        displays: HashMap<u32, QuestDisplay>,
    }

    impl QuestTextSource for Stub {
        fn quest_display(&self, quest_id: u32) -> Option<&QuestDisplay> {
            self.displays.get(&quest_id)
        }

        fn monster_name(&self, mob_id: u32) -> Option<&str> {
            if mob_id == 1002 { Some("Poring") } else { None }
        }

        fn item_label(&self, item_id: u32) -> String {
            format!("Item {item_id}")
        }
    }

    fn empty_stub() -> Stub {
        Stub { displays: HashMap::new() }
    }

    #[test]
    fn hunting_update_refreshes_counts() {
        let stub = empty_stub();
        let mut log = QuestLog::default();
        log.upsert_notification(9, true, 0, 0, Vec::new(), &stub);
        log.apply_hunting_objectives(
            &[HuntingObjective {
                quest_id: 9,
                mob_id: 1002,
                total_count: 10,
                current_count: 3,
            }],
            &stub,
        );
        assert_eq!(log.quests().len(), 1);
        assert_eq!(log.quests()[0].objectives[0].current_count, 3);
        assert!(log.quests()[0].objectives[0].summary.contains("3/10"));
        assert!(log.quests()[0].objectives[0].summary.contains("Poring"));
        assert!(!log.quests()[0].objectives[0].summary.contains("Mob #"));
        assert_eq!(log.quests()[0].title, "Quest 9");
        assert!(!log.quests()[0].title.contains('#'));
        log.remove(9);
        assert!(log.quests().is_empty());
    }

    #[test]
    fn display_text_rewards_and_filters() {
        let mut displays = HashMap::new();
        displays.insert(9, QuestDisplay {
            title: "Poring Hunt".to_owned(),
            summary: "Thin the porings".to_owned(),
            description: vec!["Talk to the guide".to_owned()],
            reward_exp: 40,
            reward_job_exp: 0,
            reward_items: vec![QuestRewardItem { item_id: 909, count: 2 }],
        });
        let stub = Stub { displays };
        let mut log = QuestLog::default();
        log.replace_all(
            vec![Quest {
                quest_id: 9,
                active: 1,
                remaining_time: 0,
                expire_time: 0,
                objective_count: 1,
                objective_details: vec![QuestDetails {
                    hunt_identification: 1,
                    objective_type: 1,
                    mob_id: 1113,
                    minimum_level: 1,
                    maximum_level: 1,
                    kill_count: 0,
                    total_count: 1,
                    mob_name: "\0".to_owned(),
                }],
            }],
            &stub,
        );
        assert_eq!(log.quests()[0].title, "Poring Hunt");
        assert_eq!(log.quests()[0].objectives[0].mob_name, "Monster 1113");
        log.toggle_selected(9);
        let lines = log.journal_lines(1_700_000_000);
        let text = lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Poring Hunt"));
        assert!(text.contains("Thin the porings"));
        assert!(text.contains("Talk to the guide"));
        assert!(text.contains("Base EXP: 40"));
        assert!(text.contains("Item 909 x2"));
        assert!(text.contains("Mark inactive"));
        assert!(!text.contains("Quest #"));
        assert!(!text.contains("Mob #"));

        log.set_active_local(9, false);
        assert!(log.journal_lines(1_700_000_000).iter().any(|line| line.text == "No active quests"));
        // Inactive tab still lists it.
        log.filter = QuestFilter::Inactive;
        assert!(log.journal_lines(1_700_000_000).iter().any(|line| line.text == "Poring Hunt"));
    }

    #[test]
    fn set_quest_active_packet_is_seven_bytes() {
        let packet = SetQuestActivePacket {
            quest_id: 0x0102_0304,
            active: 1,
        };
        let mut writer = ByteWriter::new();
        packet.packet_to_bytes(&mut writer).unwrap();
        assert_eq!(writer.as_slice(), &[0xB6, 0x02, 0x04, 0x03, 0x02, 0x01, 0x01]);
    }

    #[test]
    fn cooldown_tab_uses_unix_deadline() {
        let stub = empty_stub();
        let mut log = QuestLog::default();
        log.upsert_notification(
            4,
            true,
            0,
            1_800_000_000,
            vec![ObjectiveDetails1 {
                hunt_identification: 0,
                objective_type: 0,
                mob_id: 1,
                minimum_level: 0,
                maximum_level: 0,
                mob_count: 1,
                mob_name: String::new(),
            }],
            &stub,
        );
        assert!(log.journal_lines(1_700_000_000).iter().any(|line| line.text == "No active quests"));
        log.filter = QuestFilter::Cooldown;
        let lines = log.journal_lines(1_700_000_000);
        assert!(lines.iter().any(|line| line.text.contains("Time left:")));
    }
}
