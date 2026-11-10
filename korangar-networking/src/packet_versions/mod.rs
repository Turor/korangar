pub mod achievements;
pub mod bank_cash_roulette;
pub mod clan;
pub mod companions;
pub mod economy;
pub mod eden;
pub mod enchant;
pub mod framing_20190530;
pub mod framing_20250716;
pub mod item_use;
pub mod item_workshop;
pub mod mvp_minimap;
pub mod refine;
pub mod rodex;
pub mod stable;
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
