//! Homunculus, pet, mercenary, and elemental info for PACKETVER 20250716 MAIN.
//!
//! Layouts match the Hercules structs selected by that packet version:
//! `ZC_PROPERTY_PET` `0x01A2`, `ZC_CHANGESTATE_PET` `0x01A4`,
//! `ZC_PROPERTY_HOMUN` `0x0BA4`, `ZC_MER_INIT` `0x029B`, `ZC_MER_PROPERTY`
//! `0x029C`, `ZC_MER_PAR_CHANGE` `0x02A2`, `ZC_EL_INIT` `0x081D`, and
//! `ZC_EL_PAR_CHANGE` `0x081E`.

use ragnarok_packets::handler::{DuplicateHandlerError, PacketCallback, PacketHandler};
use ragnarok_packets::{
    ElementalInitPacket, ElementalParameterPacket, HomunculusPropertyPacket, MercenaryInitPacket, MercenaryParameterPacket,
    MercenaryPropertyPacket, PetPropertyPacket, PetStatePacket,
};

use crate::event::{NetworkEvent, NetworkEventList};

pub const PET_ACT_FEED: u8 = 1;
pub const PET_ACT_PERFORMANCE: u8 = 2;
pub const PET_ACT_RETURN: u8 = 3;
pub const PET_ACT_UNEQUIP: u8 = 4;

pub const HOM_ACT_FEED: u16 = 1;
pub const HOM_ACT_DELETE: u16 = 2;

pub const MERC_ACT_DISMISS: u8 = 1;

pub const MERC_VAR_ATTACK: u16 = 0;
pub const MERC_VAR_MAGIC_ATTACK: u16 = 1;
pub const MERC_VAR_HIT: u16 = 2;
pub const MERC_VAR_CRITICAL: u16 = 3;
pub const MERC_VAR_DEFENSE: u16 = 4;
pub const MERC_VAR_MAGIC_DEFENSE: u16 = 5;
pub const MERC_VAR_FLEE: u16 = 6;
pub const MERC_VAR_ATTACK_SPEED: u16 = 7;
pub const MERC_VAR_HP: u16 = 8;
pub const MERC_VAR_MAX_HP: u16 = 9;
pub const MERC_VAR_SP: u16 = 10;
pub const MERC_VAR_MAX_SP: u16 = 11;
pub const MERC_VAR_KILLS: u16 = 12;
pub const MERC_VAR_FAITH: u16 = 13;

pub const EL_VAR_HP: u16 = 0;
pub const EL_VAR_MAX_HP: u16 = 1;
pub const EL_VAR_SP: u16 = 2;
pub const EL_VAR_MAX_SP: u16 = 3;

pub const PET_KIND_INTIMACY: i8 = 1;
pub const PET_KIND_HUNGER: i8 = 2;
pub const PET_KIND_ACCESSORY: i8 = 3;

