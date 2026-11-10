pub mod achievements;
pub mod bank_cash_roulette;
pub mod barter;
pub mod clan;
pub mod guild;
pub mod companions;
pub mod economy;
pub mod eden;
pub mod enchant;
pub mod framing_20190530;
pub mod framing_20250716;
pub mod item_skill_selection;
pub mod item_use;
pub mod item_workshop;
pub mod mvp_minimap;
pub mod npc_dialog;
pub mod party;
pub mod quest_renewal;
pub mod refine;
pub mod rodex;
pub mod stable;
pub mod styling_shop;
pub mod version_20190530;
pub mod version_20220406;
pub mod version_20250716;
pub mod warpra;

/// All supported packet versions.
#[derive(Debug, Clone, Copy)]
pub enum SupportedPacketVersion {
    _20220406,
    _20250716,
    _20190530,
}
