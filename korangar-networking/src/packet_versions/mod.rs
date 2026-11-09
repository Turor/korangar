pub mod economy;
pub mod eden;
pub mod enchant;
pub mod framing_20190530;
pub mod framing_20250716;
pub mod item_workshop;
pub mod refine;
pub mod stable;
pub mod version_20190530;
pub mod version_20220406;
pub mod version_20250716;

/// All supported packet versions.
#[derive(Debug, Clone, Copy)]
pub enum SupportedPacketVersion {
    _20220406,
    _20250716,
    _20190530,
}
