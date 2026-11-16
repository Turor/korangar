//! Companion window state: capture, eggs, pet, homunculus, mercenary,
//! elemental.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use korangar_interface::element::StateElement;
use korangar_networking::{
    EL_VAR_HP, EL_VAR_MAX_HP, EL_VAR_MAX_SP, EL_VAR_SP, HOM_STATE_HUNGER, HOM_STATE_INTIMACY, MERC_VAR_ATTACK, MERC_VAR_ATTACK_SPEED,
    MERC_VAR_CRITICAL, MERC_VAR_DEFENSE, MERC_VAR_FAITH, MERC_VAR_FLEE, MERC_VAR_HIT, MERC_VAR_HP, MERC_VAR_KILLS, MERC_VAR_MAGIC_ATTACK,
    MERC_VAR_MAGIC_DEFENSE, MERC_VAR_MAX_HP, MERC_VAR_MAX_SP, MERC_VAR_SP, PET_KIND_ACCESSORY, PET_KIND_HUNGER, PET_KIND_INTIMACY,
    append_name_character, capture_arm, capture_cancel, capture_click, capture_is_armed, capture_result, elemental_variable_name,
    format_elemental, format_pet, hunger_label, intimacy_label, mercenary_remaining, mercenary_variable_name, validate_companion_name,
};
use rust_state::RustState;

use crate::graphics::Texture;

/// Pet fields kept so hunger, intimacy, and accessory updates can rewrite the
/// line.
pub struct PetPanel {
    pub name: String,
    pub renamed: bool,
    pub level: i16,
    pub hunger: i16,
    pub intimacy: i16,
    pub accessory: i16,
    pub class_id: i16,
}

/// Mercenary fields kept so `ZC_MER_PAR_CHANGE` can rewrite the line.
pub struct MercenaryPanel {
    pub account_id: u32,
    pub name: String,
    pub level: i16,
    pub attack: i16,
    pub magic_attack: i16,
    pub hit: i16,
    pub critical: i16,
    pub defense: i16,
    pub magic_defense: i16,
    pub flee: i16,
    pub attack_speed: i16,
    pub health_points: i32,
    pub maximum_health_points: i32,
    pub spell_points: i32,
    pub maximum_spell_points: i32,
    pub expire_date: i32,
    pub faith: i16,
    pub summons: i32,
    pub kills: i32,
    pub attack_range: i16,
}

/// Homunculus fields from `ZC_PROPERTY_HOMUN` (`0x0BA4`).
pub struct HomunculusPanel {
    pub name: String,
    pub flags: u8,
    pub level: u16,
    pub hunger: u16,
    pub intimacy: u16,
    pub attack: u16,
    pub magic_attack: u16,
    pub hit: u16,
    pub critical: u16,
    pub defense: u16,
    pub magic_defense: u16,
    pub flee: u16,
    pub attack_motion: u16,
    pub health_points: u32,
    pub maximum_health_points: u32,
    pub spell_points: u32,
    pub maximum_spell_points: u32,
    pub experience: i64,
    pub next_experience: i64,
    pub skill_points: u16,
    pub attack_range: u16,
}

/// One homunculus or mercenary skill shown with its icon.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct CompanionSkill {
    pub skill_id: u16,
    pub level: i16,
    pub sp_cost: i16,
    pub attack_range: i16,
    pub name: String,
    /// Painted row. Includes "Level up" only when this list offers skill-up.
    pub label: String,
    pub upgradable: bool,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

/// One hatchable egg. `wire_index` is the value `CZ_SELECT_PETEGG` echoes.
#[derive(Clone, Debug, RustState, StateElement)]
pub struct EggChoice {
    pub wire_index: u16,
    pub label: String,
    pub item_id: u32,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

/// Labels and numbers the pet, homunculus, mercenary, and elemental panels
/// show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompanionPanelView {
    pub status: String,
    pub capture_armed: bool,
    pub pet_known: bool,
    pub pet_name: String,
    pub pet_level: i16,
    pub pet_hunger: i16,
    pub pet_hunger_label: String,
    pub pet_intimacy: i16,
    pub pet_intimacy_label: String,
    pub pet_accessory: i16,
    pub pet_rename_allowed: bool,
    pub homunculus_known: bool,
    pub homunculus_name: String,
    pub homunculus_level: u16,
    pub homunculus_hp: u32,
    pub homunculus_max_hp: u32,
    pub homunculus_sp: u32,
    pub homunculus_max_sp: u32,
    pub homunculus_exp: i64,
    pub homunculus_next_exp: i64,
    pub homunculus_hunger: u16,
    pub homunculus_intimacy: u16,
    pub homunculus_intimacy_label: String,
    pub homunculus_skill_points: u16,
    pub homunculus_rename_allowed: bool,
    pub homunculus_skills: Vec<SkillView>,
    pub ai_mode: String,
    pub mercenary_known: bool,
    pub mercenary_hp: i32,
    pub mercenary_max_hp: i32,
    pub mercenary_sp: i32,
    pub mercenary_max_sp: i32,
    pub mercenary_remaining: String,
    pub mercenary_skills: Vec<SkillView>,
    pub elemental_known: bool,
    pub elemental_hp: u32,
    pub elemental_max_hp: u32,
    pub elemental_sp: u32,
    pub elemental_max_sp: u32,
    pub eggs: Vec<(u16, String)>,
    pub empty_pet: bool,
    pub empty_homunculus: bool,
    pub empty_mercenary: bool,
    pub empty_elemental: bool,
}

