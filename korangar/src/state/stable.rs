//! Turoran stable board: homunculus lineages, cute pets, evolution materials.
//!
//! Button clicks become `0x0EF1` / `0x0EF2`. Action words match
//! `turoran_stable.c` (`STABLE_PET_*`).

use std::sync::Arc;

use korangar_interface::element::StateElement;
use korangar_networking::format_stable_result;
#[cfg(test)]
use ragnarok_bytes::ByteWriter;
#[cfg(test)]
use ragnarok_packets::{PacketExt, RequestStableHomunculusPacket, RequestStableOpenPacket, RequestStablePetPacket};
use ragnarok_packets::{StableHomunculusEntry, StableOwnedEntry, StablePetEntry};
use rust_state::RustState;

use crate::graphics::Texture;
use crate::state::stable_catalog::pet_species;

/// Store the active homunculus (`0x0EF1`, id 0).
pub const HOMUNCULUS_STORE: i32 = 0;

pub const PET_STORE: i16 = 0;
pub const PET_SUMMON: i16 = 1;
pub const PET_DEPOSIT_EGG: i16 = 2;
pub const PET_TO_EGG: i16 = 3;
pub const PET_FEED: i16 = 4;
pub const PET_EQUIP: i16 = 5;
pub const PET_UNEQUIP: i16 = 6;
pub const PET_EVOLVE: i16 = 7;

const PET_FLAG_OUT: u8 = 0x02;
const PET_FLAG_EGG: u8 = 0x04;
const PET_LOYAL: i16 = 900;

/// Wire image of one stable button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StableRequest {
    Open,
    Homunculus(i32),
    Pet { action: i16, id: i32 },
}

impl StableRequest {
    #[cfg(test)]
    fn encode(self) -> Vec<u8> {
        let mut writer = ByteWriter::new();
        match self {
            Self::Open => {
                RequestStableOpenPacket::new().packet_to_bytes(&mut writer).expect("stable open");
            }
            Self::Homunculus(homunculus_id) => {
                RequestStableHomunculusPacket { homunculus_id }
                    .packet_to_bytes(&mut writer)
                    .expect("stable homunculus");
            }
            Self::Pet { action, id } => {
                RequestStablePetPacket { action, pet_id: id }
                    .packet_to_bytes(&mut writer)
                    .expect("stable pet");
            }
        }
        writer.into_inner()
    }
}

/// Identified item name and inventory icon, filled when a list arrives.
#[derive(Clone, Debug)]
pub struct ItemFace {
    pub item_id: i32,
    pub name: String,
    pub texture: Option<Arc<Texture>>,
}

#[derive(Clone, Debug, RustState, StateElement)]
pub struct StableIcon {
    pub item_id: i32,
    pub name: String,
    #[hidden_element]
    pub texture: Option<Arc<Texture>>,
}

#[derive(Clone, Debug, RustState, StateElement)]
pub struct StableHomunculusRow {
    pub id: i32,
    pub lineage: String,
    pub title: String,
    pub detail: String,
    pub active: bool,
}

#[derive(Clone, Debug, RustState, StateElement)]
pub struct StablePetRow {
    pub id: i32,
    pub title: String,
    pub detail: String,
    pub name: String,
    pub species: String,
    pub class_id: i32,
    pub level: i16,
    pub intimacy: i16,
    pub hunger: i16,
    pub attack: i16,
    pub attack2: i16,
    pub attack_speed: i16,
    pub experience: i32,
    pub next_experience: i32,
    pub egg: bool,
    pub active: bool,
    pub selected: bool,
    pub renamed: bool,
    pub owner: String,
    pub buff: String,
    pub equip_item_id: i32,
    pub food_item_id: i32,
    pub egg_item_id: i32,
    pub accessory_item_id: i32,
    pub icon_item_id: i32,
}

#[derive(Clone, Debug, RustState, StateElement)]
pub struct StableMaterialRow {
    pub item_id: i32,
    pub label: String,
    pub qty: String,
    pub enough: bool,
    pub header: bool,
}

