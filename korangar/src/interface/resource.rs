use ragnarok_packets::{EquipPosition, HotbarSlot};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemSource {
    Inventory,
    Equipment { position: EquipPosition },
    Storage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillSource {
    SkillTree,
    Hotbar { slot: HotbarSlot },
}
