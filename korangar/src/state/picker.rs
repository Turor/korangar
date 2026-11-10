use korangar_interface::element::StateElement;
use rust_state::RustState;

/// One icon row in the enchant or grade window.
#[derive(Clone, Debug, Default, PartialEq, Eq, RustState, StateElement)]
pub struct PickerRow {
    pub index: u16,
    pub item_id: u32,
    pub label: String,
    pub selected: bool,
}