#[derive(Clone, Debug, RustState, StateElement)]
pub struct OwnedCount {
    pub item_id: i32,
    pub owned: i32,
}

#[derive(Clone, Debug, RustState, StateElement)]
pub struct StableWindowState {
    status: String,
    pets_tab: bool,
    homunculi: Vec<StableHomunculusRow>,
    pets: Vec<StablePetRow>,
    owned: Vec<OwnedCount>,
    materials: Vec<StableMaterialRow>,
    icons: Vec<StableIcon>,
    selected_id: i32,
    detail_open: bool,
    detail_name: String,
    detail_species: String,
    detail_level: String,
    detail_exp: String,
    detail_attack: String,
    detail_aspd: String,
    detail_hunger: String,
    detail_intimacy: String,
    detail_accessory: String,
    /// Equipped accessory item, or 0 when the slot is empty.
    accessory_item_id: i32,
    /// Species accessory the Equip button looks up in the inventory.
    species_accessory_id: i32,
    detail_buff: String,
    /// 0 plain, 1 `Active:`, 2 `At Loyal:` / `When out:`.
    buff_tone: u8,
    detail_food: String,
    detail_status: String,
    detail_rename: String,
    detail_owner: String,
    evolve_egg_id: i32,
    can_summon: bool,
    can_deposit: bool,
    can_to_egg: bool,
    can_feed: bool,
    can_equip: bool,
    can_unequip: bool,
    can_evolve: bool,
}

impl Default for StableWindowState {
    fn default() -> Self {
        Self {
            status: "Homunculi and cute pets on this account.".to_owned(),
            pets_tab: false,
            homunculi: Vec::new(),
            pets: Vec::new(),
            owned: Vec::new(),
            materials: Vec::new(),
            icons: Vec::new(),
            selected_id: 0,
            detail_open: false,
            detail_name: String::new(),
            detail_species: String::new(),
            detail_level: String::new(),
            detail_exp: String::new(),
            detail_attack: String::new(),
            detail_aspd: String::new(),
            detail_hunger: String::new(),
            detail_intimacy: String::new(),
            detail_accessory: String::new(),
            accessory_item_id: 0,
            species_accessory_id: 0,
            detail_buff: String::new(),
            buff_tone: 0,
            detail_food: String::new(),
            detail_status: String::new(),
            detail_rename: String::new(),
            detail_owner: String::new(),
            evolve_egg_id: 0,
            can_summon: false,
            can_deposit: false,
            can_to_egg: false,
            can_feed: false,
            can_equip: false,
            can_unequip: false,
            can_evolve: false,
        }
    }
}

impl StableWindowState {
    pub fn show_homunculi(&mut self, active_id: i32, entries: &[StableHomunculusEntry]) {
        self.homunculi = entries
            .iter()
            .map(|entry| {
                let lineage = homunculus_lineage(i32::from(entry.class_id), i32::from(entry.previous_class));
                let active = entry.homunculus_id == active_id && active_id != 0;
                let mut title = if entry.name.is_empty() {
                    lineage.to_owned()
                } else {
                    entry.name.clone()
                };
                title.push_str(" · ");
                title.push_str(lineage);
                if active {
                    title.push_str(" · Active");
                }
                StableHomunculusRow {
                    id: entry.homunculus_id,
                    lineage: lineage.to_owned(),
                    title,
                    detail: format!(
                        "Lv {} · {} · Hunger {} · {}",
                        entry.level,
                        homunculus_intimacy(entry.intimacy),
                        entry.hunger,
                        lineage
                    ),
                    active,
                }
            })
            .collect();
    }

    pub fn show_pets(&mut self, active_id: i32, entries: &[StablePetEntry], faces: &[ItemFace]) {
        self.remember_faces(faces);
        let selected = self.selected_id;
        self.pets = entries.iter().map(|entry| pet_row(entry, active_id, selected)).collect();
        if !self.pets.iter().any(|pet| pet.id == self.selected_id) {
            self.selected_id = 0;
        }
        self.rebuild_detail();
    }