pub fn register_companion_packets<Callback>(
    packet_handler: &mut PacketHandler<NetworkEventList, Callback>,
) -> Result<(), DuplicateHandlerError>
where
    Callback: PacketCallback,
{
    packet_handler.register(|packet: PetPropertyPacket| NetworkEvent::PetInfo {
        summary: format_pet(
            &packet.name,
            packet.renamed != 0,
            packet.level,
            packet.hunger,
            packet.intimacy,
            packet.accessory_id,
            packet.class_id,
        ),
        name: packet.name,
        renamed: packet.renamed != 0,
        level: packet.level,
        hunger: packet.hunger,
        intimacy: packet.intimacy,
        accessory_id: packet.accessory_id,
        class_id: packet.class_id,
    })?;
    packet_handler.register(|packet: PetStatePacket| NetworkEvent::PetState {
        kind: packet.kind,
        entity_id: packet.entity_id,
        value: packet.value,
    })?;
    packet_handler.register(|packet: HomunculusPropertyPacket| NetworkEvent::HomunculusInfo {
        summary: format_homunculus(&packet),
        name: packet.name,
        flags: packet.flags,
        level: packet.level,
        hunger: packet.hunger,
        intimacy: packet.intimacy,
        attack: packet.attack,
        magic_attack: packet.magic_attack,
        hit: packet.hit,
        critical: packet.critical,
        defense: packet.defense,
        magic_defense: packet.magic_defense,
        flee: packet.flee,
        attack_motion: packet.attack_motion,
        health_points: packet.health_points,
        maximum_health_points: packet.maximum_health_points,
        spell_points: packet.spell_points,
        maximum_spell_points: packet.maximum_spell_points,
        experience: packet.experience,
        next_experience: packet.next_experience,
        skill_points: packet.skill_points,
        attack_range: packet.attack_range,
    })?;
    packet_handler.register(|packet: MercenaryInitPacket| NetworkEvent::MercenaryInfo {
        summary: format_mercenary_init(&packet),
        account_id: packet.account_id,
        name: packet.name,
        level: packet.level,
        attack: packet.attack,
        magic_attack: packet.magic_attack,
        hit: packet.hit,
        critical: packet.critical,
        defense: packet.defense,
        magic_defense: packet.magic_defense,
        flee: packet.flee,
        attack_speed: packet.attack_speed,
        health_points: packet.health_points,
        maximum_health_points: packet.maximum_health_points,
        spell_points: packet.spell_points,
        maximum_spell_points: packet.maximum_spell_points,
        expire_date: packet.expire_date,
        faith: packet.faith,
        summons: packet.summons,
        kills: packet.kills,
        attack_range: packet.attack_range,
    })?;
    packet_handler.register(|packet: MercenaryPropertyPacket| NetworkEvent::MercenaryInfo {
        summary: format_mercenary_property(&packet),
        account_id: 0,
        name: packet.name,
        level: packet.level,
        attack: packet.attack,
        magic_attack: packet.magic_attack,
        hit: packet.hit,
        critical: packet.critical,
        defense: packet.defense,
        magic_defense: packet.magic_defense,
        flee: packet.flee,
        attack_speed: packet.attack_speed,
        health_points: i32::from(packet.health_points),
        maximum_health_points: i32::from(packet.maximum_health_points),
        spell_points: i32::from(packet.spell_points),
        maximum_spell_points: i32::from(packet.maximum_spell_points),
        expire_date: packet.expire_date,
        faith: packet.faith,
        summons: packet.summons,
        kills: packet.kills,
        attack_range: 0,
    })?;
    packet_handler.register(|packet: MercenaryParameterPacket| NetworkEvent::MercenaryParameter {
        variable: packet.variable,
        value: packet.value,
    })?;
    packet_handler.register(|packet: ElementalInitPacket| NetworkEvent::ElementalInfo {
        summary: format_elemental(
            packet.class_id,
            packet.health_points,
            packet.maximum_health_points,
            packet.spell_points,
            packet.maximum_spell_points,
        ),
        class_id: packet.class_id,
        health_points: packet.health_points,
        maximum_health_points: packet.maximum_health_points,
        spell_points: packet.spell_points,
        maximum_spell_points: packet.maximum_spell_points,
    })?;
    packet_handler.register(|packet: ElementalParameterPacket| NetworkEvent::ElementalParameter {
        variable: packet.variable,
        value: packet.value,
    })?;
    Ok(())
}

pub fn intimacy_label(intimacy: i32) -> &'static str {
    match intimacy {
        ..100 => "Awkward",
        100..250 => "Shy",
        250..750 => "Neutral",
        750..910 => "Cordial",
        910..1000 => "Loyal",
        _ => "Devoted",
    }
}

pub fn format_pet(name: &str, renamed: bool, level: i16, hunger: i16, intimacy: i16, accessory: i16, class_id: i16) -> String {
    let name = if name.is_empty() { "Pet" } else { name };
    let renamed = if renamed { "renamed" } else { "not renamed" };
    let accessory = if accessory == 0 {
        "no accessory".to_owned()
    } else {
        format!("accessory {accessory}")
    };
    format!(
        "{name} class {class_id} Lv {level} ({renamed}). Hunger {hunger}. Intimacy {intimacy} ({}). {accessory}.",
        intimacy_label(i32::from(intimacy))
    )
}

pub fn format_homunculus_flags(flags: u8) -> String {
    let renamed = if flags & 0x01 != 0 { "renamed" } else { "not renamed" };
    let rest = if flags & 0x02 != 0 { "resting" } else { "out" };
    let life = if flags & 0x04 != 0 { "alive" } else { "down" };
    format!("{renamed}, {rest}, {life}")
}

pub fn format_homunculus(packet: &HomunculusPropertyPacket) -> String {
    let name = if packet.name.is_empty() { "Homunculus" } else { packet.name.as_str() };
    format!(
        "{name} Lv {} ({}). Hunger {}. Intimacy {} ({}). HP {}/{} SP {}/{}. ATK {} MATK {} HIT {} CRI {} DEF {} MDEF {} FLEE {} ASPD {} range {}. EXP {} / {}. Skill points {}.",
        packet.level,
        format_homunculus_flags(packet.flags),
        packet.hunger,
        packet.intimacy,
        intimacy_label(i32::from(packet.intimacy)),
        packet.health_points,
        packet.maximum_health_points,
        packet.spell_points,
        packet.maximum_spell_points,
        packet.attack,
        packet.magic_attack,
        packet.hit,
        packet.critical,
        packet.defense,
        packet.magic_defense,
        packet.flee,
        packet.attack_motion,
        packet.attack_range,
        packet.experience,
        packet.next_experience,
        packet.skill_points,
    )
}

