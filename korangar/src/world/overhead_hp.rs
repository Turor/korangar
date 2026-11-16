//! Who gets an overhead HP bar, and which theme token fills it.
//!
//! Local player and party members use `status.hp_bar`. Monsters and other
//! attackable characters use the theme `danger` color. Anyone else has no bar.

use crate::graphics::Color;
use crate::world::EntityType;

/// Relationship used to pick an overhead HP bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverheadHpSubject {
    LocalPlayer,
    PartyMember,
    /// Monster, or a non-party character the local player can attack.
    Enemy,
    /// Another player who is neither in the party nor hostile.
    OtherPlayer,
}

/// Fill chosen for [`OverheadHpSubject`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverheadHpFill {
    /// `status.hp_bar`.
    Ally,
    /// Theme `danger`.
    Danger,
    /// No overhead HP bar.
    Hidden,
}

pub fn overhead_hp_fill(subject: OverheadHpSubject) -> OverheadHpFill {
    match subject {
        OverheadHpSubject::LocalPlayer | OverheadHpSubject::PartyMember => OverheadHpFill::Ally,
        OverheadHpSubject::Enemy => OverheadHpFill::Danger,
        OverheadHpSubject::OtherPlayer => OverheadHpFill::Hidden,
    }
}

/// `Some` when a bar is drawn. Ally is `status.hp_bar`; danger is the theme
/// danger token.
pub fn overhead_hp_color(subject: OverheadHpSubject, ally: Color, danger: Color) -> Option<Color> {
    match overhead_hp_fill(subject) {
        OverheadHpFill::Ally => Some(ally),
        OverheadHpFill::Danger => Some(danger),
        OverheadHpFill::Hidden => None,
    }
}

/// Monster, or a player with the appear-packet PK / attackable flag.
pub fn is_overhead_enemy(entity_type: EntityType, attackable: bool) -> bool {
    match entity_type {
        EntityType::Monster => true,
        EntityType::Player => attackable,
        EntityType::Hidden | EntityType::Npc | EntityType::Warp => false,
    }
}

pub fn classify_overhead(is_local: bool, in_party: bool, entity_type: EntityType, attackable: bool) -> OverheadHpSubject {
    if is_local {
        OverheadHpSubject::LocalPlayer
    } else if in_party && entity_type == EntityType::Player {
        OverheadHpSubject::PartyMember
    } else if is_overhead_enemy(entity_type, attackable) {
        OverheadHpSubject::Enemy
    } else {
        OverheadHpSubject::OtherPlayer
    }
}

/// Hover and attack targeting draw a bar only for enemies. Party members
/// already have the green bar. The local player draws full bars separately.
pub fn shows_focused_enemy_bar(entity_type: EntityType, attackable: bool, in_party: bool) -> bool {
    overhead_hp_fill(classify_overhead(false, in_party, entity_type, attackable)) == OverheadHpFill::Danger
}

#[cfg(test)]
mod tests {
    use super::{OverheadHpFill, OverheadHpSubject, classify_overhead, overhead_hp_color, overhead_hp_fill, shows_focused_enemy_bar};
    use crate::state::theme::{ThemeTokens, WorldTheme};
    use crate::world::EntityType;

    fn shipped() -> Vec<ThemeTokens> {
        vec![
            ThemeTokens::verdant_rune(),
            ThemeTokens::ember_forge(),
            ThemeTokens::arcane_violet(),
            ThemeTokens::turoran_dark(),
        ]
    }

    #[test]
    fn four_overhead_cases_use_hp_bar_danger_or_nothing() {
        for tokens in shipped() {
            let ally = overhead_hp_color(OverheadHpSubject::LocalPlayer, tokens.status_hp_bar, tokens.danger);
            let party = overhead_hp_color(OverheadHpSubject::PartyMember, tokens.status_hp_bar, tokens.danger);
            let enemy = overhead_hp_color(OverheadHpSubject::Enemy, tokens.status_hp_bar, tokens.danger);
            let other = overhead_hp_color(OverheadHpSubject::OtherPlayer, tokens.status_hp_bar, tokens.danger);

            assert_eq!(ally, Some(tokens.status_hp_bar), "{}", tokens.name);
            assert_eq!(party, Some(tokens.status_hp_bar), "{}", tokens.name);
            assert_eq!(enemy, Some(tokens.danger), "{}", tokens.name);
            assert_eq!(other, None, "{}", tokens.name);
            assert_ne!(tokens.danger, tokens.status_hp_bar, "{}", tokens.name);

            let world = WorldTheme::from_tokens(&tokens);
            assert_eq!(world.status_bar.player_health_color, tokens.status_hp_bar, "{}", tokens.name);
            assert_eq!(world.status_bar.enemy_health_color, tokens.danger, "{}", tokens.name);
        }
    }

    #[test]
    fn classification_matches_the_four_cases() {
        assert_eq!(
            overhead_hp_fill(classify_overhead(true, false, EntityType::Player, false)),
            OverheadHpFill::Ally
        );
        assert_eq!(
            overhead_hp_fill(classify_overhead(false, true, EntityType::Player, false)),
            OverheadHpFill::Ally
        );
        // A party member stays green even if the map marks them attackable.
        assert_eq!(
            overhead_hp_fill(classify_overhead(false, true, EntityType::Player, true)),
            OverheadHpFill::Ally
        );
        assert_eq!(
            overhead_hp_fill(classify_overhead(false, false, EntityType::Monster, false)),
            OverheadHpFill::Danger
        );
        assert_eq!(
            overhead_hp_fill(classify_overhead(false, false, EntityType::Player, true)),
            OverheadHpFill::Danger
        );
        assert_eq!(
            overhead_hp_fill(classify_overhead(false, false, EntityType::Player, false)),
            OverheadHpFill::Hidden
        );
        assert_eq!(
            overhead_hp_fill(classify_overhead(false, false, EntityType::Npc, false)),
            OverheadHpFill::Hidden
        );

        assert!(!shows_focused_enemy_bar(EntityType::Player, false, false));
        assert!(!shows_focused_enemy_bar(EntityType::Player, true, true));
        assert!(shows_focused_enemy_bar(EntityType::Player, true, false));
        assert!(shows_focused_enemy_bar(EntityType::Monster, false, false));
        assert!(!shows_focused_enemy_bar(EntityType::Npc, false, false));
    }

    #[test]
    fn shipped_danger_is_red_except_ember_orange() {
        let verdant = ThemeTokens::verdant_rune();
        let arcane = ThemeTokens::arcane_violet();
        let dark = ThemeTokens::turoran_dark();
        let ember = ThemeTokens::ember_forge();
        for tokens in [&verdant, &arcane, &dark] {
            assert!(is_red(tokens.danger), "{} danger should be red", tokens.name);
        }
        assert!(is_orange(ember.danger), "ember danger should be orange");
        assert!(!is_red(ember.danger));
    }

    fn is_red(color: crate::graphics::Color) -> bool {
        let (red, green, blue) = (color.red_as_u8(), color.green_as_u8(), color.blue_as_u8());
        // Both other channels stay low and close. Orange keeps a higher green.
        red > 160 && u16::from(green) * 2 < u16::from(red) && u16::from(blue) * 2 < u16::from(red) && green.abs_diff(blue) < 40
    }

    fn is_orange(color: crate::graphics::Color) -> bool {
        let (red, green, blue) = (color.red_as_u8(), color.green_as_u8(), color.blue_as_u8());
        red > green && green > blue.saturating_add(30) && green > 80 && red > 180
    }
}