    pub fn show_owned(&mut self, items: &[StableOwnedEntry], faces: &[ItemFace]) {
        self.remember_faces(faces);
        self.owned = items
            .iter()
            .map(|item| OwnedCount {
                item_id: item.item_id,
                owned: item.owned,
            })
            .collect();
        self.rebuild_detail();
    }

    pub fn show_result(&mut self, result: i16, id: i32) {
        self.status = format_stable_result(result, id);
    }

    pub fn select_pet(&mut self, pet_id: i32) {
        self.selected_id = pet_id;
        for pet in &mut self.pets {
            pet.selected = pet.id == pet_id;
        }
        self.rebuild_detail();
    }

    pub fn set_item_texture(&mut self, item_id: i32, texture: Arc<Texture>) {
        if let Some(icon) = self.icons.iter_mut().find(|icon| icon.item_id == item_id) {
            icon.texture = Some(texture);
        }
    }

    pub fn remember_faces(&mut self, faces: &[ItemFace]) {
        for face in faces {
            if face.item_id <= 0 {
                continue;
            }
            if let Some(icon) = self.icons.iter_mut().find(|icon| icon.item_id == face.item_id) {
                if !face.name.is_empty() {
                    icon.name.clone_from(&face.name);
                }
                if face.texture.is_some() {
                    icon.texture.clone_from(&face.texture);
                }
            } else {
                self.icons.push(StableIcon {
                    item_id: face.item_id,
                    name: face.name.clone(),
                    texture: face.texture.clone(),
                });
            }
        }
    }

    pub fn store_homunculus_request(&self) -> StableRequest {
        StableRequest::Homunculus(HOMUNCULUS_STORE)
    }

    pub fn swap_homunculus_request(&self, homunculus_id: i32) -> Option<StableRequest> {
        let row = self.homunculi.iter().find(|row| row.id == homunculus_id)?;
        if row.active || homunculus_id == 0 {
            return None;
        }
        Some(StableRequest::Homunculus(homunculus_id))
    }

    pub fn store_pet_request(&self) -> StableRequest {
        StableRequest::Pet { action: PET_STORE, id: 0 }
    }

    pub fn summon_request(&self) -> Option<StableRequest> {
        self.action_request(self.can_summon, PET_SUMMON, self.selected_id)
    }

    pub fn deposit_request(&self) -> Option<StableRequest> {
        self.action_request(self.can_deposit, PET_DEPOSIT_EGG, self.selected_id)
    }

    pub fn to_egg_request(&self) -> Option<StableRequest> {
        self.action_request(self.can_to_egg, PET_TO_EGG, self.selected_id)
    }

    pub fn feed_request(&self) -> Option<StableRequest> {
        self.action_request(self.can_feed, PET_FEED, self.selected_id)
    }

    pub fn unequip_request(&self) -> Option<StableRequest> {
        self.action_request(self.can_unequip, PET_UNEQUIP, self.selected_id)
    }

    pub fn evolve_request(&self) -> Option<StableRequest> {
        self.action_request(self.can_evolve, PET_EVOLVE, self.evolve_egg_id)
    }

    /// `inventory_index` is the client index already stored on the item
    /// (`server + 2`).
    pub fn equip_request(&self, inventory_index: i32) -> Option<StableRequest> {
        if !self.can_equip || inventory_index < 0 {
            return None;
        }
        Some(StableRequest::Pet {
            action: PET_EQUIP,
            id: inventory_index,
        })
    }

    pub fn icon_name(&self, item_id: i32) -> String {
        if item_id <= 0 {
            return "None".to_owned();
        }
        self.icons
            .iter()
            .find(|icon| icon.item_id == item_id)
            .map(|icon| icon.name.clone())
            .filter(|name| !name.is_empty() && name != "NOTFOUND")
            .unwrap_or_else(|| format!("Item {item_id}"))
    }

    fn action_request(&self, enabled: bool, action: i16, id: i32) -> Option<StableRequest> {
        if !enabled || id == 0 {
            return None;
        }
        Some(StableRequest::Pet { action, id })
    }

