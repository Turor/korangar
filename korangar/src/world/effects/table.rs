//! Lookups over the roBrowser effect, skill, and skill-unit tables.
//!
//! STR files are the kind [`crate::loaders::EffectLoader`] can play. Sprite
//! rows are kept in the table and skipped at playback, same as cylinder, 3D,
//! and function effects.

use ragnarok_bytes::{ByteWriter, ToBytes};
use ragnarok_packets::{EffectId, UnitId};

use super::generated::{EFFECT_REFS, LAYERS, NAMED_EFFECTS, NUMERIC_EFFECTS, SKILL_UNITS, SKILL_VISUALS};
pub use super::generated::{EffectRef, Layer, LayerKind, SkillVisual};

/// Which skill-effect slot to read. Names match roBrowser's `SkillEffect.js`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillField {
    Effect,
    EffectOnCaster,
    /// Zone STR from the roBrowser table. Persistent zones are `AddSkillUnit`,
    /// so the client does not construct this slot at playback. Tests still read
    /// it.
    #[cfg_attr(not(test), allow(dead_code))]
    Ground,
    Hit,
    HitOnCaster,
    BeforeHit,
    BeforeHitOnCaster,
    BeginCast,
    Success,
    SuccessOnCaster,
}

fn layers_at(start: u16, count: u16) -> &'static [Layer] {
    let start = usize::from(start);
    let count = usize::from(count);
    &LAYERS[start..start + count]
}

fn refs_at(start: u16, count: u16) -> &'static [EffectRef] {
    let start = usize::from(start);
    let count = usize::from(count);
    &EFFECT_REFS[start..start + count]
}

pub fn numeric_layers(effect_id: u32) -> Option<&'static [Layer]> {
    let index = NUMERIC_EFFECTS.binary_search_by_key(&effect_id, |row| row.0).ok()?;
    let (_, start, count) = NUMERIC_EFFECTS[index];
    Some(layers_at(start, count))
}

pub fn named_layers(name: &str) -> Option<&'static [Layer]> {
    let index = NAMED_EFFECTS.binary_search_by_key(&name, |row| row.0).ok()?;
    let (_, start, count) = NAMED_EFFECTS[index];
    Some(layers_at(start, count))
}

pub fn skill_visual(skill_id: u16) -> Option<&'static SkillVisual> {
    let index = SKILL_VISUALS.binary_search_by_key(&skill_id, |visual| visual.skill_id).ok()?;
    Some(&SKILL_VISUALS[index])
}

pub fn skill_effect_refs(skill_id: u16, field: SkillField) -> &'static [EffectRef] {
    let Some(visual) = skill_visual(skill_id) else {
        return &[];
    };
    let (start, count) = match field {
        SkillField::Effect => visual.effect,
        SkillField::EffectOnCaster => visual.effect_on_caster,
        SkillField::Ground => visual.ground,
        SkillField::Hit => visual.hit,
        SkillField::HitOnCaster => visual.hit_on_caster,
        SkillField::BeforeHit => visual.before_hit,
        SkillField::BeforeHitOnCaster => visual.before_hit_on_caster,
        SkillField::BeginCast => visual.begin_cast,
        SkillField::Success => visual.success,
        SkillField::SuccessOnCaster => visual.success_on_caster,
    };
    refs_at(start, count)
}

pub fn unit_effect(unit_id: u32) -> Option<EffectRef> {
    let index = SKILL_UNITS.binary_search_by_key(&unit_id, |row| row.0).ok()?;
    Some(SKILL_UNITS[index].1)
}

pub fn effect_id_value(effect_id: EffectId) -> u32 {
    numeric_u32(&effect_id)
}

pub fn unit_id_value(unit_id: &UnitId) -> u32 {
    numeric_u32(unit_id)
}

fn numeric_u32(value: &impl ToBytes) -> u32 {
    let mut writer = ByteWriter::new();
    if value.to_bytes(&mut writer).is_err() {
        return 0;
    }
    let bytes = writer.into_inner();
    let mut padded = [0; 4];
    let count = bytes.len().min(4);
    padded[..count].copy_from_slice(&bytes[..count]);
    u32::from_le_bytes(padded)
}

/// STR paths the effect loader can play for these references.
///
/// Sprite, cylinder, 3D, and unknown rows are skipped. A debug build logs that
/// skip; nothing is written to chat.
pub fn collect_str_paths(effect_refs: &[EffectRef]) -> Vec<&'static str> {
    let mut paths = Vec::new();
    for effect_ref in effect_refs {
        let (layers, label) = match *effect_ref {
            EffectRef::Id(id) => (numeric_layers(id), format!("effect {id}")),
            EffectRef::Name(name) => (named_layers(name), name.to_string()),
        };
        let Some(layers) = layers else {
            note_unsupported(&label);
            continue;
        };
        let mut played = false;
        for layer in layers {
            match layer.kind {
                LayerKind::Str if !layer.file.is_empty() => {
                    paths.push(layer.file);
                    played = true;
                }
                LayerKind::Sprite | LayerKind::Other | LayerKind::Str => {}
            }
        }
        if !played {
            note_unsupported(&label);
        }
    }
    paths
}

/// Special-effect handling never asks for a chat line.
///
/// Unmapped ids used to be pushed as `Effect: ...`. That string is gone.
pub fn chat_line_for_special_effect(_effect_id: u32) -> Option<&'static str> {
    None
}

#[cfg(feature = "debug")]
fn note_unsupported(label: &str) {
    use korangar_debug::logging::Colorize;
    korangar_debug::logging::print_debug!("[{}] skipped unsupported skill effect {}", "effect".magenta(), label);
}

#[cfg(not(feature = "debug"))]
fn note_unsupported(_label: &str) {}

