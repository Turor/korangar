use korangar_interface::element::StateElement;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{
    EL_VAR_HP, EL_VAR_MAX_HP, EL_VAR_MAX_SP, EL_VAR_SP, MERC_VAR_ATTACK, MERC_VAR_ATTACK_SPEED, MERC_VAR_CRITICAL, MERC_VAR_DEFENSE,
    MERC_VAR_FAITH, MERC_VAR_FLEE, MERC_VAR_HIT, MERC_VAR_HP, MERC_VAR_KILLS, MERC_VAR_MAGIC_ATTACK, MERC_VAR_MAGIC_DEFENSE, MERC_VAR_MAX_HP,
    MERC_VAR_MAX_SP, MERC_VAR_SP, PET_KIND_ACCESSORY, PET_KIND_HUNGER, PET_KIND_INTIMACY, elemental_variable_name, format_elemental,
    format_pet, mercenary_variable_name,
};
use rust_state::RustState;

use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};

/// Pet fields kept so hunger, intimacy, and accessory updates can rewrite the line.
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
    elemental_class: u32,
    elemental_hp: u32,
    elemental_max_hp: u32,
    elemental_sp: u32,
    elemental_max_sp: u32,
    elemental_known: bool,
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
            elemental_class: 0,
            elemental_hp: 0,
            elemental_max_hp: 0,
            elemental_sp: 0,
            elemental_max_sp: 0,
            elemental_known: false,
        }
    }
}

impl CompanionWindowState {
    pub fn clear(&mut self) {
        *self = Self::default();
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
        self.status = "Pet updated.".to_owned();
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
        self.status = "Pet updated.".to_owned();
    }

    pub fn show_homunculus(&mut self, summary: String) {
        self.homunculus_text = summary;
        self.status = "Homunculus updated.".to_owned();
    }

    /// `keep_range` is set for `ZC_MER_PROPERTY`, which has no attack range.
    pub fn show_mercenary(&mut self, mercenary: MercenaryPanel, keep_range: bool, summary: String) {
        let range = if keep_range && mercenary.attack_range == 0 {
            self.mercenary_range
        } else {
            mercenary.attack_range
        };
        self.mercenary_known = true;
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
        self.mercenary_text = if keep_range {
            format!("{summary} Range {range}.")
        } else {
            summary
        };
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
            "{} Lv {}. HP {}/{} SP {}/{}. ATK {} MATK {} HIT {} CRI {} DEF {} MDEF {} FLEE {} ASPD {} range {}. Faith {}. Summons {}. Kills {}. Contract until {}.",
            if self.mercenary_name.is_empty() { "Mercenary" } else { self.mercenary_name.as_str() },
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
        self.status = format!("Mercenary {} is {value}.", mercenary_variable_name(variable));
    }

    pub fn show_elemental(&mut self, class_id: u32, hp: u32, max_hp: u32, sp: u32, max_sp: u32, summary: String) {
        self.elemental_known = true;
        self.elemental_class = class_id;
        self.elemental_hp = hp;
        self.elemental_max_hp = max_hp;
        self.elemental_sp = sp;
        self.elemental_max_sp = max_sp;
        self.elemental_text = summary;
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
        self.status = format!("Elemental {} is {value}.", elemental_variable_name(variable));
    }
}

pub struct CompanionWindow;

impl CompanionWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for CompanionWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Companions)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().companion_window();

        window! {
            title: "Companions",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            elements: (
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! { text: "Pet" },
                text! {
                    text: state_path.pet_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! { text: "Feed pet", event: InputEvent::PetFeed },
                button! { text: "Pet performance", event: InputEvent::PetPerformance },
                button! { text: "Return pet to egg", event: InputEvent::PetReturn },
                button! { text: "Unequip pet accessory", event: InputEvent::PetUnequip },
                text! { text: "Homunculus" },
                text! {
                    text: state_path.homunculus_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! { text: "Feed homunculus", event: InputEvent::HomunculusFeed },
                button! { text: "Release homunculus", event: InputEvent::HomunculusRelease },
                text! { text: "Mercenary" },
                text! {
                    text: state_path.mercenary_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                button! { text: "Dismiss mercenary", event: InputEvent::MercenaryDismiss },
                text! { text: "Elemental" },
                text! {
                    text: state_path.elemental_text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
            ),
        }
    }
}
