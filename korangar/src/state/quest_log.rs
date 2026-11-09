use korangar_interface::element::StateElement;
use ragnarok_packets::{HuntingObjective, ObjectiveDetails1, Quest};
use rust_state::RustState;

/// One tracked quest for the journal UI.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct TrackedQuest {
    pub quest_id: u32,
    pub active: bool,
    pub start_time: u32,
    pub expire_time: u32,
    pub objectives: Vec<QuestObjective>,
    pub summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct QuestObjective {
    pub mob_id: u32,
    pub mob_name: String,
    pub current_count: u16,
    pub total_count: u16,
    pub summary: String,
}

fn quest_summary(quest_id: u32, active: bool) -> String {
    if active {
        format!("Quest #{quest_id}")
    } else {
        format!("Quest #{quest_id} (inactive)")
    }
}

fn objective_summary(mob_name: &str, mob_id: u32, current: u16, total: u16) -> String {
    let name = if mob_name.is_empty() {
        format!("Mob #{mob_id}")
    } else {
        mob_name.to_string()
    };
    format!("{name}: {current}/{total}")
}

/// Quest journal state driven by the typed quest network events.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct QuestLog {
    quests: Vec<TrackedQuest>,
}

impl QuestLog {
    pub fn replace_all(&mut self, quests: Vec<Quest>) {
        self.quests = quests
            .into_iter()
            .map(|quest| {
                let active = quest.active != 0;
                let objectives = quest
                    .objective_details
                    .into_iter()
                    .map(|objective| {
                        let summary = objective_summary(
                            &objective.mob_name,
                            objective.mob_id,
                            objective.kill_count,
                            objective.total_count,
                        );
                        QuestObjective {
                            mob_id: objective.mob_id,
                            mob_name: objective.mob_name,
                            current_count: objective.kill_count,
                            total_count: objective.total_count,
                            summary,
                        }
                    })
                    .collect();
                TrackedQuest {
                    summary: quest_summary(quest.quest_id, active),
                    quest_id: quest.quest_id,
                    active,
                    start_time: 0,
                    expire_time: quest.expire_time,
                    objectives,
                }
            })
            .collect();
        self.quests.sort_by_key(|quest| quest.quest_id);
    }

    pub fn upsert_notification(
        &mut self,
        quest_id: u32,
        active: bool,
        start_time: u32,
        expire_time: u32,
        objectives: Vec<ObjectiveDetails1>,
    ) {
        let mapped: Vec<_> = objectives
            .into_iter()
            .map(|objective| {
                let summary = objective_summary(&objective.mob_name, objective.mob_id, 0, objective.mob_count);
                QuestObjective {
                    mob_id: objective.mob_id,
                    mob_name: objective.mob_name,
                    current_count: 0,
                    total_count: objective.mob_count,
                    summary,
                }
            })
            .collect();
        if let Some(existing) = self.quests.iter_mut().find(|quest| quest.quest_id == quest_id) {
            existing.active = active;
            existing.start_time = start_time;
            existing.expire_time = expire_time;
            existing.objectives = mapped;
            existing.summary = quest_summary(quest_id, active);
        } else {
            self.quests.push(TrackedQuest {
                quest_id,
                active,
                start_time,
                expire_time,
                objectives: mapped,
                summary: quest_summary(quest_id, active),
            });
            self.quests.sort_by_key(|quest| quest.quest_id);
        }
    }

    pub fn apply_hunting_objectives(&mut self, objectives: &[HuntingObjective]) {
        for objective in objectives {
            if let Some(quest) = self.quests.iter_mut().find(|quest| quest.quest_id == objective.quest_id) {
                if let Some(slot) = quest.objectives.iter_mut().find(|slot| slot.mob_id == objective.mob_id) {
                    slot.current_count = objective.current_count;
                    slot.total_count = objective.total_count;
                    slot.summary = objective_summary(&slot.mob_name, slot.mob_id, slot.current_count, slot.total_count);
                } else {
                    let summary = objective_summary("", objective.mob_id, objective.current_count, objective.total_count);
                    quest.objectives.push(QuestObjective {
                        mob_id: objective.mob_id,
                        mob_name: String::new(),
                        current_count: objective.current_count,
                        total_count: objective.total_count,
                        summary,
                    });
                }
            }
        }
    }

    pub fn remove(&mut self, quest_id: u32) {
        self.quests.retain(|quest| quest.quest_id != quest_id);
    }

    pub fn clear(&mut self) {
        self.quests.clear();
    }

    pub fn quests(&self) -> &[TrackedQuest] {
        &self.quests
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::HuntingObjective;

    use super::QuestLog;

    #[test]
    fn hunting_update_refreshes_counts() {
        let mut log = QuestLog::default();
        log.upsert_notification(9, true, 0, 0, Vec::new());
        log.apply_hunting_objectives(&[HuntingObjective {
            quest_id: 9,
            mob_id: 1002,
            total_count: 10,
            current_count: 3,
        }]);
        assert_eq!(log.quests().len(), 1);
        assert_eq!(log.quests()[0].objectives[0].current_count, 3);
        assert!(log.quests()[0].objectives[0].summary.contains("3/10"));
        log.remove(9);
        assert!(log.quests().is_empty());
    }
}