#[cfg(test)]
mod tests {
    use ragnarok_packets::EffectId;

    use super::*;

    fn field(skill_id: u16, slot: SkillField) -> Vec<EffectRef> {
        skill_effect_refs(skill_id, slot).to_vec()
    }

    #[test]
    fn twenty_skills_match_the_robrowser_tables() {
        assert_eq!(field(5, SkillField::BeginCast), vec![EffectRef::Id(16)]);
        assert_eq!(field(5, SkillField::Hit), vec![EffectRef::Id(1)]);
        assert_eq!(field(6, SkillField::Success), vec![EffectRef::Id(67)]);
        assert_eq!(field(7, SkillField::EffectOnCaster), vec![EffectRef::Id(17)]);
        assert_eq!(field(7, SkillField::Effect), vec![EffectRef::Name("quake_magnum")]);
        assert_eq!(field(8, SkillField::Effect), vec![EffectRef::Id(11)]);
        assert_eq!(field(14, SkillField::BeforeHit), vec![EffectRef::Name("ef_coldbolt")]);
        assert_eq!(field(14, SkillField::Hit), vec![EffectRef::Id(51)]);
        assert_eq!(field(18, SkillField::Hit), vec![EffectRef::Id(49)]);
        assert_eq!(field(18, SkillField::Ground), vec![EffectRef::Id(25)]);
        assert_eq!(field(25, SkillField::Ground), vec![EffectRef::Id(141)]);
        assert_eq!(field(28, SkillField::Effect), vec![EffectRef::Id(312)]);
        assert_eq!(field(28, SkillField::Hit), vec![EffectRef::Id(320)]);
        assert_eq!(field(46, SkillField::BeginCast), vec![EffectRef::Id(16)]);
        assert_eq!(field(46, SkillField::BeforeHit), vec![EffectRef::Name("ef_arrow_projectile")]);
        assert_eq!(field(46, SkillField::Hit), vec![EffectRef::Id(1)]);
        assert_eq!(field(50, SkillField::Success), vec![EffectRef::Id(18)]);
        assert_eq!(field(54, SkillField::Effect), vec![EffectRef::Id(77), EffectRef::Id(140)]);
        assert!(skill_visual(57).is_some_and(|visual| visual.hide_cast_bar && visual.hide_cast_aura));
        assert_eq!(field(57, SkillField::Effect), vec![EffectRef::Id(70)]);
        assert_eq!(field(57, SkillField::EffectOnCaster), vec![EffectRef::Id(144)]);
        assert!(skill_visual(61).is_some_and(|visual| visual.hide_cast_aura && !visual.hide_cast_bar));
        assert_eq!(field(70, SkillField::Effect), vec![EffectRef::Id(83)]);
        assert_eq!(field(70, SkillField::Ground), vec![EffectRef::Id(319)]);
        assert_eq!(field(83, SkillField::Effect), vec![EffectRef::Id(92)]);
        assert_eq!(field(83, SkillField::Hit), vec![EffectRef::Id(49)]);
        assert_eq!(field(708, SkillField::Effect), field(2213, SkillField::Effect));
        assert_eq!(field(708, SkillField::Effect), vec![EffectRef::Name("ef_wl_comet")]);
        assert!(skill_visual(739).is_some());
        assert!(field(739, SkillField::Effect).is_empty());
        assert_eq!(field(752, SkillField::Effect), vec![EffectRef::Name("ef_npc_gradual_gravity")]);
        assert_eq!(field(783, SkillField::Hit), vec![EffectRef::Name("ef_npc_killing_aura_hit")]);
        assert_eq!(field(784, SkillField::Effect), vec![EffectRef::Id(508)]);
    }

    #[test]
    fn str_sprite_and_unit_rows_resolve() {
        let coin = numeric_layers(effect_id_value(EffectId::Coin)).expect("coin effect");
        assert!(coin.iter().any(|layer| layer.kind == LayerKind::Str && layer.file == "maemor.str"));

        let firewall = numeric_layers(25).expect("firewall");
        assert!(
            firewall
                .iter()
                .any(|layer| layer.file.starts_with("firewall") && layer.kind == LayerKind::Str)
        );

        // EffectTable.js comments out 157 (`EF_LVUP`) because `levelup.str` is missing.
        // The level-up STR is effect 371 (`EF_ANGEL`, `angel.str`).
        assert!(numeric_layers(effect_id_value(EffectId::Lvup)).is_none());
        let angel = numeric_layers(371).expect("angel");
        assert!(angel.iter().any(|layer| layer.file == "angel.str"));

        assert!(numeric_layers(1884).is_some(), "Turoran card heal effect");
        assert!(named_layers("ef_coldbolt").is_some());
        assert!(named_layers("ef_cloud_kill").is_some());

        assert_eq!(unit_effect(127), Some(EffectRef::Id(25)));
        assert_eq!(unit_effect(133), Some(EffectRef::Id(141)));
        assert_eq!(unit_effect(224), Some(EffectRef::Name("ef_cloud_kill")));

        let paths = collect_str_paths(&[EffectRef::Id(10), EffectRef::Name("ef_coldbolt"), EffectRef::Id(0)]);
        assert!(paths.contains(&"maemor.str"));
        assert!(paths.iter().all(|path| path.ends_with(".str")));
    }

    #[test]
    fn special_effect_handling_does_not_push_chat() {
        for effect_id in [0, 1, 10, 25, 312, 1884, 999_999] {
            assert!(chat_line_for_special_effect(effect_id).is_none());
        }
        assert!(chat_line_for_special_effect(effect_id_value(EffectId::Coin)).is_none());
        assert!(chat_line_for_special_effect(effect_id_value(EffectId::Heal)).is_none());
    }
}
