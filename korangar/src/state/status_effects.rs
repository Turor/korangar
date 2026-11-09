use korangar_interface::element::StateElement;
use ragnarok_packets::ClientTick;
use rust_state::RustState;

/// One timed (or flag) status currently on the local player.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct ActiveStatusEffect {
    pub status_index: u16,
    pub remaining_milliseconds: u32,
    pub total_duration_milliseconds: u32,
    /// Client tick when the status was last refreshed.
    pub updated_at_tick: u32,
    /// Preformatted chip label (`SC#12 5s`) so the window can borrow a stable
    /// `&str`.
    pub label: String,
}

/// Local-player status effects driven by `NetworkEvent::StatusChange*`.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct StatusEffects {
    effects: Vec<ActiveStatusEffect>,
}

fn format_label(status_index: u16, remaining_milliseconds: u32) -> String {
    if remaining_milliseconds == 0 {
        format!("SC#{status_index}")
    } else {
        let seconds = remaining_milliseconds.div_ceil(1000).max(1);
        format!("SC#{status_index} {seconds}s")
    }
}

impl StatusEffects {
    pub fn apply(&mut self, status_index: u16, remaining_milliseconds: u32, total_duration_milliseconds: u32, updated_at: ClientTick) {
        let label = format_label(status_index, remaining_milliseconds);
        if let Some(existing) = self.effects.iter_mut().find(|effect| effect.status_index == status_index) {
            existing.remaining_milliseconds = remaining_milliseconds;
            existing.total_duration_milliseconds = total_duration_milliseconds;
            existing.updated_at_tick = updated_at.0;
            existing.label = label;
            return;
        }
        self.effects.push(ActiveStatusEffect {
            status_index,
            remaining_milliseconds,
            total_duration_milliseconds,
            updated_at_tick: updated_at.0,
            label,
        });
        self.effects.sort_by_key(|effect| effect.status_index);
    }

    pub fn end(&mut self, status_index: u16) {
        self.effects.retain(|effect| effect.status_index != status_index);
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    pub fn effects(&self) -> &[ActiveStatusEffect] {
        &self.effects
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::ClientTick;

    use super::StatusEffects;

    #[test]
    fn apply_upserts_and_end_removes() {
        let mut bar = StatusEffects::default();
        bar.apply(12, 5000, 5000, ClientTick(100));
        bar.apply(7, 1000, 1000, ClientTick(100));
        assert_eq!(bar.effects().len(), 2);
        assert_eq!(bar.effects()[0].status_index, 7);
        assert!(bar.effects()[0].label.contains("SC#7"));
        bar.apply(12, 2000, 5000, ClientTick(150));
        assert_eq!(bar.effects().len(), 2);
        assert_eq!(
            bar.effects().iter().find(|e| e.status_index == 12).unwrap().remaining_milliseconds,
            2000
        );
        bar.end(7);
        assert_eq!(bar.effects().len(), 1);
        assert_eq!(bar.effects()[0].status_index, 12);
    }
}