    fn owned_of(&self, item_id: i32) -> i32 {
        self.owned
            .iter()
            .find(|row| row.item_id == item_id)
            .map(|row| row.owned)
            .unwrap_or(0)
    }

    fn rebuild_detail(&mut self) {
        let Some(pet) = self.pets.iter().find(|pet| pet.id == self.selected_id).cloned() else {
            self.detail_open = false;
            self.materials.clear();
            self.can_summon = false;
            self.can_deposit = false;
            self.can_to_egg = false;
            self.can_feed = false;
            self.can_equip = false;
            self.can_unequip = false;
            self.can_evolve = false;
            self.evolve_egg_id = 0;
            self.accessory_item_id = 0;
            self.species_accessory_id = 0;
            return;
        };

        self.detail_open = true;
        self.detail_name = pet.name.clone();
        self.detail_species = format!("{} ({})", pet.species, pet.class_id);
        self.detail_level = pet.level.to_string();
        self.detail_exp = exp_line(pet.experience, pet.next_experience);
        self.detail_attack = format!("{} ~ {}", pet.attack, pet.attack2);
        self.detail_aspd = pet.attack_speed.to_string();
        self.detail_hunger = format!("{} ({}/100)", hunger_label(pet.hunger), pet.hunger);
        self.detail_intimacy = format!("{} ({}/1000)", pet_intimacy(pet.intimacy), pet.intimacy);
        let shown_accessory = if pet.equip_item_id > 0 { pet.equip_item_id } else { 0 };
        self.accessory_item_id = shown_accessory;
        self.species_accessory_id = pet.accessory_item_id;
        self.detail_accessory = if shown_accessory > 0 {
            self.icon_name(shown_accessory)
        } else if pet.accessory_item_id > 0 {
            format!("None · {}", self.icon_name(pet.accessory_item_id))
        } else {
            "None".to_owned()
        };
        let buff = pet.buff.trim();
        self.buff_tone = buff_tone(buff);
        self.detail_buff = if buff.is_empty() {
            "No owner bonus".to_owned()
        } else {
            buff.to_owned()
        };
        self.detail_food = self.icon_name(pet.food_item_id);
        self.detail_status = pet_place(&pet);
        self.detail_rename = if pet.renamed { "Yes".to_owned() } else { "No".to_owned() };
        self.detail_owner = if pet.owner.is_empty() {
            "—".to_owned()
        } else {
            pet.owner.clone()
        };

        self.materials.clear();
        let mut evolve_ready = false;
        let mut evolve_egg_id = 0;
        if let Some(species) = pet_species(pet.class_id) {
            if !species.evolutions.is_empty() {
                evolve_ready = true;
            }
            for recipe in species.evolutions {
                if evolve_egg_id == 0 {
                    evolve_egg_id = recipe.target_egg_id;
                }
                let target = self.icon_name(recipe.target_egg_id);
                self.materials.push(StableMaterialRow {
                    item_id: recipe.target_egg_id,
                    label: format!("Into {target}"),
                    qty: String::new(),
                    enough: true,
                    header: true,
                });
                for &(item_id, amount) in recipe.materials {
                    let owned = self.owned_of(item_id);
                    let enough = owned >= amount;
                    if !enough {
                        evolve_ready = false;
                    }
                    self.materials.push(StableMaterialRow {
                        item_id,
                        label: self.icon_name(item_id),
                        qty: format!("{owned}/{amount}"),
                        enough,
                        header: false,
                    });
                }
            }
        }
        let loyal = pet.intimacy >= PET_LOYAL;
        self.evolve_egg_id = evolve_egg_id;
        self.can_deposit = pet.egg;
        self.can_summon = !pet.egg && !pet.active;
        self.can_to_egg = !pet.egg;
        self.can_feed = pet.active && !pet.egg;
        self.can_equip = pet.active && !pet.egg && pet.equip_item_id == 0 && pet.accessory_item_id > 0;
        self.can_unequip = pet.active && !pet.egg && pet.equip_item_id > 0;
        self.can_evolve = pet.active && !pet.egg && loyal && evolve_ready && evolve_egg_id != 0;
    }
}