/// One skill line in [`CompanionPanelView`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillView {
    pub skill_id: u16,
    pub name: String,
    pub level: i16,
    pub upgradable: bool,
}

/// Live homunculus, pet, mercenary, and elemental stats.
#[derive(RustState, StateElement)]
pub struct CompanionWindowState {
    status: String,
    pet_text: String,
    homunculus_text: String,
    mercenary_text: String,
    elemental_text: String,
    pet_name: String,
    pet_renamed: bool,
    pet_level: i16,
    pet_hunger: i16,
    pet_intimacy: i16,
    pet_accessory: i16,
    pet_class: i16,
    pet_known: bool,
    pet_hunger_label: String,
    pet_hunger_line: String,
    pet_intimacy_label: String,
    pet_intimacy_line: String,
    pet_accessory_label: String,
    pet_rename_draft: String,
    pet_rename_allowed: bool,
    #[hidden_element]
    pet_accessory_texture: Option<Arc<Texture>>,
    homunculus_name: String,
    homunculus_flags: u8,
    homunculus_level: u16,
    homunculus_hunger: u16,
    homunculus_intimacy: u16,
    homunculus_attack: u16,
    homunculus_magic_attack: u16,
    homunculus_hit: u16,
    homunculus_critical: u16,
    homunculus_defense: u16,
    homunculus_magic_defense: u16,
    homunculus_flee: u16,
    homunculus_attack_motion: u16,
    homunculus_hp: u32,
    homunculus_max_hp: u32,
    homunculus_sp: u32,
    homunculus_max_sp: u32,
    homunculus_exp: i64,
    homunculus_next_exp: i64,
    homunculus_skill_points: u16,
    homunculus_range: u16,
    homunculus_known: bool,
    homunculus_intimacy_label: String,
    homunculus_hp_line: String,
    homunculus_sp_line: String,
    homunculus_exp_line: String,
    homunculus_hunger_line: String,
    homunculus_intimacy_line: String,
    homunculus_rename_draft: String,
    homunculus_rename_allowed: bool,
    homunculus_entity_id: u32,
    homunculus_skills: Vec<CompanionSkill>,
    ai_mode: String,
    attack_target: u32,
    mercenary_account_id: u32,
    mercenary_name: String,
    mercenary_level: i16,
    mercenary_attack: i16,
    mercenary_magic_attack: i16,
    mercenary_hit: i16,
    mercenary_critical: i16,
    mercenary_defense: i16,
    mercenary_magic_defense: i16,
    mercenary_flee: i16,
    mercenary_attack_speed: i16,
    mercenary_hp: i32,
    mercenary_max_hp: i32,
    mercenary_sp: i32,
    mercenary_max_sp: i32,
    mercenary_expire: i32,
    mercenary_faith: i16,
    mercenary_summons: i32,
    mercenary_kills: i32,
    mercenary_range: i16,
    mercenary_known: bool,
    mercenary_remaining: String,
    mercenary_hp_line: String,
    mercenary_sp_line: String,
    mercenary_skills: Vec<CompanionSkill>,
    elemental_class: u32,
    elemental_hp: u32,
    elemental_max_hp: u32,
    elemental_sp: u32,
    elemental_max_sp: u32,
    elemental_hp_line: String,
    elemental_sp_line: String,
    elemental_known: bool,
    eggs: Vec<EggChoice>,
    capture_phase: u8,
}

