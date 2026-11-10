//! Pocket zeny, experience, weight, and 4th-job trait stats.
//!
//! Values come from `StatType` on `ZC_PAR_CHANGE`, `ZC_LONGLONGPAR_CHANGE`,
//! `ZC_COUPLESTATUS`, and `ZC_STATUS_CHANGE`. Weight is stored in the server's
//! 0.1 unit and divided by 10 for display, matching roBrowser BasicInfo.

use korangar_interface::element::StateElement;
use ragnarok_packets::StatType;
use rust_state::RustState;

/// How full the weight limit is. 50% is caution, 90% is overweight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightPressure {
    Normal,
    Caution,
    Overweight,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct BasicInfoStats {
    pub zeny: u64,
    pub base_experience: u64,
    pub job_experience: u64,
    pub next_base_experience: u64,
    pub next_job_experience: u64,
    pub weight: u32,
    pub maximum_weight: u32,
    pub trait_points: u32,
    pub power: i32,
    pub bonus_power: i32,
    pub power_stat_points_cost: u8,
    pub stamina: i32,
    pub bonus_stamina: i32,
    pub stamina_stat_points_cost: u8,
    pub wisdom: i32,
    pub bonus_wisdom: i32,
    pub wisdom_stat_points_cost: u8,
    pub spell: i32,
    pub bonus_spell: i32,
    pub spell_stat_points_cost: u8,
    pub concentration: i32,
    pub bonus_concentration: i32,
    pub concentration_stat_points_cost: u8,
    pub creativity: i32,
    pub bonus_creativity: i32,
    pub creativity_stat_points_cost: u8,
    pub physical_attack: u32,
    pub spell_magic_attack: u32,
    pub resistance: u32,
    pub magic_resistance: u32,
    pub healing_plus: u32,
    pub critical_damage_rate: u32,
}

impl BasicInfoStats {
    /// Apply one basic-info or trait stat. Returns false when `stat_type` is
    /// a combat or primary stat handled on [`crate::world::Player`] itself.
    pub fn apply(&mut self, stat_type: StatType) -> bool {
        match stat_type {
            StatType::Zeny(value) => self.zeny = u64::from(value),
            StatType::BaseExperience(value) => self.base_experience = value,
            StatType::JobExperience(value) => self.job_experience = value,
            StatType::NextBaseExperience(value) => self.next_base_experience = value,
            StatType::NextJobExperience(value) => self.next_job_experience = value,
            StatType::Weight(value) => self.weight = value,
            StatType::MaximumWeight(value) => self.maximum_weight = value,
            StatType::TraitPoint(value) => self.trait_points = value,
            StatType::Power(base, bonus) => {
                self.power = base as i32;
                self.bonus_power = bonus as i32;
            }
            StatType::Stamina(base, bonus) => {
                self.stamina = base as i32;
                self.bonus_stamina = bonus as i32;
            }
            StatType::Wisdom(base, bonus) => {
                self.wisdom = base as i32;
                self.bonus_wisdom = bonus as i32;
            }
            StatType::Spell(base, bonus) => {
                self.spell = base as i32;
                self.bonus_spell = bonus as i32;
            }
            StatType::Concentration(base, bonus) => {
                self.concentration = base as i32;
                self.bonus_concentration = bonus as i32;
            }
            StatType::Creativity(base, bonus) => {
                self.creativity = base as i32;
                self.bonus_creativity = bonus as i32;
            }
            StatType::PowerStatPointCost(cost) => self.power_stat_points_cost = cost,
            StatType::StaminaStatPointCost(cost) => self.stamina_stat_points_cost = cost,
            StatType::WisdomStatPointCost(cost) => self.wisdom_stat_points_cost = cost,
            StatType::SpellStatPointCost(cost) => self.spell_stat_points_cost = cost,
            StatType::ConcentrationStatPointCost(cost) => self.concentration_stat_points_cost = cost,
            StatType::CreativitySpellPointCost(cost) => self.creativity_stat_points_cost = cost,
            StatType::PhysicalAttack(value) => self.physical_attack = value,
            StatType::SpellMagicAttack(value) => self.spell_magic_attack = value,
            StatType::Resistance(value) => self.resistance = value,
            StatType::MagicResistance(value) => self.magic_resistance = value,
            StatType::HealingPlus(value) => self.healing_plus = value,
            StatType::CriticalDamageRate(value) => self.critical_damage_rate = value,
            _ => return false,
        }
        true
    }
}

pub fn weight_pressure(current: u32, maximum: u32) -> WeightPressure {
    if maximum == 0 {
        return WeightPressure::Normal;
    }
    let scaled = u64::from(current).saturating_mul(100);
    let limit = u64::from(maximum);
    if scaled >= limit.saturating_mul(90) {
        WeightPressure::Overweight
    } else if scaled >= limit.saturating_mul(50) {
        WeightPressure::Caution
    } else {
        WeightPressure::Normal
    }
}

/// roBrowser prints weight divided by 10.
pub fn format_weight(current: u32, maximum: u32) -> String {
    format!("Weight {} / {}", current / 10, maximum / 10)
}

pub fn format_zeny(zeny: u64) -> String {
    format!("Zeny {}", group_digits(zeny))
}

pub fn group_digits(value: u64) -> String {
    let digits = value.to_string();
    let mut reversed = String::new();
    for (index, character) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            reversed.push(',');
        }
        reversed.push(character);
    }
    reversed.chars().rev().collect()
}