pub fn elemental_name(class_id: u32) -> &'static str {
    match class_id {
        2114..=2116 | 20813 => "Agni",
        2117..=2119 | 20814 => "Aqua",
        2120..=2122 | 20815 => "Ventus",
        2123..=2125 | 20816 => "Tera",
        20817 => "Diluvio",
        20818 => "Procella",
        20819 => "Terremotus",
        20820 => "Serpens",
        _ => "Elemental",
    }
}

pub fn format_elemental(class_id: u32, hp: u32, max_hp: u32, sp: u32, max_sp: u32) -> String {
    format!(
        "{} (class {class_id}). HP {hp}/{max_hp}. SP {sp}/{max_sp}.",
        elemental_name(class_id)
    )
}

pub fn format_mercenary_init(packet: &MercenaryInitPacket) -> String {
    let name = if packet.name.is_empty() { "Mercenary" } else { packet.name.as_str() };
    format!(
        "{name} Lv {level}. HP {hp}/{max_hp} SP {sp}/{max_sp}. ATK {attack} MATK {magic_attack} HIT {hit} CRI {critical} DEF {defense} MDEF {magic_defense} FLEE {flee} ASPD {attack_speed} range {attack_range}. Faith {faith}. Summons {summons}. Kills {kills}. Contract until {expire}.",
        level = packet.level,
        hp = packet.health_points,
        max_hp = packet.maximum_health_points,
        sp = packet.spell_points,
        max_sp = packet.maximum_spell_points,
        attack = packet.attack,
        magic_attack = packet.magic_attack,
        hit = packet.hit,
        critical = packet.critical,
        defense = packet.defense,
        magic_defense = packet.magic_defense,
        flee = packet.flee,
        attack_speed = packet.attack_speed,
        attack_range = packet.attack_range,
        faith = packet.faith,
        summons = packet.summons,
        kills = packet.kills,
        expire = packet.expire_date,
    )
}

pub fn format_mercenary_property(packet: &MercenaryPropertyPacket) -> String {
    let name = if packet.name.is_empty() { "Mercenary" } else { packet.name.as_str() };
    format!(
        "{name} Lv {level}. HP {hp}/{max_hp} SP {sp}/{max_sp}. ATK {attack} MATK {magic_attack} HIT {hit} CRI {critical} DEF {defense} MDEF {magic_defense} FLEE {flee} ASPD {attack_speed}. Faith {faith}. Summons {summons}. Kills {kills}. Contract until {expire}.",
        level = packet.level,
        hp = packet.health_points,
        max_hp = packet.maximum_health_points,
        sp = packet.spell_points,
        max_sp = packet.maximum_spell_points,
        attack = packet.attack,
        magic_attack = packet.magic_attack,
        hit = packet.hit,
        critical = packet.critical,
        defense = packet.defense,
        magic_defense = packet.magic_defense,
        flee = packet.flee,
        attack_speed = packet.attack_speed,
        faith = packet.faith,
        summons = packet.summons,
        kills = packet.kills,
        expire = packet.expire_date,
    )
}

pub fn mercenary_variable_name(variable: u16) -> &'static str {
    match variable {
        MERC_VAR_ATTACK => "ATK",
        MERC_VAR_MAGIC_ATTACK => "MATK",
        MERC_VAR_HIT => "HIT",
        MERC_VAR_CRITICAL => "CRI",
        MERC_VAR_DEFENSE => "DEF",
        MERC_VAR_MAGIC_DEFENSE => "MDEF",
        MERC_VAR_FLEE => "FLEE",
        MERC_VAR_ATTACK_SPEED => "ASPD",
        MERC_VAR_HP => "HP",
        MERC_VAR_MAX_HP => "max HP",
        MERC_VAR_SP => "SP",
        MERC_VAR_MAX_SP => "max SP",
        MERC_VAR_KILLS => "kills",
        MERC_VAR_FAITH => "faith",
        _ => "stat",
    }
}

pub fn elemental_variable_name(variable: u16) -> &'static str {
    match variable {
        EL_VAR_HP => "HP",
        EL_VAR_MAX_HP => "max HP",
        EL_VAR_SP => "SP",
        EL_VAR_MAX_SP => "max SP",
        _ => "stat",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_flags() {
        assert_eq!(intimacy_label(50), "Awkward");
        assert_eq!(intimacy_label(1000), "Devoted");
        assert_eq!(format_homunculus_flags(0x07), "renamed, resting, alive");
        assert_eq!(elemental_name(2114), "Agni");
        assert_eq!(mercenary_variable_name(MERC_VAR_HP), "HP");
        assert_eq!(elemental_variable_name(EL_VAR_SP), "SP");
    }
}
