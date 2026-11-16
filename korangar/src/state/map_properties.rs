//! Map property flags and map type for the status badge.
//!
//! Hercules `clif_maptypeproperty2` (`0x099B`) always sends type `0x28`. The
//! `u32` flags are the roBrowser `MapFlag` bits. `noteleport`, `noreturn`, and
//! `nowarp` are server mapflags only: they are not in this bitfield and never
//! arrive on `0x099B` or `0x01D6`. The closest client-visible restriction is
//! map type Jail (`15`) or No skill (`13`).

use korangar_interface::element::StateElement;
use rust_state::RustState;

const FLAG_PVP: u32 = 1 << 0;
const FLAG_GVG: u32 = 1 << 1;
const FLAG_SIEGE: u32 = 1 << 2;
const FLAG_USE_SIMPLE_EFFECT: u32 = 1 << 3;
const FLAG_DISABLE_LOCKON: u32 = 1 << 4;
const FLAG_COUNT_PK: u32 = 1 << 5;
const FLAG_NO_PARTY_FORMATION: u32 = 1 << 6;
const FLAG_BATTLEFIELD: u32 = 1 << 7;
const FLAG_DISABLE_COSTUME: u32 = 1 << 8;
const FLAG_USE_CART: u32 = 1 << 9;
const FLAG_SUN_MOON_STAR: u32 = 1 << 10;

/// One decoded `0x099B` flag word. Spare bits above 10 stay off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapFlagBits {
    pub pvp: bool,
    pub gvg: bool,
    pub siege: bool,
    pub use_simple_effect: bool,
    pub disable_lockon: bool,
    pub count_pk: bool,
    pub no_party_formation: bool,
    pub battlefield: bool,
    pub disable_costume: bool,
    pub use_cart: bool,
    pub sun_moon_star: bool,
}

/// Loaded map name plus the property and type the server last sent.
///
/// `badge` is empty on a normal map. The status panel paints it only then.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct MapProperties {
    #[hidden_element]
    map_name: String,
    #[hidden_element]
    property_type: u16,
    #[hidden_element]
    flags: u32,
    #[hidden_element]
    map_type: u16,
    badge: String,
}

impl MapProperties {
    /// `0x099B`. Type `0` and `0x28` carry no zone by themselves. Types `1`,
    /// `2`, `4`, and `5` are PvP, `3` is GvG, and `6` is no-skill (the older
    /// `ZC_NOTIFY_MAPPROPERTY` enum).
    pub fn apply_property(&mut self, property_type: u16, flags: u32) {
        self.property_type = property_type;
        self.flags = flags;
        self.rebuild_badge();
    }

    /// `0x01D6`. `map_type` is Hercules `enum map_type`.
    pub fn apply_map_type(&mut self, map_type: u16) {
        self.map_type = map_type;
        self.rebuild_badge();
    }

    /// A map change drops the previous zone until the new map's packets arrive.
    pub fn note_map(&mut self, map_name: &str) {
        self.map_name = strip_map_name(map_name).to_owned();
        self.property_type = 0;
        self.flags = 0;
        self.map_type = 0;
        self.badge.clear();
    }

    /// Empty loaded name accepts any cell packet. Otherwise the `.gat` suffix
    /// and trailing NULs are ignored.
    pub fn accepts_map(&self, map_name: &str) -> bool {
        self.map_name.is_empty() || self.map_name == strip_map_name(map_name)
    }

    pub fn map_name(&self) -> &str {
        &self.map_name
    }

    pub fn badge_text(&self) -> &str {
        &self.badge
    }

    pub fn flag_bits(&self) -> MapFlagBits {
        MapFlagBits {
            pvp: self.flags & FLAG_PVP != 0,
            gvg: self.flags & FLAG_GVG != 0,
            siege: self.flags & FLAG_SIEGE != 0,
            use_simple_effect: self.flags & FLAG_USE_SIMPLE_EFFECT != 0,
            disable_lockon: self.flags & FLAG_DISABLE_LOCKON != 0,
            count_pk: self.flags & FLAG_COUNT_PK != 0,
            no_party_formation: self.flags & FLAG_NO_PARTY_FORMATION != 0,
            battlefield: self.flags & FLAG_BATTLEFIELD != 0,
            disable_costume: self.flags & FLAG_DISABLE_COSTUME != 0,
            use_cart: self.flags & FLAG_USE_CART != 0,
            sun_moon_star: self.flags & FLAG_SUN_MOON_STAR != 0,
        }
    }

