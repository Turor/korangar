use korangar_interface::element::StateElement;
use rust_state::RustState;

/// An identified equippable stack the refine window can select.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefineEquip {
    pub index: u16,
    pub item_id: u32,
    pub refine: u8,
}

/// One ore row, with a label that includes the chance when the server sent one.
#[derive(Clone, Default, RustState, StateElement)]
pub struct RefineMaterialRow {
    pub item_id: u32,
    pub label: String,
}

/// One item the player can place in the refine window.
#[derive(Clone, Default, RustState, StateElement)]
pub struct RefineCandidate {
    pub index: u16,
    pub item_id: u32,
    pub label: String,
}

/// In-progress refine UI. The server keeps the character still until close.
#[derive(Default, RustState, StateElement)]
pub struct RefineState {
    pub open: bool,
    pub selected_index: Option<u16>,
    pub selected_item_id: Option<u32>,
    pub selected_label: String,
    pub blessing: String,
    pub status: String,
    pub candidates: Vec<RefineCandidate>,
    pub materials: Vec<RefineMaterialRow>,
    /// The last attempt left the item in the inventory, so the client asks for
    /// a fresh ore list.
    pub rearm: bool,
}

/// Public item ids for the ores the refine UI names when it sees them.
pub fn refine_ore_name(item_id: u32) -> Option<&'static str> {
    Some(match item_id {
        984 => "Oridecon",
        985 => "Elunium",
        1010 => "Phracon",
        1011 => "Emveretarcon",
        6223 => "Bradium",
        6224 => "Carnium",
        6225 => "HD Bradium",
        6226 => "HD Carnium",
        6240 => "HD Elunium",
        6241 => "HD Oridecon",
        6635 => "Blacksmith Blessing",
        7619 => "Enriched Elunium",
        7620 => "Enriched Oridecon",
        _ => return None,
    })
}

pub fn material_label(item_id: u32, chance_percent: u8, zeny: u32) -> String {
    match refine_ore_name(item_id) {
        Some(name) => format!("{name}  {chance_percent}%  {zeny}z"),
        None => format!("Ore {item_id}  {chance_percent}%  {zeny}z"),
    }
}

fn candidate_label(equip: &RefineEquip) -> String {
    format!("+{} item {}", equip.refine, equip.item_id)
}

impl RefineState {
    pub fn open_with(&mut self, equips: &[RefineEquip]) {
        self.open = true;
        self.rearm = false;
        self.selected_index = None;
        self.selected_item_id = None;
        self.selected_label.clear();
        self.blessing.clear();
        self.materials.clear();
        self.status = "Choose an item to refine.".to_owned();
        self.candidates = equips
            .iter()
            .map(|equip| RefineCandidate {
                index: equip.index,
                item_id: equip.item_id,
                label: candidate_label(equip),
            })
            .collect();
    }

    pub fn select(&mut self, index: u16) {
        if let Some(candidate) = self.candidates.iter().find(|candidate| candidate.index == index) {
            self.selected_index = Some(candidate.index);
            self.selected_item_id = Some(candidate.item_id);
            self.selected_label = candidate.label.clone();
            self.materials.clear();
            self.rearm = false;
            self.status = format!("Checking ores for {}.", candidate.label);
        }
    }

    pub fn set_materials(&mut self, index: u16, blacksmith_blessing: u8, rows: Vec<RefineMaterialRow>) {
        if self.selected_index.is_some() && self.selected_index != Some(index) {
            return;
        }
        self.selected_index = Some(index);
        self.rearm = false;
        self.blessing = match blacksmith_blessing {
            0 => String::new(),
            amount => format!("Blacksmith Blessing x{amount}"),
        };
        let count = rows.len();
        self.materials = rows;
        self.status = match count {
            0 => "This item cannot be refined further.".to_owned(),
            _ => format!("{count} ore choices."),
        };
    }

    /// Apply `ZC_ACK_ITEMREFINING`.
    ///
    /// Success, a downgrade, and a safe failure leave the item in the bag, so
    /// the caller sends select again (re-arm). Destruction does not.
    pub fn note_result(&mut self, index: u16, result: u16, refine_level: u8) -> Option<u16> {
        self.status = match result {
            0 => format!("Refine succeeded (+{refine_level})."),
            1 => "Refine failed. The item was destroyed.".to_owned(),
            2 => format!("Refine failed. The item is now +{refine_level}."),
            3 => format!("Refine failed. The item stayed +{refine_level}."),
            other => format!("Refine finished ({other})."),
        };
        if let Some(candidate) = self.candidates.iter_mut().find(|candidate| candidate.index == index)
            && result != 1
        {
            candidate.label = format!("+{refine_level} item {}", candidate.item_id);
            if self.selected_index == Some(index) {
                self.selected_label = candidate.label.clone();
            }
        }
        if !self.open {
            return None;
        }
        match result {
            0 | 2 | 3 if self.selected_index == Some(index) => {
                self.materials.clear();
                self.rearm = true;
                Some(index)
            }
            1 => {
                self.candidates.retain(|candidate| candidate.index != index);
                if self.selected_index == Some(index) {
                    self.selected_index = None;
                    self.selected_item_id = None;
                    self.selected_label.clear();
                    self.materials.clear();
                }
                self.rearm = false;
                None
            }
            _ => None,
        }
    }

    pub fn close(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RefineState {
        let mut refine = RefineState::default();
        refine.open_with(&[RefineEquip {
            index: 3,
            item_id: 1101,
            refine: 7,
        }]);
        refine.select(3);
        refine
    }

    #[test]
    fn bradium_and_carnium_are_named_and_chances_are_shown() {
        assert_eq!(refine_ore_name(6223), Some("Bradium"));
        assert_eq!(refine_ore_name(6224), Some("Carnium"));
        assert_eq!(material_label(6223, 70, 20_000), "Bradium  70%  20000z");
        assert_eq!(material_label(6224, 40, 50_000), "Carnium  40%  50000z");
        assert!(material_label(9, 10, 1).starts_with("Ore 9"));
    }

    #[test]
    fn success_rearms_the_same_item() {
        let mut refine = sample();
        assert_eq!(refine.note_result(3, 0, 8), Some(3));
        assert!(refine.rearm);
        assert!(refine.materials.is_empty());
        assert_eq!(refine.selected_label, "+8 item 1101");
    }

    #[test]
    fn downgrade_and_safe_failure_rearm() {
        let mut refine = sample();
        assert_eq!(refine.note_result(3, 2, 6), Some(3));
        let mut refine = sample();
        assert_eq!(refine.note_result(3, 3, 7), Some(3));
    }

    #[test]
    fn destruction_does_not_rearm() {
        let mut refine = sample();
        assert_eq!(refine.note_result(3, 1, 7), None);
        assert!(refine.selected_index.is_none());
        assert!(refine.candidates.is_empty());
        assert!(!refine.rearm);
    }

    #[test]
    fn a_result_without_the_window_does_not_rearm() {
        let mut refine = RefineState::default();
        assert_eq!(refine.note_result(3, 0, 4), None);
        assert!(!refine.rearm);
    }
}