pub fn pet_face_ids(pets: &[StablePetEntry]) -> Vec<i32> {
    let mut ids = Vec::new();
    for pet in pets {
        push_id(&mut ids, pet.egg_item_id);
        push_id(&mut ids, pet.equip_item_id);
        push_id(&mut ids, pet.food_item_id);
        let Some(species) = pet_species(pet.class_id) else {
            continue;
        };
        if pet.food_item_id <= 0 {
            push_id(&mut ids, species.food_item_id);
        }
        push_id(&mut ids, species.accessory_item_id);
        push_id(&mut ids, species.egg_item_id);
        for recipe in species.evolutions {
            push_id(&mut ids, recipe.target_egg_id);
            for material in recipe.materials {
                push_id(&mut ids, material.0);
            }
        }
    }
    ids
}

pub fn homunculus_lineage(class_id: i32, previous_class: i32) -> &'static str {
    let class = if matches!(class_id, 6048..=6052) && previous_class > 0 {
        previous_class
    } else {
        class_id
    };
    match class {
        6001 | 6005 | 6009 | 6013 | 6048 => "Lif",
        6002 | 6006 | 6010 | 6014 | 6049 => "Amistr",
        6003 | 6007 | 6011 | 6015 | 6051 => "Filir",
        6004 | 6008 | 6012 | 6016 | 6050 => "Vanilmirth",
        6052 => "Eleanor",
        _ => "Homunculus",
    }
}

fn homunculus_intimacy(value: i32) -> &'static str {
    if value >= 91_000 {
        "Loyal"
    } else if value >= 75_000 {
        "Cordial"
    } else if value >= 25_000 {
        "Neutral"
    } else if value >= 10_000 {
        "Shy"
    } else {
        "Awkward"
    }
}

fn pet_intimacy(value: i16) -> &'static str {
    if value >= 900 {
        "Loyal"
    } else if value >= 600 {
        "Cordial"
    } else if value >= 250 {
        "Neutral"
    } else if value >= 100 {
        "Shy"
    } else {
        "Awkward"
    }
}

fn hunger_label(value: i16) -> &'static str {
    if value < 10 {
        "Very hungry"
    } else if value < 25 {
        "Hungry"
    } else if value < 75 {
        "Normal"
    } else if value < 90 {
        "Satisfied"
    } else {
        "Stuffed"
    }
}

pub fn buff_tone(text: &str) -> u8 {
    let text = text.trim();
    if text.len() >= 7 && text[..7].eq_ignore_ascii_case("active:") {
        1
    } else if text.len() >= 9 && (text[..9].eq_ignore_ascii_case("at loyal:") || text[..9].eq_ignore_ascii_case("when out:")) {
        2
    } else {
        0
    }
}

fn exp_line(exp: i32, next: i32) -> String {
    if next <= 0 {
        "Max".to_owned()
    } else {
        let pct = (i64::from(exp) * 100 / i64::from(next)).clamp(0, 100);
        format!("{exp} / {next} ({pct}%)")
    }
}

fn pet_place(pet: &StablePetRow) -> String {
    if pet.egg {
        "Legacy egg on this character".to_owned()
    } else if pet.active {
        if pet.owner.is_empty() {
            "Out".to_owned()
        } else {
            format!("Out · {}", pet.owner)
        }
    } else {
        "In account stable".to_owned()
    }
}

