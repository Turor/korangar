//! Enchant grade window state.
//!
//! The server sends the material list, success chance, and blessing offer
//! (`ZC_GRADE_ENCHANT_MATERIAL_LIST`). The player picks an equip from the
//! inventory and adjusts the blessing with the plus and minus controls.

use korangar_interface::element::StateElement;
use rust_state::RustState;

use super::enchant::OwnedStack;
use super::picker::PickerRow;

pub fn format_grade_result(result: i32, grade: i16) -> String {
    let outcome = match result {
        0 => "Grade enchant succeeded",
        1 => "Grade enchant failed",
        2 => "Grade enchant lowered the grade",
        3 => "Grade enchant destroyed the item",
        4 => "Grade enchant was protected",
        _ => "Grade enchant finished",
    };
    format!("{outcome} (result {result}, grade {grade})")
}

/// Result 0 forces refine to 0. Result 2 keeps the current refine and shows the
/// grade after the drop.
fn grade_result_label(label: &str, grade: i16, zero_refine: bool) -> String {
    let (refine, rest) = label.split_once(' ').unwrap_or(("+0", label));
    let name = rest.split(" (grade ").next().unwrap_or(rest);
    let refine = if zero_refine { "+0" } else { refine };
    if grade > 0 {
        format!("{refine} {name} (grade {grade})")
    } else {
        format!("{refine} {name}")
    }
}

