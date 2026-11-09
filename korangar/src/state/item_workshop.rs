use korangar_interface::element::StateElement;
use rust_state::RustState;

const MAX_SYNTHESIS_ROWS: usize = 10;

/// One grade option the player can spend.
#[derive(Clone, Default, RustState, StateElement)]
pub struct GradeMaterialRow {
    pub material_index: i32,
    pub label: String,
}

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

pub fn format_grade_result(result: i32, grade: i16) -> String {
    let outcome = match result {
        0 => "Grade enchant succeeded",
        1 => "Grade enchant failed",
        2 => "Grade enchant lowered the refine",
        3 => "Grade enchant destroyed the item",
        4 => "Grade enchant was protected",
        _ => "Grade enchant finished",
    };
    format!("{outcome} (result {result}, grade {grade})")
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

/// Enchant grade UI.
#[derive(RustState, StateElement)]
pub struct EnchantGradeState {
    pub index_text: String,
    pub index_ghost: String,
    pub blessing_text: String,
    pub blessing_ghost: String,
    pub status: String,
    pub selected_material: Option<i32>,
    pub materials: Vec<GradeMaterialRow>,
}

impl Default for EnchantGradeState {
    fn default() -> Self {
        Self {
            index_text: String::new(),
            index_ghost: "index".to_owned(),
            blessing_text: "0".to_owned(),
            blessing_ghost: "blessing".to_owned(),
            status: String::new(),
            selected_material: None,
            materials: Vec::new(),
        }
    }
}

/// Fields sent in `CZ_GRADE_ENCHANT_REQUEST` besides the protect flag.
pub struct GradeEnchantSubmission {
    pub index: u16,
    pub material_index: i32,
    pub blessing_flag: u8,
    pub blessing_amount: i32,
}

impl EnchantGradeState {
    pub fn open(&mut self, data: i32) {
        self.materials.clear();
        self.selected_material = None;
        self.status = format!("Grade enchant is open (data {data}). Select an inventory index.");
    }

    pub fn set_materials(&mut self, index: Option<u16>, success_chance: i32, blessing_item: i32, rows: Vec<GradeMaterialRow>) {
        self.materials = rows;
        self.selected_material = None;
        self.status = match index {
            Some(index) => format!("Item {index}: {success_chance}% with blessing item {blessing_item}. Pick an option."),
            None => "That item cannot be graded.".to_owned(),
        };
    }

    pub fn pick(&mut self, material_index: i32) {
        self.selected_material = Some(material_index);
        self.status = format!("Option {material_index} selected.");
    }

    pub fn note_result(&mut self, index: u16, grade: i16, result: i32) {
        self.status = format!("{} on index {index}", format_grade_result(result, grade));
    }

    pub fn submission(&self) -> Result<GradeEnchantSubmission, &'static str> {
        let index = parse_u16(&self.index_text).ok_or("Enter an inventory index.")?;
        let material_index = self.selected_material.ok_or("Pick a grade option.")?;
        let blessing_amount = match self.blessing_text.trim() {
            "" => 0,
            text => text.parse::<i32>().map_err(|_| "Blessing amount is not a number.")?,
        };
        let blessing_flag = u8::from(blessing_amount > 0);
        let blessing_amount = blessing_amount.max(0);
        Ok(GradeEnchantSubmission {
            index,
            material_index,
            blessing_flag,
            blessing_amount,
        })
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

    pub fn add_row(&mut self) -> Result<(), &'static str> {
        if self.materials.len() >= MAX_SYNTHESIS_ROWS {
            self.status = format!("Synthesis accepts at most {MAX_SYNTHESIS_ROWS} stacks.");
            return Err("too many");
        }
        let index = parse_u16(&self.index_text).ok_or("Enter an inventory index.")?;
        let count = parse_u16(&self.count_text).filter(|count| *count > 0).ok_or("Enter a count.")?;
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
    use super::{EnchantGradeState, ItemReformState, LaphineSynthesisState, format_grade_result};

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
    fn grade_submission_uses_the_selected_option() {
        let mut state = EnchantGradeState::default();
        state.open(0);
        state.pick(2);
        state.index_text = "4".to_owned();
        state.blessing_text = "3".to_owned();
        let submission = state.submission().unwrap();
        assert_eq!(submission.index, 4);
        assert_eq!(submission.material_index, 2);
        assert_eq!(submission.blessing_flag, 1);
        assert_eq!(submission.blessing_amount, 3);
        state.blessing_text.clear();
        let submission = state.submission().unwrap();
        assert_eq!(submission.blessing_flag, 0);
        assert!(format_grade_result(3, 1).contains("destroyed"));
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
        assert!(state.add_row().is_err());
        assert_eq!(state.stacks().len(), 10);
        state.clear_rows();
        assert!(state.stacks().is_empty());
    }
}