fn pet_row(entry: &StablePetEntry, active_id: i32, selected: i32) -> StablePetRow {
    let egg = entry.flags & PET_FLAG_EGG != 0;
    let active = !egg && entry.pet_id == active_id && active_id != 0;
    let species_info = pet_species(entry.class_id);
    let species = species_info.map(|species| species.name).unwrap_or("Pet");
    let accessory_item_id = species_info.map(|species| species.accessory_item_id).unwrap_or(0);
    let food_item_id = if entry.food_item_id > 0 {
        entry.food_item_id
    } else {
        species_info.map(|species| species.food_item_id).unwrap_or(0)
    };
    let mut title = if entry.name.is_empty() {
        species.to_owned()
    } else {
        entry.name.clone()
    };
    if active || entry.flags & PET_FLAG_OUT != 0 && !egg {
        title.push_str(" · Out");
    }
    let icon_item_id = if entry.egg_item_id > 0 {
        entry.egg_item_id
    } else if entry.equip_item_id > 0 {
        entry.equip_item_id
    } else {
        accessory_item_id
    };
    StablePetRow {
        id: entry.pet_id,
        title,
        detail: format!("Lv {level} · {species}", level = entry.level),
        name: entry.name.clone(),
        species: species.to_owned(),
        class_id: entry.class_id,
        level: entry.level,
        intimacy: entry.intimacy,
        hunger: entry.hunger,
        attack: entry.attack,
        attack2: entry.attack2,
        attack_speed: entry.attack_speed,
        experience: entry.experience,
        next_experience: entry.next_experience,
        egg,
        active,
        selected: entry.pet_id == selected,
        renamed: entry.rename_flag != 0,
        owner: entry.owner_name.clone(),
        buff: entry.buff.clone(),
        equip_item_id: entry.equip_item_id,
        food_item_id,
        egg_item_id: entry.egg_item_id,
        accessory_item_id,
        icon_item_id,
    }
}

