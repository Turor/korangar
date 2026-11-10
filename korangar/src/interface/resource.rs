use ragnarok_packets::{EquipPosition, HotbarSlot};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemSource {
    Inventory,
    Cart,
    Equipment { position: EquipPosition },
    Storage,
    Hotbar { tab: ragnarok_packets::HotbarTab, slot: HotbarSlot },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillSource {
    SkillTree,
    Hotbar {
        tab: ragnarok_packets::HotbarTab,
        slot: HotbarSlot,
    },
}