    fn rebuild_badge(&mut self) {
        let bits = self.flag_bits();
        let pvp = bits.pvp || matches!(self.map_type, 5 | 6 | 11 | 12 | 17 | 20) || matches!(self.property_type, 1 | 2 | 4 | 5);
        // Siege shares the GvG chip. A second "Siege" label would only repeat it.
        let gvg = bits.gvg || bits.siege || matches!(self.map_type, 7 | 8 | 18 | 25) || self.property_type == 3;
        let battleground = bits.battlefield || self.map_type == 19;
        let jail = self.map_type == 15;
        let no_skill = self.map_type == 13 || self.property_type == 6;

        let mut labels = Vec::new();
        if pvp {
            labels.push("PvP");
        }
        if gvg {
            labels.push("GvG");
        }
        if battleground {
            labels.push("BG");
        }
        if jail {
            labels.push("Jail");
        }
        if no_skill {
            labels.push("No skill");
        }
        self.badge = labels.join(" · ");
    }
}

/// Map name without a `.gat` suffix or trailing NUL padding.
pub fn strip_map_name(map_name: &str) -> &str {
    let map_name = map_name.split('\0').next().unwrap_or(map_name);
    map_name.strip_suffix(".gat").unwrap_or(map_name)
}

#[cfg(test)]
mod tests {
    use super::{MapFlagBits, MapProperties};

    fn bits(flags: u32) -> MapFlagBits {
        let mut properties = MapProperties::default();
        properties.apply_property(0x28, flags);
        properties.flag_bits()
    }

    #[test]
    fn each_hercules_flag_bit_decodes() {
        assert!(bits(1 << 0).pvp);
        assert!(bits(1 << 1).gvg);
        assert!(bits(1 << 2).siege);
        assert!(bits(1 << 3).use_simple_effect);
        assert!(bits(1 << 4).disable_lockon);
        assert!(bits(1 << 5).count_pk);
        assert!(bits(1 << 6).no_party_formation);
        assert!(bits(1 << 7).battlefield);
        assert!(bits(1 << 8).disable_costume);
        assert!(bits(1 << 9).use_cart);
        assert!(bits(1 << 10).sun_moon_star);
        // Spare bits, including anything a noteleport flag would have needed, do
        // nothing.
        let spare = bits(1 << 11);
        assert_eq!(spare, MapFlagBits::default());
    }

    #[test]
    fn badge_follows_flags_and_map_type() {
        let mut properties = MapProperties::default();
        properties.apply_property(0x28, 0);
        assert_eq!(properties.badge_text(), "");
        properties.apply_map_type(0);
        assert_eq!(properties.badge_text(), "");
        properties.apply_map_type(2);
        assert_eq!(properties.badge_text(), "");
        properties.apply_map_type(3);
        assert_eq!(properties.badge_text(), "");

        properties.apply_property(0x28, 1);
        assert_eq!(properties.badge_text(), "PvP");
        properties.apply_property(0x28, (1 << 1) | (1 << 2));
        assert_eq!(properties.badge_text(), "GvG");
        properties.apply_property(0x28, 1 << 2);
        assert_eq!(properties.badge_text(), "GvG");
        properties.apply_property(0x28, (1 << 0) | (1 << 1));
        assert_eq!(properties.badge_text(), "PvP · GvG");
        properties.apply_property(0x28, 1 << 7);
        assert_eq!(properties.badge_text(), "BG");
        properties.apply_property(0x28, 1 << 9);
        assert_eq!(properties.badge_text(), "");
        properties.apply_property(0x28, 1 << 11);
        assert_eq!(properties.badge_text(), "");

        properties.apply_property(1, 0);
        assert_eq!(properties.badge_text(), "PvP");
        properties.apply_property(3, 0);
        assert_eq!(properties.badge_text(), "GvG");
        properties.apply_property(6, 0);
        assert_eq!(properties.badge_text(), "No skill");
        properties.apply_property(0, 0);
        assert_eq!(properties.badge_text(), "");

        for map_type in [5_u16, 6, 11, 12, 17, 20] {
            properties.apply_map_type(map_type);
            assert_eq!(properties.badge_text(), "PvP", "map type {map_type}");
        }
        for map_type in [7_u16, 8, 18, 25] {
            properties.apply_map_type(map_type);
            assert_eq!(properties.badge_text(), "GvG", "map type {map_type}");
        }
        properties.apply_map_type(19);
        assert_eq!(properties.badge_text(), "BG");
        properties.apply_map_type(15);
        assert_eq!(properties.badge_text(), "Jail");
        properties.apply_map_type(13);
        assert_eq!(properties.badge_text(), "No skill");
    }

    #[test]
    fn map_change_clears_the_badge_and_matches_gat_names() {
        let mut properties = MapProperties::default();
        assert!(properties.accepts_map("anything.gat"));
        properties.apply_property(0x28, 1);
        properties.note_map("prontera.gat\0");
        assert_eq!(properties.map_name(), "prontera");
        assert_eq!(properties.badge_text(), "");
        assert!(properties.accepts_map("prontera.gat"));
        assert!(!properties.accepts_map("geffen.gat"));
    }
}
