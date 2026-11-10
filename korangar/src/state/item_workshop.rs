use korangar_interface::element::StateElement;
use rust_state::RustState;

const MAX_SYNTHESIS_ROWS: usize = 10;

/// One stack queued for Laphine synthesis.
#[derive(Clone, Default, RustState, StateElement)]
pub struct LaphineMaterialRow {
    pub index: u16,
    pub count: u16,
    pub label: String,
}

fn parse_u16(text: &str) -> Option<u16> {
    text.trim().parse().ok()
}

/// Why a synthesis row was rejected. The cap writes its own status line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynthesisAddError {
    TooMany,
    Message(&'static str),
}

pub fn format_reform_result(result: i8, index: u16) -> String {
    if result == 0 {
        format!("Item reform succeeded (index {index})")
    } else {
        format!("Item reform failed (result {result}, index {index})")
    }
}

pub fn format_laphine_result(kind: &str, result: i32) -> String {
    if result == 0 {
        format!("{kind} succeeded")
    } else {
        format!("{kind} failed (result {result})")
    }
}

/// Item reform UI. The catalyst id came from the server.
#[derive(RustState, StateElement)]
pub struct ItemReformState {
    pub catalyst_item_id: u32,
    pub index_text: String,
    pub index_ghost: String,
    pub status: String,
}

impl Default for ItemReformState {
    fn default() -> Self {
        Self {
            catalyst_item_id: 0,
            index_text: String::new(),
            index_ghost: "index".to_owned(),
            status: String::new(),
        }
    }
}

impl ItemReformState {
    pub fn open(&mut self, item_id: u32) {
        self.catalyst_item_id = item_id;
        self.status = format!("Reform catalyst {item_id}. Enter an inventory index.");
    }

    pub fn note_result(&mut self, index: u16, result: i8) -> bool {
        self.status = format_reform_result(result, index);
        result == 0
    }

    pub fn index(&self) -> Result<u16, &'static str> {
        parse_u16(&self.index_text).ok_or("Enter an inventory index.")
    }
}

/// Laphine synthesis (LaphineSys).
#[derive(RustState, StateElement)]
pub struct LaphineSynthesisState {
    pub item_id: u32,
    pub index_text: String,
    pub count_text: String,
    pub index_ghost: String,
    pub count_ghost: String,
    pub status: String,
    pub materials: Vec<LaphineMaterialRow>,
}

impl Default for LaphineSynthesisState {
    fn default() -> Self {
        Self {
            item_id: 0,
            index_text: String::new(),
            count_text: "1".to_owned(),
            index_ghost: "index".to_owned(),
            count_ghost: "count".to_owned(),
            status: String::new(),
            materials: Vec::new(),
        }
    }
}

impl LaphineSynthesisState {
    pub fn open(&mut self, item_id: u32) {
        self.item_id = item_id;
        self.materials.clear();
        self.status = format!("Laphine synthesis item {item_id}. Add inventory stacks, then submit.");
    }

    pub fn add_row(&mut self) -> Result<(), SynthesisAddError> {
        if self.materials.len() >= MAX_SYNTHESIS_ROWS {
            self.status = format!("Synthesis accepts at most {MAX_SYNTHESIS_ROWS} stacks.");
            return Err(SynthesisAddError::TooMany);
        }
        let index = parse_u16(&self.index_text).ok_or(SynthesisAddError::Message("Enter an inventory index."))?;
        let count = parse_u16(&self.count_text)
            .filter(|count| *count > 0)
            .ok_or(SynthesisAddError::Message("Enter a count."))?;
        self.materials.push(LaphineMaterialRow {
            index,
            count,
            label: format!("index {index} x{count}"),
        });
        self.status = self.materials.iter().map(|row| row.label.as_str()).collect::<Vec<_>>().join(", ");
        Ok(())
    }

    pub fn clear_rows(&mut self) {
        self.materials.clear();
        self.status = "Synthesis list cleared.".to_owned();
    }

    pub fn stacks(&self) -> Vec<(u16, u16)> {
        self.materials.iter().map(|row| (row.index, row.count)).collect()
    }

    pub fn note_result(&mut self, result: i16) {
        self.status = format_laphine_result("Laphine synthesis", i32::from(result));
    }
}

/// Laphine upgrade (LaphineUpg).
#[derive(RustState, StateElement)]
pub struct LaphineUpgradeState {
    pub item_id: u32,
    pub index_text: String,
    pub index_ghost: String,
    pub status: String,
}

impl Default for LaphineUpgradeState {
    fn default() -> Self {
        Self {
            item_id: 0,
            index_text: String::new(),
            index_ghost: "index".to_owned(),
            status: String::new(),
        }
    }
}

impl LaphineUpgradeState {
    pub fn open(&mut self, item_id: u32) {
        self.item_id = item_id;
        self.status = format!("Laphine upgrade item {item_id}. Enter an inventory index.");
    }

    pub fn index(&self) -> Result<u16, &'static str> {
        parse_u16(&self.index_text).ok_or("Enter an inventory index.")
    }

    pub fn note_result(&mut self, result: u16) {
        self.status = format_laphine_result("Laphine upgrade", i32::from(result));
    }
}

#[cfg(test)]
mod tests {
    use super::{ItemReformState, LaphineSynthesisState};

    #[test]
    fn reform_success_closes_and_failure_stays() {
        let mut state = ItemReformState::default();
        state.open(4001);
        assert!(state.note_result(3, 0));
        assert!(state.status.contains("succeeded"));
        let mut state = ItemReformState::default();
        assert!(!state.note_result(3, 1));
        assert!(state.status.contains("failed"));
    }

    #[test]
    fn synthesis_caps_the_queue() {
        let mut state = LaphineSynthesisState::default();
        state.open(7210);
        state.index_text = "1".to_owned();
        state.count_text = "2".to_owned();
        for _ in 0..10 {
            state.add_row().unwrap();
        }
        assert_eq!(state.add_row(), Err(super::SynthesisAddError::TooMany));
        assert_eq!(state.stacks().len(), 10);
        state.clear_rows();
        assert!(state.stacks().is_empty());
    }
}