fn push_id(ids: &mut Vec<i32>, item_id: i32) {
    if item_id > 0 && !ids.contains(&item_id) {
        ids.push(item_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(item_id: i32, name: &str) -> ItemFace {
        ItemFace {
            item_id,
            name: name.to_owned(),
            texture: None,
        }
    }

    fn homun(id: i32, class_id: i16, previous: i16, name: &str) -> StableHomunculusEntry {
        StableHomunculusEntry {
            homunculus_id: id,
            class_id,
            previous_class: previous,
            level: 12,
            intimacy: 91_000,
            hunger: 40,
            vaporize: 0,
            rename_flag: 0,
            name: name.to_owned(),
        }
    }

    fn pet(id: i32, class_id: i32, flags: u8, intimacy: i16, equip: i32) -> StablePetEntry {
        StablePetEntry {
            pet_id: id,
            class_id,
            level: 5,
            intimacy,
            hunger: 80,
            egg_item_id: 9001,
            equip_item_id: equip,
            character_id: 1,
            incubate: 0,
            rename_flag: 0,
            flags,
            name: "Poki".to_owned(),
            owner_name: "Ada".to_owned(),
            attack: 3,
            attack2: 7,
            attack_speed: 150,
            experience: 10,
            next_experience: 50,
            food_item_id: 531,
            buff: "Active: LUK +3".to_owned(),
        }
    }

    fn opcode(bytes: &[u8]) -> u16 {
        u16::from_le_bytes([bytes[0], bytes[1]])
    }

    #[test]
    fn each_button_encodes_its_action_packet() {
        let open = StableRequest::Open.encode();
        assert_eq!(open, vec![0xF0, 0x0E]);

        let store_homun = StableRequest::Homunculus(HOMUNCULUS_STORE).encode();
        assert_eq!(opcode(&store_homun), 0x0EF1);
        assert_eq!(store_homun.len(), 6);
        assert_eq!(&store_homun[2..6], &0i32.to_le_bytes());

        let swap = StableRequest::Homunculus(15).encode();
        assert_eq!(&swap[2..6], &15i32.to_le_bytes());

        let cases = [
            (PET_STORE, 0, "store"),
            (PET_SUMMON, 9, "summon"),
            (PET_DEPOSIT_EGG, 9, "deposit"),
            (PET_TO_EGG, 9, "withdraw egg"),
            (PET_FEED, 9, "feed"),
            (PET_EQUIP, 14, "equip"),
            (PET_UNEQUIP, 9, "unequip"),
            (PET_EVOLVE, 9069, "evolve"),
        ];
        for (action, id, label) in cases {
            let bytes = StableRequest::Pet { action, id }.encode();
            assert_eq!(opcode(&bytes), 0x0EF2, "{label}");
            assert_eq!(bytes.len(), 8, "{label}");
            assert_eq!(&bytes[2..4], &action.to_le_bytes(), "{label}");
            assert_eq!(&bytes[4..8], &id.to_le_bytes(), "{label}");
        }
    }

    #[test]
    fn state_gates_buttons_and_evolution_counts() {
        let mut state = StableWindowState::default();
        state.show_homunculi(4, &[homun(4, 6001, 0, "Alpha"), homun(8, 6050, 6004, "Beta")]);
        assert_eq!(state.homunculi[0].lineage, "Lif");
        assert!(state.homunculi[0].active);
        assert!(state.swap_homunculus_request(4).is_none());
        assert_eq!(state.swap_homunculus_request(8), Some(StableRequest::Homunculus(8)));
        assert_eq!(state.homunculi[1].lineage, "Vanilmirth");
        assert_eq!(state.store_homunculus_request(), StableRequest::Homunculus(0));

        let faces = [
            face(9001, "Poring Egg"),
            face(531, "Apple Juice"),
            face(10013, "Backpack"),
            face(610, "Leaf Of Yggdrasil"),
            face(619, "Unripe Apple"),
            face(9069, "Mastering Egg"),
        ];
        state.show_pets(
            9,
            &[pet(9, 1002, PET_FLAG_OUT, 900, 0), pet(3, 1002, PET_FLAG_EGG, 100, 0)],
            &faces,
        );
        state.show_owned(
            &[StableOwnedEntry { item_id: 610, owned: 10 }, StableOwnedEntry {
                item_id: 619,
                owned: 3,
            }],
            &faces,
        );
        state.select_pet(9);
        assert!(state.detail_open);
        assert_eq!(state.detail_buff, "Active: LUK +3");
        assert_eq!(state.buff_tone, 1);
        assert_eq!(state.feed_request(), Some(StableRequest::Pet { action: PET_FEED, id: 9 }));
        assert!(state.summon_request().is_none());
        assert!(state.deposit_request().is_none());
        assert_eq!(state.to_egg_request(), Some(StableRequest::Pet { action: PET_TO_EGG, id: 9 }));
        assert_eq!(
            state.evolve_request(),
            Some(StableRequest::Pet {
                action: PET_EVOLVE,
                id: 9069
            })
        );
        assert_eq!(state.equip_request(14), Some(StableRequest::Pet { action: PET_EQUIP, id: 14 }));
        assert!(state.unequip_request().is_none());
        let leaf = state.materials.iter().find(|row| row.item_id == 610).unwrap();
        assert_eq!(leaf.qty, "10/10");
        assert!(leaf.enough);
        assert_eq!(state.store_pet_request().encode()[2], PET_STORE as u8);

        state.select_pet(3);
        assert_eq!(
            state.deposit_request(),
            Some(StableRequest::Pet {
                action: PET_DEPOSIT_EGG,
                id: 3
            })
        );
        assert!(state.feed_request().is_none());
        assert!(state.evolve_request().is_none());
        assert_eq!(state.detail_status, "Legacy egg on this character");

        state.show_result(11, 9);
        assert_eq!(state.status, "No pet food in inventory or storage.");
        state.show_result(13, 9);
        assert_eq!(state.status, "Evolution failed (loyalty, materials, or recipe).");
    }

    #[test]
    fn short_materials_block_evolve_and_pending_buff_is_marked() {
        let mut state = StableWindowState::default();
        let mut stored = pet(9, 1002, 0, 900, 10013);
        stored.buff = "At Loyal: LUK +2".to_owned();
        state.show_pets(0, &[stored], &[face(610, "Leaf"), face(619, "Apple"), face(10013, "Backpack")]);
        state.show_owned(&[StableOwnedEntry { item_id: 610, owned: 1 }], &[]);
        state.select_pet(9);
        assert!(state.evolve_request().is_none());
        assert!(state.summon_request().is_some());
        assert_eq!(state.buff_tone, 2);
        let leaf = state.materials.iter().find(|row| row.item_id == 610).unwrap();
        assert_eq!(leaf.qty, "1/10");
        assert!(!leaf.enough);
        assert_eq!(state.unequip_request(), None, "not the active pet");
    }
}