impl Default for CompanionWindowState {
    fn default() -> Self {
        Self {
            status: "Companions appear here when the map server sends them.".to_owned(),
            pet_text: "No pet.".to_owned(),
            homunculus_text: "No homunculus.".to_owned(),
            mercenary_text: "No mercenary.".to_owned(),
            elemental_text: "No elemental.".to_owned(),
            pet_name: String::new(),
            pet_renamed: false,
            pet_level: 0,
            pet_hunger: 0,
            pet_intimacy: 0,
            pet_accessory: 0,
            pet_class: 0,
            pet_known: false,
            pet_hunger_label: String::new(),
            pet_hunger_line: "Hunger ·  · 0/100".to_owned(),
            pet_intimacy_label: String::new(),
            pet_intimacy_line: "Intimacy ·  · 0/1000".to_owned(),
            pet_accessory_label: "No accessory.".to_owned(),
            pet_rename_draft: String::new(),
            pet_rename_allowed: false,
            pet_accessory_texture: None,
            homunculus_name: String::new(),
            homunculus_flags: 0,
            homunculus_level: 0,
            homunculus_hunger: 0,
            homunculus_intimacy: 0,
            homunculus_attack: 0,
            homunculus_magic_attack: 0,
            homunculus_hit: 0,
            homunculus_critical: 0,
            homunculus_defense: 0,
            homunculus_magic_defense: 0,
            homunculus_flee: 0,
            homunculus_attack_motion: 0,
            homunculus_hp: 0,
            homunculus_max_hp: 0,
            homunculus_sp: 0,
            homunculus_max_sp: 0,
            homunculus_exp: 0,
            homunculus_next_exp: 0,
            homunculus_skill_points: 0,
            homunculus_range: 0,
            homunculus_known: false,
            homunculus_intimacy_label: String::new(),
            homunculus_hp_line: "HP 0/0".to_owned(),
            homunculus_sp_line: "SP 0/0".to_owned(),
            homunculus_exp_line: "EXP 0/0".to_owned(),
            homunculus_hunger_line: "Hunger 0/100".to_owned(),
            homunculus_intimacy_line: "Intimacy ·  · 0/1000".to_owned(),
            homunculus_rename_draft: String::new(),
            homunculus_rename_allowed: false,
            homunculus_entity_id: 0,
            homunculus_skills: Vec::new(),
            ai_mode: "Standby".to_owned(),
            attack_target: 0,
            mercenary_account_id: 0,
            mercenary_name: String::new(),
            mercenary_level: 0,
            mercenary_attack: 0,
            mercenary_magic_attack: 0,
            mercenary_hit: 0,
            mercenary_critical: 0,
            mercenary_defense: 0,
            mercenary_magic_defense: 0,
            mercenary_flee: 0,
            mercenary_attack_speed: 0,
            mercenary_hp: 0,
            mercenary_max_hp: 0,
            mercenary_sp: 0,
            mercenary_max_sp: 0,
            mercenary_expire: 0,
            mercenary_faith: 0,
            mercenary_summons: 0,
            mercenary_kills: 0,
            mercenary_range: 0,
            mercenary_known: false,
            mercenary_remaining: "No contract.".to_owned(),
            mercenary_hp_line: "HP 0/0".to_owned(),
            mercenary_sp_line: "SP 0/0".to_owned(),
            mercenary_skills: Vec::new(),
            elemental_class: 0,
            elemental_hp: 0,
            elemental_max_hp: 0,
            elemental_sp: 0,
            elemental_max_sp: 0,
            elemental_hp_line: "HP 0/0".to_owned(),
            elemental_sp_line: "SP 0/0".to_owned(),
            elemental_known: false,
            eggs: Vec::new(),
            capture_phase: 0,
        }
    }
}