/// Fields sent in `CZ_GRADE_ENCHANT_REQUEST` besides the protect flag.
pub struct GradeEnchantSubmission {
    pub index: u16,
    pub material_index: i32,
    pub blessing_flag: u8,
    pub blessing_amount: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GradeBlessingOffer {
    pub item_id: u32,
    pub name: String,
    pub amount: i32,
    pub max_blessing: i32,
    pub bonus: i32,
    pub owned: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GradeOption {
    pub material_index: i32,
    pub item_id: u32,
    pub amount: i32,
    pub price: i32,
    pub downgrade: bool,
    pub breakable: bool,
    pub name: String,
    /// How many of `item_id` the inventory currently holds.
    pub owned: u32,
}

#[derive(Default, RustState, StateElement)]
pub struct EnchantGradeState {
    pub status: String,
    pub chance: String,
    pub blessing: String,
    pub cost: String,
    pub candidates: Vec<PickerRow>,
    pub materials: Vec<PickerRow>,
    selected_index: Option<u16>,
    selected_material: Option<i32>,
    blessing_count: i32,
    success_chance: i32,
    #[hidden_element]
    offer: GradeBlessingFields,
    #[hidden_element]
    options: Vec<GradeOption>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub(crate) struct GradeBlessingFields {
    item_id: u32,
    name: String,
    amount: i32,
    max_blessing: i32,
    bonus: i32,
    owned: u32,
}

impl EnchantGradeState {
    pub fn open(&mut self, rows: Vec<PickerRow>) {
        *self = Self::default();
        self.candidates = rows;
        self.status = if self.candidates.is_empty() {
            "No equipment can be graded.".to_owned()
        } else {
            "Choose equipment to grade.".to_owned()
        };
    }

    pub fn select(&mut self, index: u16) -> bool {
        if !self.candidates.iter().any(|row| row.index == index) {
            self.status = "That item cannot be graded.".to_owned();
            return false;
        }
        self.selected_index = Some(index);
        self.selected_material = None;
        self.blessing_count = 0;
        self.materials.clear();
        self.chance.clear();
        self.blessing.clear();
        self.cost.clear();
        self.options.clear();
        for row in &mut self.candidates {
            row.selected = row.index == index;
        }
        let label = self
            .candidates
            .iter()
            .find(|row| row.index == index)
            .map(|row| row.label.clone())
            .unwrap_or_default();
        self.status = format!("Checking grade materials for {label}.");
        true
    }

    pub fn set_materials(&mut self, index: Option<u16>, success_chance: i32, offer: GradeBlessingOffer, options: Vec<GradeOption>) {
        self.blessing_count = 0;
        self.selected_material = None;
        let Some(index) = index else {
            self.success_chance = 0;
            self.offer = GradeBlessingFields::default();
            self.options.clear();
            self.materials.clear();
            self.chance.clear();
            self.blessing.clear();
            self.cost.clear();
            self.status = "That item cannot be graded.".to_owned();
            return;
        };
        self.success_chance = success_chance;
        self.offer = GradeBlessingFields {
            item_id: offer.item_id,
            name: offer.name,
            amount: offer.amount,
            max_blessing: offer.max_blessing,
            bonus: offer.bonus,
            owned: offer.owned,
        };
        self.options = options;
        self.selected_index = Some(index);
        for row in &mut self.candidates {
            row.selected = row.index == index;
        }
        self.status = if self.options.is_empty() {
            "That item cannot be graded.".to_owned()
        } else {
            "Choose a material, then grade.".to_owned()
        };
        self.refresh_lines();
    }

    pub fn pick(&mut self, material_index: i32) {
        if !self.options.iter().any(|option| option.material_index == material_index) {
            return;
        }
        self.selected_material = Some(material_index);
        self.refresh_lines();
        self.status = "Material selected.".to_owned();
    }

    pub fn add_blessing(&mut self) {
        if self.offer.item_id == 0 {
            self.status = "This grade has no blessing.".to_owned();
            return;
        }
        if self.blessing_count >= self.offer.max_blessing {
            self.status = "Blessing is already at the maximum.".to_owned();
            return;
        }
        let next = self.blessing_count + 1;
        let needed = next.saturating_mul(self.offer.amount).max(0) as u32;
        if self.offer.owned < needed {
            self.status = "Not enough blessing items.".to_owned();
            return;
        }
        self.blessing_count = next;
        self.refresh_lines();
    }

    pub fn remove_blessing(&mut self) {
        if self.blessing_count > 0 {
            self.blessing_count -= 1;
            self.refresh_lines();
        }
    }

    /// Copy current bag counts onto the material rows and the blessing offer.
    /// A rejected item has no rows, so this leaves the cleared offer alone.
    pub fn reload_owned(&mut self, stock: &[OwnedStack]) {
        let amount_of = |item_id: u32| {
            stock
                .iter()
                .find(|stack| stack.item_id == item_id)
                .map(|stack| stack.amount)
                .unwrap_or(0)
        };
        for option in &mut self.options {
            option.owned = amount_of(option.item_id);
        }
        if self.offer.item_id != 0 {
            self.offer.owned = amount_of(self.offer.item_id);
        }
        if self.options.is_empty() || self.selected_index.is_none() {
            return;
        }
        while self.blessing_count > 0 {
            let needed = u32::try_from(self.blessing_count.saturating_mul(self.offer.amount.max(0))).unwrap_or(u32::MAX);
            if self.offer.owned >= needed {
                break;
            }
            self.blessing_count -= 1;
        }
        self.refresh_lines();
    }

    pub fn note_result(&mut self, index: u16, grade: i16, result: i32) {
        // A downgrade ack carries the grade from before Hercules subtracts one.
        let shown = if result == 2 { grade.saturating_sub(1).max(0) } else { grade };
        self.status = format!("{} on index {index}", format_grade_result(result, shown));
        if result == 0 || result == 2 {
            if let Some(row) = self.candidates.iter_mut().find(|row| row.index == index) {
                row.label = grade_result_label(&row.label, shown, result == 0);
            }
            self.clear_offer();
        }
        if result == 3 {
            self.candidates.retain(|row| row.index != index);
            if self.selected_index == Some(index) {
                self.selected_index = None;
                self.materials.clear();
                self.chance.clear();
                self.cost.clear();
            }
        }
    }

    fn clear_offer(&mut self) {
        self.selected_material = None;
        self.blessing_count = 0;
        self.success_chance = 0;
        self.offer = GradeBlessingFields::default();
        self.options.clear();
        self.materials.clear();
        self.chance.clear();
        self.blessing.clear();
        self.cost.clear();
    }

    pub fn submission(&self) -> Result<GradeEnchantSubmission, &'static str> {
        let index = self.selected_index.ok_or("Choose equipment to grade.")?;
        let material_index = self.selected_material.ok_or("Choose a grade material.")?;
        if self.displayed_chance() <= 0 {
            return Err("The success chance is zero.");
        }
        let owned = self
            .options
            .iter()
            .find(|option| option.material_index == material_index)
            .map(|option| (option.owned, option.amount))
            .unwrap_or((0, 1));
        let needed = u32::try_from(owned.1.max(0)).unwrap_or(u32::MAX);
        if owned.0 < needed {
            return Err("Not enough materials.");
        }
        // Hercules multiplies this count by the blessing's per-step item amount.
        let blessing_amount = self.blessing_count.max(0);
        Ok(GradeEnchantSubmission {
            index,
            material_index,
            blessing_flag: u8::from(self.blessing_count > 0),
            blessing_amount,
        })
    }

    fn displayed_chance(&self) -> i32 {
        self.success_chance + self.blessing_count.saturating_mul(self.offer.bonus)
    }

    fn refresh_lines(&mut self) {
        self.chance = format!("Success {}%", self.displayed_chance());
        if self.offer.item_id == 0 {
            self.blessing.clear();
        } else {
            let used = self.blessing_count.saturating_mul(self.offer.amount.max(0));
            let name = if self.offer.name.is_empty() {
                format!("Item {}", self.offer.item_id)
            } else {
                self.offer.name.clone()
            };
            self.blessing = format!(
                "{name} x{used} (owned {}, max {}, +{}% each)",
                self.offer.owned, self.offer.max_blessing, self.offer.bonus
            );
        }
        self.materials = self
            .options
            .iter()
            .map(|option| {
                let flags = match (option.downgrade, option.breakable) {
                    (true, true) => "downgrade, break",
                    (true, false) => "downgrade",
                    (false, true) => "break",
                    (false, false) => "safe",
                };
                let name = if option.name.is_empty() {
                    format!("Item {}", option.item_id)
                } else {
                    option.name.clone()
                };
                PickerRow {
                    index: u16::try_from(option.material_index).unwrap_or(0),
                    item_id: option.item_id,
                    label: format!("{name} x{} (have {})  {}z  {flags}", option.amount, option.owned, option.price),
                    selected: self.selected_material == Some(option.material_index),
                }
            })
            .collect();
        self.cost = self
            .options
            .iter()
            .find(|option| Some(option.material_index) == self.selected_material)
            .map(|option| format!("Cost {}z", option.price))
            .unwrap_or_else(|| "Cost 0z".to_owned());
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::ByteWriter;
    use ragnarok_packets::{GradeEnchantRequestPacket, InventoryIndex, PacketExt};

    use super::*;
    use crate::state::enchant::OwnedStack;
    use crate::state::picker::PickerRow;

    fn opened() -> EnchantGradeState {
        let mut state = EnchantGradeState::default();
        state.open(vec![PickerRow {
            index: 4,
            item_id: 1201,
            label: "+9 Knife".to_owned(),
            selected: false,
        }]);
        assert!(state.select(4));
        state.set_materials(
            Some(4),
            40,
            GradeBlessingOffer {
                item_id: 6635,
                name: "Blessing".to_owned(),
                amount: 2,
                max_blessing: 3,
                bonus: 5,
                owned: 6,
            },
            vec![GradeOption {
                material_index: 1,
                item_id: 984,
                amount: 1,
                price: 1000,
                downgrade: false,
                breakable: false,
                name: "Oridecon".to_owned(),
                owned: 5,
            }],
        );
        state.pick(1);
        state
    }

    #[test]
    fn blessing_stops_at_the_owned_stack_and_raises_the_chance() {
        let mut state = opened();
        assert_eq!(state.chance, "Success 40%");
        state.add_blessing();
        state.add_blessing();
        state.add_blessing();
        assert_eq!(state.blessing_count, 3);
        assert!(state.status.contains("maximum") || state.blessing_count == 3);
        state.add_blessing();
        assert_eq!(state.blessing_count, 3);
        assert_eq!(state.chance, "Success 55%");
        let submission = state.submission().unwrap();
        assert_eq!(submission.blessing_flag, 1);
        assert_eq!(submission.blessing_amount, 3);
        assert_eq!(submission.material_index, 1);
        assert_eq!(submission.index, 4);

        let mut writer = ByteWriter::new();
        GradeEnchantRequestPacket {
            index: InventoryIndex(submission.index),
            material_index: submission.material_index,
            blessing_flag: submission.blessing_flag,
            blessing_amount: submission.blessing_amount,
            protect_flag: 0,
        }
        .packet_to_bytes(&mut writer)
        .unwrap();
        assert_eq!(writer.as_slice().len(), 14);
        assert_eq!(writer.as_slice()[0], 0x5B);
        // InventoryIndex writes the stored index plus 2.
        assert_eq!(&writer.as_slice()[2..4], &6u16.to_le_bytes());
    }

    #[test]
    fn blessing_refuses_a_stack_the_player_does_not_have() {
        let mut state = opened();
        state.offer.owned = 2;
        state.add_blessing();
        state.add_blessing();
        assert_eq!(state.blessing_count, 1);
        assert!(state.status.contains("Not enough"));
    }

    #[test]
    fn destroyed_result_drops_the_item_and_zero_chance_does_not_submit() {
        let mut state = opened();
        state.success_chance = 0;
        state.refresh_lines();
        assert!(state.submission().is_err());
        state.success_chance = 40;
        state.note_result(4, 0, 3);
        assert!(state.status.contains("destroyed"));
        assert!(state.candidates.is_empty());
        assert!(format_grade_result(0, 2).contains("succeeded"));
    }

    #[test]
    fn an_empty_material_stack_does_not_submit() {
        let mut state = opened();
        state.options[0].owned = 0;
        assert!(matches!(state.submission(), Err("Not enough materials.")));
    }

    #[test]
    fn a_rejected_item_clears_the_previous_offer() {
        let mut state = opened();
        assert!(!state.chance.is_empty());
        state.set_materials(
            None,
            90,
            GradeBlessingOffer {
                item_id: 6635,
                name: "Blessing".to_owned(),
                amount: 2,
                max_blessing: 3,
                bonus: 5,
                owned: 6,
            },
            vec![GradeOption {
                material_index: 0,
                item_id: 984,
                amount: 1,
                price: 1000,
                downgrade: true,
                breakable: true,
                name: "Oridecon".to_owned(),
                owned: 9,
            }],
        );
        assert_eq!(state.status, "That item cannot be graded.");
        assert!(state.chance.is_empty());
        assert!(state.blessing.is_empty());
        assert!(state.cost.is_empty());
        assert!(state.materials.is_empty());
        assert!(state.options.is_empty());
        assert_eq!(state.offer, GradeBlessingFields::default());
        assert_eq!(state.success_chance, 0);
        assert!(state.submission().is_err());
        state.reload_owned(&[OwnedStack { item_id: 984, amount: 9 }]);
        assert!(state.chance.is_empty());
        assert!(state.options.is_empty());
    }

    #[test]
    fn reload_owned_blocks_a_stack_the_old_list_still_counts() {
        let mut state = opened();
        state.pick(1);
        assert!(state.submission().is_ok());
        state.reload_owned(&[OwnedStack { item_id: 984, amount: 0 }]);
        assert!(state.materials[0].label.contains("have 0"));
        assert!(matches!(state.submission(), Err("Not enough materials.")));
    }

    #[test]
    fn a_successful_grade_drops_the_old_material_list() {
        let mut state = opened();
        state.pick(1);
        assert!(matches!(
            state.submission(),
            Ok(GradeEnchantSubmission { material_index: 1, .. })
        ));
        state.note_result(4, 2, 0);
        assert!(state.submission().is_err());
        assert!(state.chance.is_empty());
        assert!(state.cost.is_empty());
        assert!(state.blessing.is_empty());
        assert!(state.options.is_empty());
        assert_eq!(state.candidates[0].label, "+0 Knife (grade 2)");

        assert!(state.select(4));
        state.set_materials(
            Some(4),
            40,
            GradeBlessingOffer {
                item_id: 6635,
                name: "Blessing".to_owned(),
                amount: 2,
                max_blessing: 3,
                bonus: 5,
                owned: 6,
            },
            vec![GradeOption {
                material_index: 3,
                item_id: 985,
                amount: 1,
                price: 2000,
                downgrade: false,
                breakable: false,
                name: "Elunium".to_owned(),
                owned: 4,
            }],
        );
        state.pick(3);
        assert!(matches!(
            state.submission(),
            Ok(GradeEnchantSubmission { material_index: 3, .. })
        ));
    }

    #[test]
    fn a_failed_or_protected_grade_keeps_the_material_list() {
        let mut state = opened();
        state.pick(1);
        state.note_result(4, 1, 1);
        assert!(matches!(
            state.submission(),
            Ok(GradeEnchantSubmission { material_index: 1, .. })
        ));
        state.note_result(4, 1, 4);
        assert!(matches!(
            state.submission(),
            Ok(GradeEnchantSubmission { material_index: 1, .. })
        ));
        assert_eq!(state.candidates[0].label, "+9 Knife");
    }

    #[test]
    fn a_downgrade_stores_the_previous_grade_minus_one_and_drops_the_offer() {
        let mut state = opened();
        state.candidates[0].label = "+9 Knife (grade 1)".to_owned();
        assert!(matches!(
            state.submission(),
            Ok(GradeEnchantSubmission { material_index: 1, .. })
        ));

        let mut inventory = crate::state::inventory::Inventory::from_items(vec![graded_knife(9, 1)]);
        inventory.apply_enchant_grade(InventoryIndex(4), 1, 2);
        assert_eq!(inventory.equip_levels(InventoryIndex(4)), Some((9, 0)));

        state.note_result(4, 1, 2);
        assert!(state.status.contains("lowered the grade"));
        assert!(!state.status.contains("refine"));
        assert_eq!(state.candidates[0].label, "+9 Knife");
        assert!(state.submission().is_err());
        assert!(state.options.is_empty());
        assert!(state.chance.is_empty());

        assert!(state.select(4));
        state.set_materials(
            Some(4),
            40,
            GradeBlessingOffer {
                item_id: 6635,
                name: "Blessing".to_owned(),
                amount: 2,
                max_blessing: 3,
                bonus: 5,
                owned: 6,
            },
            vec![GradeOption {
                material_index: 5,
                item_id: 985,
                amount: 5,
                price: 1000,
                downgrade: true,
                breakable: false,
                name: "Etel Skyblue Jewel".to_owned(),
                owned: 5,
            }],
        );
        state.pick(5);
        assert!(matches!(
            state.submission(),
            Ok(GradeEnchantSubmission { material_index: 5, .. })
        ));
    }

    fn graded_knife(refine: u8, grade: u8) -> korangar_networking::InventoryItem<crate::world::ResourceMetadata> {
        let option = || ragnarok_packets::ItemOptions {
            index: 0,
            value: 0,
            parameter: 0,
        };
        korangar_networking::InventoryItem {
            metadata: crate::world::ResourceMetadata {
                texture: None,
                name: "Knife".to_owned(),
                description: String::new(),
            },
            index: InventoryIndex(4),
            item_id: ragnarok_packets::ItemId(1201),
            item_type: 5,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: korangar_networking::InventoryItemDetails::Equippable {
                equip_position: ragnarok_packets::EquipPosition::empty(),
                equipped_position: ragnarok_packets::EquipPosition::empty(),
                bind_on_equip_type: 0,
                w_item_sprite_number: 0,
                option_count: 0,
                option_data: [option(), option(), option(), option(), option()],
                forge_data: None,
                refinement_level: refine,
                enchantment_level: grade,
                flags: ragnarok_packets::EquippableItemFlags::IDENTIFIED,
            },
        }
    }
}