/// One-decimal percent, capped at 100.0, matching roBrowser `bexp`/`jexp`.
pub fn format_exp_percent(current: u64, next: u64) -> String {
    if next == 0 {
        return "0.0%".to_owned();
    }
    let tenths = (current.saturating_mul(1000) / next).min(1000);
    format!("{}.{}%", tenths / 10, tenths % 10)
}

pub fn format_exp_pair(current: u64, next: u64) -> String {
    format!("{} / {}", group_digits(current), group_digits(next))
}

/// Fill ratio of an experience bar. Uses the floored percent roBrowser assigns
/// to the bar width.
pub fn exp_bar_ratio(current: u64, next: u64) -> f32 {
    if next == 0 {
        return 0.0;
    }
    let percent = (current.saturating_mul(100) / next).min(100);
    percent as f32 / 100.0
}

#[allow(dead_code)]
/// Renewal hard-armor reduction from roBrowser `WinStatsCommon`.
///
/// DEF: `value / (value + 400) * 90`. MDEF: `value / (value + 100) * 90`.
/// Soft DEF/MDEF subtract after this percent and are not included.
pub fn renewal_armor_reduction_percent(value: u32, magical: bool) -> f64 {
    if value == 0 {
        return 0.0;
    }
    let amount = f64::from(value);
    let scale = if magical { 100.0 } else { 400.0 };
    amount / (amount + scale) * 90.0
}

#[allow(dead_code)]
pub fn format_armor_reduction(label: &str, value: u32, magical: bool) -> String {
    let percent = renewal_armor_reduction_percent(value, magical);
    format!("{label}: {percent:.1}%. Soft DEF/MDEF subtracts after")
}

pub fn stat_up_feedback(success: bool) -> Option<&'static str> {
    if success { None } else { Some("Could not raise that stat.") }
}

#[cfg(test)]
mod tests {
    use ragnarok_bytes::ByteReader;
    use ragnarok_packets::{PacketExt, StatType, UpdateStatPacket1, UpdateStatPacket2};

    use super::*;

    #[test]
    fn applies_64_bit_experience_zeny_weight_and_traits() {
        let exp = 9_000_000_000_000u64;
        let mut exp_bytes = vec![0xCB, 0x0A, 0x01, 0x00];
        exp_bytes.extend_from_slice(&exp.to_le_bytes());
        let mut reader = ByteReader::without_metadata(&exp_bytes);
        let packet = UpdateStatPacket2::packet_from_bytes(&mut reader).unwrap();

        let mut stats = BasicInfoStats::default();
        assert!(stats.apply(packet.stat_type));
        assert_eq!(stats.base_experience, exp);

        assert!(stats.apply(StatType::Zeny(1_234_567)));
        assert_eq!(stats.zeny, 1_234_567);

        assert!(stats.apply(StatType::Weight(1500)));
        assert!(stats.apply(StatType::MaximumWeight(2000)));
        assert_eq!(format_weight(stats.weight, stats.maximum_weight), "Weight 150 / 200");
        // 1500/2000 is 75%: caution starts at 50%, overweight at 90%.
        assert_eq!(weight_pressure(stats.weight, stats.maximum_weight), WeightPressure::Caution);

        let bytes = [0x41, 0x01, 0xDB, 0x00, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00];
        let mut reader = ByteReader::without_metadata(&bytes);
        let packet = UpdateStatPacket1::packet_from_bytes(&mut reader).unwrap();
        assert!(stats.apply(packet.stat_type));
        assert_eq!((stats.power, stats.bonus_power), (10, 2));

        assert!(stats.apply(StatType::TraitPoint(4)));
        assert!(stats.apply(StatType::PowerStatPointCost(1)));
        assert!(stats.apply(StatType::PhysicalAttack(30)));
        assert!(stats.apply(StatType::SpellMagicAttack(12)));
        assert!(stats.apply(StatType::Resistance(5)));
        assert!(stats.apply(StatType::MagicResistance(6)));
        assert!(stats.apply(StatType::HealingPlus(7)));
        assert!(stats.apply(StatType::CriticalDamageRate(8)));
        assert!(!stats.apply(StatType::Hit(9)));
        assert_eq!(stats.trait_points, 4);
        assert_eq!(stats.physical_attack, 30);
        assert_eq!(stats.critical_damage_rate, 8);
    }

    #[test]
    fn weight_bands_and_exp_bar_follow_robrowser() {
        assert_eq!(weight_pressure(49, 100), WeightPressure::Normal);
        assert_eq!(weight_pressure(50, 100), WeightPressure::Caution);
        assert_eq!(weight_pressure(89, 100), WeightPressure::Caution);
        assert_eq!(weight_pressure(90, 100), WeightPressure::Overweight);
        assert_eq!(format_exp_percent(1, 1000), "0.1%");
        assert_eq!(format_exp_percent(1000, 1000), "100.0%");
        assert_eq!(format_exp_percent(5, 0), "0.0%");
        assert_eq!(exp_bar_ratio(250, 1000), 0.25);
        assert_eq!(format_zeny(1234567), "Zeny 1,234,567");
        assert_eq!(format_exp_pair(1234, 10000), "1,234 / 10,000");
        assert_eq!(
            format_armor_reduction("DEF", 400, false),
            "DEF: 45.0%. Soft DEF/MDEF subtracts after"
        );
        assert!((renewal_armor_reduction_percent(400, false) - 45.0).abs() < 0.001);
        assert!((renewal_armor_reduction_percent(100, true) - 45.0).abs() < 0.001);
        assert_eq!(renewal_armor_reduction_percent(0, false), 0.0);
        assert_eq!(stat_up_feedback(true), None);
        assert_eq!(stat_up_feedback(false), Some("Could not raise that stat."));
    }
}