impl CompanionWindowState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn panel_view(&self) -> CompanionPanelView {
        CompanionPanelView {
            status: self.status.clone(),
            capture_armed: self.capture_armed(),
            pet_known: self.pet_known,
            pet_name: self.pet_name.clone(),
            pet_level: self.pet_level,
            pet_hunger: self.pet_hunger,
            pet_hunger_label: self.pet_hunger_label.clone(),
            pet_intimacy: self.pet_intimacy,
            pet_intimacy_label: self.pet_intimacy_label.clone(),
            pet_accessory: self.pet_accessory,
            pet_rename_allowed: self.pet_rename_allowed,
            homunculus_known: self.homunculus_known,
            homunculus_name: self.homunculus_name.clone(),
            homunculus_level: self.homunculus_level,
            homunculus_hp: self.homunculus_hp,
            homunculus_max_hp: self.homunculus_max_hp,
            homunculus_sp: self.homunculus_sp,
            homunculus_max_sp: self.homunculus_max_sp,
            homunculus_exp: self.homunculus_exp,
            homunculus_next_exp: self.homunculus_next_exp,
            homunculus_hunger: self.homunculus_hunger,
            homunculus_intimacy: self.homunculus_intimacy,
            homunculus_intimacy_label: self.homunculus_intimacy_label.clone(),
            homunculus_skill_points: self.homunculus_skill_points,
            homunculus_rename_allowed: self.homunculus_rename_allowed,
            homunculus_skills: skill_views(&self.homunculus_skills),
            ai_mode: self.ai_mode.clone(),
            mercenary_known: self.mercenary_known,
            mercenary_hp: self.mercenary_hp,
            mercenary_max_hp: self.mercenary_max_hp,
            mercenary_sp: self.mercenary_sp,
            mercenary_max_sp: self.mercenary_max_sp,
            mercenary_remaining: self.mercenary_remaining.clone(),
            mercenary_skills: skill_views(&self.mercenary_skills),
            elemental_known: self.elemental_known,
            elemental_hp: self.elemental_hp,
            elemental_max_hp: self.elemental_max_hp,
            elemental_sp: self.elemental_sp,
            elemental_max_sp: self.elemental_max_sp,
            eggs: self.eggs.iter().map(|egg| (egg.wire_index, egg.label.clone())).collect(),
            empty_pet: self.pet_text == "No pet.",
            empty_homunculus: self.homunculus_text == "No homunculus.",
            empty_mercenary: self.mercenary_text == "No mercenary.",
            empty_elemental: self.elemental_text == "No elemental.",
        }
    }

    pub fn show_pet(&mut self, pet: PetPanel, summary: String) {
        self.pet_known = true;
        self.pet_name = pet.name;
        self.pet_renamed = pet.renamed;
        self.pet_level = pet.level;
        self.pet_hunger = pet.hunger;
        self.pet_intimacy = pet.intimacy;
        self.pet_accessory = pet.accessory;
        self.pet_class = pet.class_id;
        self.pet_text = summary;
        self.pet_rename_allowed = !pet.renamed;
        self.refresh_pet_labels();
        self.status = "Pet updated.".to_owned();
    }

    pub fn set_accessory_texture(&mut self, texture: Option<Arc<Texture>>) {
        self.pet_accessory_texture = texture;
    }

    pub fn apply_pet_state(&mut self, kind: i8, value: i32) {
        if !self.pet_known {
            self.pet_known = true;
        }
        match kind {
            PET_KIND_INTIMACY => self.pet_intimacy = value.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            PET_KIND_HUNGER => self.pet_hunger = value.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            PET_KIND_ACCESSORY => self.pet_accessory = value.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            _ => {
                self.status = format!("Pet state {kind} = {value}.");
                return;
            }
        }
        self.pet_text = format_pet(
            &self.pet_name,
            self.pet_renamed,
            self.pet_level,
            self.pet_hunger,
            self.pet_intimacy,
            self.pet_accessory,
            self.pet_class,
        );
        self.refresh_pet_labels();
        self.status = "Pet updated.".to_owned();
    }

    pub fn show_homunculus(&mut self, homunculus: HomunculusPanel, summary: String) {
        self.homunculus_known = true;
        self.homunculus_name = homunculus.name;
        self.homunculus_flags = homunculus.flags;
        self.homunculus_level = homunculus.level;
        self.homunculus_hunger = homunculus.hunger;
        self.homunculus_intimacy = homunculus.intimacy;
        self.homunculus_attack = homunculus.attack;
        self.homunculus_magic_attack = homunculus.magic_attack;
        self.homunculus_hit = homunculus.hit;
        self.homunculus_critical = homunculus.critical;
        self.homunculus_defense = homunculus.defense;
        self.homunculus_magic_defense = homunculus.magic_defense;
        self.homunculus_flee = homunculus.flee;
        self.homunculus_attack_motion = homunculus.attack_motion;
        self.homunculus_hp = homunculus.health_points;
        self.homunculus_max_hp = homunculus.maximum_health_points;
        self.homunculus_sp = homunculus.spell_points;
        self.homunculus_max_sp = homunculus.maximum_spell_points;
        self.homunculus_exp = homunculus.experience;
        self.homunculus_next_exp = homunculus.next_experience;
        self.homunculus_skill_points = homunculus.skill_points;
        self.homunculus_range = homunculus.attack_range;
        self.homunculus_text = summary;
        self.homunculus_rename_allowed = homunculus.flags & 0x01 == 0;
        self.homunculus_intimacy_label = intimacy_label(i32::from(self.homunculus_intimacy)).to_owned();
        self.refresh_homunculus_lines();
        self.status = "Homunculus updated.".to_owned();
    }

    pub fn apply_homunculus_state(&mut self, kind: i8, entity_id: u32, value: i32) {
        if entity_id != 0 {
            self.homunculus_entity_id = entity_id;
        }
        self.homunculus_known = true;
        let narrow = value.clamp(0, i32::from(u16::MAX)) as u16;
        match kind {
            HOM_STATE_INTIMACY => {
                self.homunculus_intimacy = narrow;
                self.homunculus_intimacy_label = intimacy_label(i32::from(narrow)).to_owned();
                self.status = format!("Homunculus intimacy is {}.", self.homunculus_intimacy_label);
            }
            HOM_STATE_HUNGER => {
                self.homunculus_hunger = narrow;
                self.status = format!("Homunculus hunger is {narrow}/100.");
            }
            _ => self.status = format!("Homunculus state {kind} = {value}."),
        }
        self.refresh_homunculus_lines();
    }

    pub fn set_homunculus_skills(&mut self, skills: Vec<CompanionSkill>) {
        let previous = std::mem::take(&mut self.homunculus_skills);
        self.homunculus_skills = skills
            .into_iter()
            .map(|mut skill| {
                if skill.texture.is_none() {
                    skill.texture = previous
                        .iter()
                        .find(|old| old.skill_id == skill.skill_id)
                        .and_then(|old| old.texture.clone());
                }
                skill
            })
            .collect();
        self.homunculus_known = true;
        self.status = "Homunculus skills updated.".to_owned();
    }

    pub fn apply_homunculus_skill_update(&mut self, skill_id: u16, level: i16, sp_cost: i16, attack_range: i16, upgradable: bool) {
        if let Some(skill) = self.homunculus_skills.iter_mut().find(|skill| skill.skill_id == skill_id) {
            skill.level = level;
            skill.sp_cost = sp_cost;
            skill.attack_range = attack_range;
            skill.upgradable = upgradable;
            let name = skill.name.clone();
            skill.label = format_skill_label(&name, skill.level, skill.sp_cost, skill.upgradable, true);
        }
    }

    /// `keep_range` is set for `ZC_MER_PROPERTY`, which has no attack range.
    /// `keep_account` is set for that same packet, which has no account id.
    pub fn show_mercenary(&mut self, mercenary: MercenaryPanel, keep_range: bool, summary: String) {
        let range = if keep_range && mercenary.attack_range == 0 {
            self.mercenary_range
        } else {
            mercenary.attack_range
        };
        let account_id = if keep_range && mercenary.account_id == 0 {
            self.mercenary_account_id
        } else {
            mercenary.account_id
        };
        self.mercenary_known = true;
        self.mercenary_account_id = account_id;
        self.mercenary_name = mercenary.name;
        self.mercenary_level = mercenary.level;
        self.mercenary_attack = mercenary.attack;
        self.mercenary_magic_attack = mercenary.magic_attack;
        self.mercenary_hit = mercenary.hit;
        self.mercenary_critical = mercenary.critical;
        self.mercenary_defense = mercenary.defense;
        self.mercenary_magic_defense = mercenary.magic_defense;
        self.mercenary_flee = mercenary.flee;
        self.mercenary_attack_speed = mercenary.attack_speed;
        self.mercenary_hp = mercenary.health_points;
        self.mercenary_max_hp = mercenary.maximum_health_points;
        self.mercenary_sp = mercenary.spell_points;
        self.mercenary_max_sp = mercenary.maximum_spell_points;
        self.mercenary_expire = mercenary.expire_date;
        self.mercenary_faith = mercenary.faith;
        self.mercenary_summons = mercenary.summons;
        self.mercenary_kills = mercenary.kills;
        self.mercenary_range = range;
        self.mercenary_remaining = mercenary_remaining(self.mercenary_expire, unix_now());
        self.mercenary_text = if keep_range { format!("{summary} Range {range}.") } else { summary };
        self.refresh_mercenary_lines();
        self.status = "Mercenary updated.".to_owned();
    }

    pub fn apply_mercenary_parameter(&mut self, variable: u16, value: i32) {
        if !self.mercenary_known {
            self.mercenary_known = true;
        }
        let narrow = value.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        match variable {
            MERC_VAR_ATTACK => self.mercenary_attack = narrow,
            MERC_VAR_MAGIC_ATTACK => self.mercenary_magic_attack = narrow,
            MERC_VAR_HIT => self.mercenary_hit = narrow,
            MERC_VAR_CRITICAL => self.mercenary_critical = narrow,
            MERC_VAR_DEFENSE => self.mercenary_defense = narrow,
            MERC_VAR_MAGIC_DEFENSE => self.mercenary_magic_defense = narrow,
            MERC_VAR_FLEE => self.mercenary_flee = narrow,
            MERC_VAR_ATTACK_SPEED => self.mercenary_attack_speed = narrow,
            MERC_VAR_HP => self.mercenary_hp = value,
            MERC_VAR_MAX_HP => self.mercenary_max_hp = value,
            MERC_VAR_SP => self.mercenary_sp = value,
            MERC_VAR_MAX_SP => self.mercenary_max_sp = value,
            MERC_VAR_KILLS => self.mercenary_kills = value,
            MERC_VAR_FAITH => self.mercenary_faith = narrow,
            _ => {}
        }
        self.mercenary_text = format!(
            "{} Lv {}. HP {}/{} SP {}/{}. ATK {} MATK {} HIT {} CRI {} DEF {} MDEF {} FLEE {} ASPD {} range {}. Faith {}. Summons {}. \
             Kills {}. Contract until {}.",
            if self.mercenary_name.is_empty() {
                "Mercenary"
            } else {
                self.mercenary_name.as_str()
            },
            self.mercenary_level,
            self.mercenary_hp,
            self.mercenary_max_hp,
            self.mercenary_sp,
            self.mercenary_max_sp,
            self.mercenary_attack,
            self.mercenary_magic_attack,
            self.mercenary_hit,
            self.mercenary_critical,
            self.mercenary_defense,
            self.mercenary_magic_defense,
            self.mercenary_flee,
            self.mercenary_attack_speed,
            self.mercenary_range,
            self.mercenary_faith,
            self.mercenary_summons,
            self.mercenary_kills,
            self.mercenary_expire,
        );
        self.refresh_mercenary_lines();
        self.status = format!("Mercenary {} is {value}.", mercenary_variable_name(variable));
    }

    pub fn set_mercenary_skills(&mut self, skills: Vec<CompanionSkill>) {
        let previous = std::mem::take(&mut self.mercenary_skills);
        self.mercenary_skills = skills
            .into_iter()
            .map(|mut skill| {
                if skill.texture.is_none() {
                    skill.texture = previous
                        .iter()
                        .find(|old| old.skill_id == skill.skill_id)
                        .and_then(|old| old.texture.clone());
                }
                skill
            })
            .collect();
        self.mercenary_known = true;
        self.status = "Mercenary skills updated.".to_owned();
    }

    pub fn apply_mercenary_skill_update(&mut self, skill_id: u16, level: i16, sp_cost: i16, attack_range: i16, upgradable: bool) {
        if let Some(skill) = self.mercenary_skills.iter_mut().find(|skill| skill.skill_id == skill_id) {
            skill.level = level;
            skill.sp_cost = sp_cost;
            skill.attack_range = attack_range;
            skill.upgradable = upgradable;
            let name = skill.name.clone();
            skill.label = format_skill_label(&name, skill.level, skill.sp_cost, skill.upgradable, false);
        }
    }

    pub fn refresh_contract(&mut self, now: i64) {
        if self.mercenary_known {
            self.mercenary_remaining = mercenary_remaining(self.mercenary_expire, now);
        }
    }

    pub fn show_elemental(&mut self, class_id: u32, hp: u32, max_hp: u32, sp: u32, max_sp: u32, summary: String) {
        self.elemental_known = true;
        self.elemental_class = class_id;
        self.elemental_hp = hp;
        self.elemental_max_hp = max_hp;
        self.elemental_sp = sp;
        self.elemental_max_sp = max_sp;
        self.elemental_text = summary;
        self.refresh_elemental_lines();
        self.status = "Elemental updated.".to_owned();
    }

    pub fn apply_elemental_parameter(&mut self, variable: u16, value: u32) {
        if !self.elemental_known {
            self.elemental_known = true;
        }
        match variable {
            EL_VAR_HP => self.elemental_hp = value,
            EL_VAR_MAX_HP => self.elemental_max_hp = value,
            EL_VAR_SP => self.elemental_sp = value,
            EL_VAR_MAX_SP => self.elemental_max_sp = value,
            _ => {}
        }
        self.elemental_text = format_elemental(
            self.elemental_class,
            self.elemental_hp,
            self.elemental_max_hp,
            self.elemental_sp,
            self.elemental_max_sp,
        );
        self.refresh_elemental_lines();
        self.status = format!("Elemental {} is {value}.", elemental_variable_name(variable));
    }

    pub fn capture_armed(&self) -> bool {
        capture_is_armed(self.capture_phase)
    }

    pub fn arm_capture(&mut self) {
        self.capture_phase = capture_arm();
        self.status = "Click a monster to tame it.".to_owned();
    }

    /// Escape while capture is armed. Returns whether Escape was consumed.
    pub fn cancel_capture(&mut self) -> bool {
        if let Some(phase) = capture_cancel(self.capture_phase) {
            self.capture_phase = phase;
            self.status = "Capture cancelled.".to_owned();
            true
        } else {
            false
        }
    }

    pub fn apply_capture_result(&mut self, success: bool) {
        self.capture_phase = capture_result(success);
        self.status = if success {
            "Taming succeeded.".to_owned()
        } else {
            "Taming failed.".to_owned()
        };
    }

    /// A monster click while armed should send `0x019F` and stay armed.
    pub fn accept_capture_click(&mut self) -> bool {
        let (phase, send) = capture_click(self.capture_phase, true);
        self.capture_phase = phase;
        if send {
            self.status = "Trying to tame this monster.".to_owned();
        }
        send
    }

    pub fn show_eggs(&mut self, eggs: Vec<EggChoice>) {
        self.eggs = eggs;
        self.status = if self.eggs.is_empty() {
            "No eggs to hatch.".to_owned()
        } else {
            "Choose an egg to hatch.".to_owned()
        };
    }

    /// Choose an egg by row. Returns the wire index to send, unchanged.
    pub fn choose_egg(&mut self, row: usize) -> Option<u16> {
        let wire = self.eggs.get(row).map(|egg| egg.wire_index)?;
        self.eggs.clear();
        self.status = "Hatching the egg.".to_owned();
        Some(wire)
    }

    pub fn choose_egg_wire(&mut self, wire_index: u16) -> bool {
        if self.eggs.iter().any(|egg| egg.wire_index == wire_index) {
            self.eggs.clear();
            self.status = "Hatching the egg.".to_owned();
            true
        } else {
            false
        }
    }

    pub fn cancel_eggs(&mut self) {
        if !self.eggs.is_empty() {
            self.eggs.clear();
            self.status = "Egg hatching cancelled.".to_owned();
        }
    }

    pub fn append_pet_name(&mut self, character: char) -> bool {
        append_name_character(&mut self.pet_rename_draft, character)
    }

    pub fn append_homunculus_name(&mut self, character: char) -> bool {
        append_name_character(&mut self.homunculus_rename_draft, character)
    }

    pub fn confirm_pet_rename(&mut self) -> Result<String, &'static str> {
        if !self.pet_rename_allowed {
            self.status = "This pet cannot be renamed.".to_owned();
            return Err("rename is not allowed");
        }
        match validate_companion_name(&self.pet_rename_draft) {
            Ok(name) => {
                self.status = format!("Renaming pet to {name}.");
                self.pet_rename_draft.clear();
                Ok(name)
            }
            Err(message) => {
                self.status = message.to_owned();
                Err(message)
            }
        }
    }

    pub fn confirm_homunculus_rename(&mut self) -> Result<String, &'static str> {
        if !self.homunculus_rename_allowed {
            self.status = "This homunculus cannot be renamed.".to_owned();
            return Err("rename is not allowed");
        }
        match validate_companion_name(&self.homunculus_rename_draft) {
            Ok(name) => {
                self.status = format!("Renaming homunculus to {name}.");
                self.homunculus_rename_draft.clear();
                Ok(name)
            }
            Err(message) => {
                self.status = message.to_owned();
                Err(message)
            }
        }
    }

    /// Actor id for follow and attack. The homunculus GID wins over the
    /// mercenary.
    pub fn command_actor(&self) -> u32 {
        if self.homunculus_entity_id != 0 {
            self.homunculus_entity_id
        } else {
            self.mercenary_account_id
        }
    }

    pub fn command_follow(&mut self) -> Option<u32> {
        self.ai_mode = "Follow".to_owned();
        let actor = self.command_actor();
        if actor == 0 {
            self.status = "No homunculus or mercenary is on the map yet.".to_owned();
            None
        } else {
            self.status = "Following.".to_owned();
            Some(actor)
        }
    }

    pub fn command_attack(&mut self) -> Option<(u32, u32)> {
        self.ai_mode = "Attack".to_owned();
        let actor = self.command_actor();
        if actor == 0 {
            self.status = "No homunculus or mercenary is on the map yet.".to_owned();
            return None;
        }
        if self.attack_target == 0 {
            self.status = "Click a monster to attack.".to_owned();
            return None;
        }
        self.status = "Attacking.".to_owned();
        Some((actor, self.attack_target))
    }

    /// Standby stops the client attack mode. Hercules has no standby opcode.
    pub fn command_standby(&mut self) {
        self.ai_mode = "Standby".to_owned();
        self.status = "Standing by.".to_owned();
    }

    pub fn remember_attack_target(&mut self, entity_id: u32) {
        self.attack_target = entity_id;
    }

    pub fn attack_follow_up(&self, entity_id: u32) -> Option<(u32, u32)> {
        if self.ai_mode != "Attack" {
            return None;
        }
        let actor = self.command_actor();
        if actor == 0 { None } else { Some((actor, entity_id)) }
    }

    pub fn can_level_homunculus_skill(&self, skill_id: u16) -> bool {
        self.homunculus_skill_points > 0
            && self
                .homunculus_skills
                .iter()
                .any(|skill| skill.skill_id == skill_id && skill.upgradable)
    }

    pub fn note_skill_up_sent(&mut self, skill_id: u16) {
        if self.homunculus_skill_points > 0 {
            self.homunculus_skill_points -= 1;
        }
        self.status = format!("Raising skill {skill_id}.");
    }

    fn refresh_pet_labels(&mut self) {
        self.pet_hunger_label = hunger_label(i32::from(self.pet_hunger)).to_owned();
        self.pet_intimacy_label = intimacy_label(i32::from(self.pet_intimacy)).to_owned();
        self.pet_hunger_line = format!("Hunger · {} · {}/100", self.pet_hunger_label, self.pet_hunger);
        self.pet_intimacy_line = format!("Intimacy · {} · {}/1000", self.pet_intimacy_label, self.pet_intimacy);
        self.pet_accessory_label = if self.pet_accessory == 0 {
            "No accessory.".to_owned()
        } else {
            format!("Accessory {}", self.pet_accessory)
        };
    }

    fn refresh_homunculus_lines(&mut self) {
        self.homunculus_hp_line = format!("HP {}/{}", self.homunculus_hp, self.homunculus_max_hp);
        self.homunculus_sp_line = format!("SP {}/{}", self.homunculus_sp, self.homunculus_max_sp);
        self.homunculus_exp_line = format!("EXP {}/{}", self.homunculus_exp, self.homunculus_next_exp);
        self.homunculus_hunger_line = format!("Hunger {}/100", self.homunculus_hunger);
        self.homunculus_intimacy_line = format!(
            "Intimacy · {} · {}/1000",
            self.homunculus_intimacy_label, self.homunculus_intimacy
        );
    }

    fn refresh_mercenary_lines(&mut self) {
        self.mercenary_hp_line = format!("HP {}/{}", self.mercenary_hp, self.mercenary_max_hp);
        self.mercenary_sp_line = format!("SP {}/{}", self.mercenary_sp, self.mercenary_max_sp);
    }

    fn refresh_elemental_lines(&mut self) {
        self.elemental_hp_line = format!("HP {}/{}", self.elemental_hp, self.elemental_max_hp);
        self.elemental_sp_line = format!("SP {}/{}", self.elemental_sp, self.elemental_max_sp);
    }
}

pub(crate) fn format_skill_label(name: &str, level: i16, sp_cost: i16, upgradable: bool, offer_level_up: bool) -> String {
    if upgradable && offer_level_up {
        format!("{name}  Lv {level}  SP {sp_cost}  · Level up")
    } else {
        format!("{name}  Lv {level}  SP {sp_cost}")
    }
}

fn skill_views(skills: &[CompanionSkill]) -> Vec<SkillView> {
    skills
        .iter()
        .map(|skill| SkillView {
            skill_id: skill.skill_id,
            name: skill.name.clone(),
            level: skill.level,
            upgradable: skill.upgradable,
        })
        .collect()
}

pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poring() -> PetPanel {
        PetPanel {
            name: "Poring".to_owned(),
            renamed: false,
            level: 4,
            hunger: 5,
            intimacy: 910,
            accessory: 12,
            class_id: 1002,
        }
    }

    #[test]
    fn pet_panel_labels_and_rename_follow_the_server_flag() {
        let mut state = CompanionWindowState::default();
        state.show_pet(poring(), "summary".to_owned());
        let view = state.panel_view();
        assert_eq!(view.pet_hunger, 5);
        assert_eq!(view.pet_hunger_label, "Very Hungry");
        assert_eq!(view.pet_intimacy, 910);
        assert_eq!(view.pet_intimacy_label, "Loyal");
        assert!(view.pet_rename_allowed);
        assert!(state.append_pet_name('P'));
        assert!(state.append_pet_name('i'));
        assert_eq!(state.confirm_pet_rename().as_deref(), Ok("Pi"));

        state.show_pet(PetPanel { renamed: true, ..poring() }, "summary".to_owned());
        assert!(state.confirm_pet_rename().is_err());
        assert_eq!(state.panel_view().status, "This pet cannot be renamed.");
    }

    #[test]
    fn capture_click_and_escape_use_the_state_machine() {
        let mut state = CompanionWindowState::default();
        assert!(!state.accept_capture_click());
        state.arm_capture();
        assert!(state.capture_armed());
        assert_eq!(state.panel_view().status, "Click a monster to tame it.");
        assert!(state.accept_capture_click());
        assert!(state.capture_armed());
        assert!(state.cancel_capture());
        assert!(!state.capture_armed());
        assert!(!state.accept_capture_click());
        state.arm_capture();
        state.apply_capture_result(false);
        assert!(!state.capture_armed());
        assert_eq!(state.panel_view().status, "Taming failed.");
    }

    #[test]
    fn egg_choice_returns_the_wire_index_and_close_sends_nothing() {
        let mut state = CompanionWindowState::default();
        state.show_eggs(vec![
            EggChoice {
                wire_index: 4,
                label: "Poring Egg".to_owned(),
                item_id: 9001,
                texture: None,
            },
            EggChoice {
                wire_index: 9,
                label: "Drops Egg".to_owned(),
                item_id: 9002,
                texture: None,
            },
        ]);
        assert_eq!(state.panel_view().eggs.len(), 2);
        assert_eq!(state.choose_egg(1), Some(9));
        assert!(state.panel_view().eggs.is_empty());
        state.show_eggs(vec![EggChoice {
            wire_index: 4,
            label: "Poring Egg".to_owned(),
            item_id: 1,
            texture: None,
        }]);
        state.cancel_eggs();
        assert!(state.panel_view().eggs.is_empty());
        assert!(!state.choose_egg_wire(4));
    }
}
