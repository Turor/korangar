//! Item tooltip lines and Shift-to-compare against worn gear.
//!
//! The line builder is pure so unit tests and e2e can assert it without a
//! window. Colors are applied later, when the tooltip is painted.
//!
//! Sell price stays empty for the real Item Finder catalog: that JSON has no
//! price field (`r` is refineable, not zeny). A caller can still set
//! [`ItemTooltipFacts::sell_price`] when a table actually carries one.
//! Random-option names come from the client Lua tables when those files are
//! in the GRF. Otherwise the line is `Option #<id> +<value>`.

use std::collections::HashMap;

use korangar_loaders::FileLoader;
use korangar_networking::{InventoryItem, InventoryItemDetails};
use mlua::Lua;
use ragnarok_packets::{EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions, RegularItemFlags};

use crate::loaders::GameFileLoader;
use crate::world::{ResourceMetadata, execute_lua_chunk};

/// Turoran forge metadata stored in an option slot. Not a player-facing roll.
const FORGE_OPTION: u16 = 0x7FFE;
const FORGE_CARD: u32 = 0x00FF;
const CREATED_CARD: u32 = 0x00FE;
const PET_CARD: u32 = 0xFF00;
const BELOVED_PET: u32 = 0x0001;

const OPTION_FILES: &[&str] = &[
    r"data\luafiles514\lua files\datainfo\enumvar.lub",
    "data/luafiles514/lua files/datainfo/enumvar.lub",
    r"data\luafiles514\lua files\datainfo\addrandomoptionnametable.turoran.lub",
    "data/luafiles514/lua files/datainfo/addrandomoptionnametable.turoran.lub",
    r"data\luafiles514\lua files\datainfo\addrandomoptionnametable.lub",
    "data/luafiles514/lua files/datainfo/addrandomoptionnametable.lub",
];

/// Whether a line belongs to the hovered item or the worn compare block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineRole {
    Item,
    Compare,
}

/// Tone applied when the line is painted. Gold is refine and grade. Accent is
/// the "Equipped" header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipTone {
    Body,
    Gold,
    Accent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TooltipPart {
    pub tone: TooltipTone,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TooltipLine {
    pub role: LineRole,
    pub parts: Vec<TooltipPart>,
}

/// Rows the Item Finder catalog actually carries, plus an optional sell price.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemTooltipFacts {
    pub name: String,
    pub slots: i32,
    pub weight: i32,
    pub equip_level: i32,
    pub jobs: String,
    pub sell_price: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemTooltipCatalog {
    items: HashMap<u32, ItemTooltipFacts>,
    options: HashMap<u16, String>,
}

impl ItemTooltipCatalog {
    pub fn get(&self, id: u32) -> Option<&ItemTooltipFacts> {
        self.items.get(&id)
    }

    pub fn insert_item(&mut self, id: u32, name: String, slots: i32, weight: i32, equip_level: i32, jobs: String) {
        self.items.insert(id, ItemTooltipFacts {
            name,
            slots,
            weight,
            equip_level,
            jobs,
            sell_price: None,
        });
    }

    pub fn set_sell_price(&mut self, id: u32, price: i32) {
        if let Some(facts) = self.items.get_mut(&id) {
            facts.sell_price = Some(price);
        }
    }

    pub fn set_option_template(&mut self, id: u16, template: impl Into<String>) {
        self.options.insert(id, template.into());
    }

    pub fn set_options(&mut self, options: HashMap<u16, String>) {
        self.options = options;
    }

    pub fn take_options(&mut self) -> HashMap<u16, String> {
        std::mem::take(&mut self.options)
    }

    fn option_template(&self, id: u16) -> Option<&str> {
        self.options.get(&id).map(String::as_str)
    }
}

/// Shift is recorded every frame so a focused inventory window can still see
/// it. The hotbar path only reads Shift when the interface does not have focus.
#[derive(Clone, Debug, Default, korangar_interface::element::StateElement, rust_state::RustState)]
pub struct ItemCompareState {
    pub shift_held: bool,
}

/// Best-effort option names from the client data path. A missing file leaves
/// the map empty and tooltips use the `Option #id` fallback.
pub fn load_option_names(loader: &GameFileLoader) -> HashMap<u16, String> {
    let mut chunks = Vec::new();
    for path in OPTION_FILES {
        if !loader.file_exists(path) {
            continue;
        }
        if let Ok(bytes) = FileLoader::get(loader, path) {
            chunks.push(bytes);
        }
    }
    let views: Vec<&[u8]> = chunks.iter().map(Vec::as_slice).collect();
    option_names_from_chunks(&views)
}

fn option_names_from_chunks(chunks: &[&[u8]]) -> HashMap<u16, String> {
    let lua = Lua::new();
    for (index, chunk) in chunks.iter().enumerate() {
        let _ = execute_lua_chunk(&lua, &format!("random-option-{index}"), chunk);
    }
    read_option_names(&lua)
}

fn read_option_names(lua: &Lua) -> HashMap<u16, String> {
    let mut names = HashMap::new();
    let globals = lua.globals();
    for key in ["NameTable_VAR", "RandomOption"] {
        if let Ok(table) = globals.get::<mlua::Table>(key) {
            absorb_option_table(&table, &mut names);
        }
    }
    if names.is_empty()
        && let Ok(table) = globals.get::<mlua::Table>("tbl")
    {
        absorb_option_table(&table, &mut names);
    }
    names
}

fn absorb_option_table(table: &mlua::Table, names: &mut HashMap<u16, String>) {
    for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
        let Some(id) = lua_u16(pair.0) else { continue };
        let Some(text) = lua_text(pair.1) else { continue };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        names.insert(id, text.to_owned());
    }
}

fn lua_u16(value: mlua::Value) -> Option<u16> {
    let number = match value {
        mlua::Value::Integer(number) if number > 0 => number,
        mlua::Value::Number(number) if number.is_finite() && number > 0.0 && number <= u16::MAX as f64 => number as i64,
        _ => return None,
    };
    u16::try_from(number).ok()
}

fn lua_text(value: mlua::Value) -> Option<String> {
    match value {
        mlua::Value::String(text) => lua_owned_string(text),
        _ => None,
    }
}

fn lua_owned_string(value: mlua::String) -> Option<String> {
    let bytes = value.as_bytes();
    if let Ok(text) = std::str::from_utf8(bytes.as_ref()) {
        return Some(text.to_owned());
    }
    encoding_rs::EUC_KR
        .decode_without_bom_handling_and_without_replacement(bytes.as_ref())
        .map(|text| text.into_owned())
}

/// Tooltip lines for `item`. `worn` is the player inventory (equipped rows are
/// picked by position). Non-equippable items and an empty overlap produce no
/// compare block.
pub fn item_tooltip_lines(
    item: &InventoryItem<ResourceMetadata>,
    catalog: &ItemTooltipCatalog,
    worn: &[InventoryItem<ResourceMetadata>],
) -> Vec<TooltipLine> {
    let mut lines = Vec::new();
    push_item_lines(&mut lines, item, catalog, LineRole::Item);
    let matches = compare_targets(item, worn);
    if matches.is_empty() {
        return lines;
    }
    lines.push(TooltipLine {
        role: LineRole::Compare,
        parts: vec![part(TooltipTone::Accent, "Equipped")],
    });
    for (index, worn_item) in matches.iter().enumerate() {
        if index > 0 {
            lines.push(TooltipLine {
                role: LineRole::Compare,
                parts: Vec::new(),
            });
        }
        push_item_lines(&mut lines, worn_item, catalog, LineRole::Compare);
    }
    lines
}

/// Worn inventory rows whose equipped position overlaps `item`'s equip slot.
/// The hovered stack is left out when it is itself worn. A storage or cart
/// index that collides with an inventory index still matches, because that
/// stack is not equipped.
pub fn compare_targets<'a>(
    item: &InventoryItem<ResourceMetadata>,
    worn: &'a [InventoryItem<ResourceMetadata>],
) -> Vec<&'a InventoryItem<ResourceMetadata>> {
    let Some(equip_position) = comparable_position(item) else {
        return Vec::new();
    };
    worn.iter()
        .filter(|other| {
            let equipped = equipped_position(other);
            overlaps(equip_position, equipped) && !same_worn_stack(item, other)
        })
        .collect()
}

fn comparable_position(item: &InventoryItem<ResourceMetadata>) -> Option<EquipPosition> {
    if !item.is_identified() {
        return None;
    }
    match &item.details {
        InventoryItemDetails::Equippable { equip_position, .. } if equip_position.bits() != 0 => Some(*equip_position),
        _ => None,
    }
}

fn equipped_position(item: &InventoryItem<ResourceMetadata>) -> EquipPosition {
    match &item.details {
        InventoryItemDetails::Equippable { equipped_position, .. } => *equipped_position,
        InventoryItemDetails::Regular { equipped_position, .. } => *equipped_position,
    }
}

fn overlaps(equip_position: EquipPosition, equipped_position: EquipPosition) -> bool {
    equip_position.bits() & equipped_position.bits() != 0
}

fn same_worn_stack(item: &InventoryItem<ResourceMetadata>, other: &InventoryItem<ResourceMetadata>) -> bool {
    item.index == other.index && item.item_id == other.item_id && equipped_position(item).bits() != 0
}

fn push_item_lines(lines: &mut Vec<TooltipLine>, item: &InventoryItem<ResourceMetadata>, catalog: &ItemTooltipCatalog, role: LineRole) {
    let facts = catalog.get(item.item_id.0);
    let name = display_name(item, facts);
    if !item.is_identified() {
        lines.push(body_line(role, name));
        push_description(lines, item, role);
        return;
    }

    let mut parts = Vec::new();
    if let InventoryItemDetails::Equippable {
        refinement_level,
        enchantment_level,
        ..
    } = &item.details
    {
        if *refinement_level > 0 {
            parts.push(part(TooltipTone::Gold, format!("+{refinement_level}")));
        }
        if *enchantment_level > 0 {
            parts.push(part(TooltipTone::Gold, format!("Grade {enchantment_level}")));
        }
    }
    let slots = facts.map(|facts| facts.slots).unwrap_or(0);
    let show_slots = slots > 0 && !slots_hidden(item);
    let name = if show_slots { format!("{name} [{slots}]") } else { name };
    parts.push(part(TooltipTone::Body, name));
    lines.push(TooltipLine { role, parts });

    if !slots_hidden(item) {
        for card_id in card_ids(item, slots) {
            lines.push(body_line(role, card_name(catalog, card_id)));
        }
    }
    for option_line in option_lines(item, catalog) {
        lines.push(body_line(role, option_line));
    }
    if let Some(facts) = facts {
        lines.push(body_line(role, format!("Weight: {}", facts.weight)));
        if facts.equip_level > 0 {
            lines.push(body_line(role, format!("Required Level: {}", facts.equip_level)));
        }
        if !facts.jobs.is_empty() {
            lines.push(body_line(role, format!("Jobs: {}", facts.jobs)));
        }
        if let Some(price) = facts.sell_price {
            lines.push(body_line(role, format!("Sell: {price}z")));
        }
    }
    if let InventoryItemDetails::Regular { amount, .. } = &item.details {
        lines.push(body_line(role, format!("Quantity: {amount}")));
    }
    push_description(lines, item, role);
}

fn push_description(lines: &mut Vec<TooltipLine>, item: &InventoryItem<ResourceMetadata>, role: LineRole) {
    if !item.metadata.description.is_empty() {
        lines.push(body_line(role, item.metadata.description.clone()));
    }
}

fn display_name(item: &InventoryItem<ResourceMetadata>, facts: Option<&ItemTooltipFacts>) -> String {
    let raw = item.metadata.name.trim();
    let missing = raw.is_empty() || raw == "NOTFOUND";
    if !item.is_identified() {
        return if missing {
            "Unknown item".to_owned()
        } else {
            item.metadata.name.clone()
        };
    }
    if !missing {
        return item.metadata.name.clone();
    }
    facts
        .map(|facts| facts.name.trim())
        .filter(|name| !name.is_empty() && *name != "NOTFOUND")
        .map(str::to_owned)
        .unwrap_or_else(|| "Unknown item".to_owned())
}

fn slots_hidden(item: &InventoryItem<ResourceMetadata>) -> bool {
    item.slot[0] == FORGE_CARD || item.slot[0] == CREATED_CARD || item.slot[0] == PET_CARD || item.slot[3] == BELOVED_PET
}

fn card_ids(item: &InventoryItem<ResourceMetadata>, slots: i32) -> Vec<u32> {
    let limit = if slots > 0 { (slots as usize).min(4) } else { 4 };
    item.slot
        .iter()
        .copied()
        .take(limit)
        .filter(|id| *id > 1 && *id != FORGE_CARD && *id != CREATED_CARD && *id != PET_CARD)
        .collect()
}

fn card_name(catalog: &ItemTooltipCatalog, id: u32) -> String {
    catalog
        .get(id)
        .map(|facts| facts.name.trim())
        .filter(|name| !name.is_empty() && *name != "NOTFOUND")
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Card #{id}"))
}

fn option_lines(item: &InventoryItem<ResourceMetadata>, catalog: &ItemTooltipCatalog) -> Vec<String> {
    let InventoryItemDetails::Equippable {
        option_count, option_data, ..
    } = &item.details
    else {
        return Vec::new();
    };
    let entries: Vec<&ItemOptions> = if *option_count > 0 {
        option_data.iter().take(*option_count as usize).collect()
    } else {
        option_data.iter().filter(|option| option.index != 0).collect()
    };
    entries
        .into_iter()
        .filter(|option| option.index != 0 && option.index != FORGE_OPTION)
        .map(|option| match catalog.option_template(option.index) {
            Some(template) => format_option(template, option.value),
            None => format!("Option #{} +{}", option.index, option.value),
        })
        .collect()
}

fn format_option(template: &str, value: u16) -> String {
    if !template.contains("%d") {
        return template.to_owned();
    }
    let escaped = template.replace("%%", "\u{E000}");
    escaped.replace("%d", &value.to_string()).replace('\u{E000}', "%")
}

fn part(tone: TooltipTone, text: impl Into<String>) -> TooltipPart {
    TooltipPart { tone, text: text.into() }
}

fn body_line(role: LineRole, text: impl Into<String>) -> TooltipLine {
    TooltipLine {
        role,
        parts: vec![part(TooltipTone::Body, text)],
    }
}

/// Plain text of one role, for tests and the offline e2e fixture.
pub fn tooltip_plain(lines: &[TooltipLine], role: LineRole) -> String {
    lines
        .iter()
        .filter(|line| line.role == role)
        .map(line_text)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn line_text(line: &TooltipLine) -> String {
    line.parts
        .iter()
        .map(|part| part.text.as_str())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Offline fixture: refined slotted weapon, worn armor, potion, unidentified
/// item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineTooltipReport {
    pub weapon: String,
    pub armor_compare: String,
    pub potion: String,
    pub unidentified: String,
}

pub fn offline_tooltip_view() -> OfflineTooltipReport {
    let mut catalog = ItemTooltipCatalog::default();
    catalog.insert_item(1101, "Sword".to_owned(), 2, 50, 4, "Swordsman classes".to_owned());
    catalog.set_sell_price(1101, 120);
    catalog.insert_item(4001, "Poring Card".to_owned(), 0, 1, 0, String::new());
    catalog.insert_item(4002, "Drops Card".to_owned(), 0, 1, 0, String::new());
    catalog.insert_item(2301, "Cotton Shirt".to_owned(), 1, 10, 1, "Every Job".to_owned());
    catalog.set_option_template(3, "STR +%d");

    let weapon = Gear::identified("Sword", 1101)
        .index(4)
        .refine(7)
        .grade(2)
        .slots([4001, 4002, 0, 0])
        .equip(EquipPosition::RIGHT_HAND)
        .option(3, 5)
        .description("A sharp blade.")
        .build();
    let worn = Gear::identified("Cotton Shirt", 2301)
        .index(2)
        .kind(4)
        .refine(5)
        .equip(EquipPosition::ARMOR)
        .worn(EquipPosition::ARMOR)
        .build();
    let unworn_armor = Gear::identified("Adventure Suit", 2305)
        .index(8)
        .kind(4)
        .equip(EquipPosition::ARMOR)
        .build();
    let potion = stack("Red Potion", "Restores HP.", 501, 1, 12);
    let unidentified = Gear::identified("Unidentified Sword", 1101)
        .index(9)
        .refine(7)
        .grade(2)
        .slots([4001, 4002, 0, 0])
        .equip(EquipPosition::RIGHT_HAND)
        .option(3, 5)
        .description("You can't tell.")
        .unidentified()
        .build();

    let weapon_lines = item_tooltip_lines(&weapon, &catalog, std::slice::from_ref(&worn));
    let armor_lines = item_tooltip_lines(&unworn_armor, &catalog, std::slice::from_ref(&worn));
    let potion_lines = item_tooltip_lines(&potion, &catalog, std::slice::from_ref(&worn));
    let unidentified_lines = item_tooltip_lines(&unidentified, &catalog, &[]);
    let potion_item = tooltip_plain(&potion_lines, LineRole::Item);
    let potion_compare = tooltip_plain(&potion_lines, LineRole::Compare);

    OfflineTooltipReport {
        weapon: tooltip_plain(&weapon_lines, LineRole::Item),
        armor_compare: tooltip_plain(&armor_lines, LineRole::Compare),
        potion: [potion_item, potion_compare]
            .into_iter()
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        unidentified: tooltip_plain(&unidentified_lines, LineRole::Item),
    }
}

struct Gear {
    name: String,
    description: String,
    id: u32,
    index: u16,
    item_type: u8,
    refine: u8,
    grade: u8,
    slot: [u32; 4],
    equip: EquipPosition,
    equipped: EquipPosition,
    options: [ItemOptions; 5],
    option_count: u8,
    identified: bool,
}

impl Gear {
    fn identified(name: &str, id: u32) -> Self {
        Self {
            name: name.to_owned(),
            description: String::new(),
            id,
            index: 1,
            item_type: 5,
            refine: 0,
            grade: 0,
            slot: [0; 4],
            equip: EquipPosition::empty(),
            equipped: EquipPosition::empty(),
            options: std::array::from_fn(|_| ItemOptions {
                index: 0,
                value: 0,
                parameter: 0,
            }),
            option_count: 0,
            identified: true,
        }
    }

    fn index(mut self, index: u16) -> Self {
        self.index = index;
        self
    }

    fn kind(mut self, item_type: u8) -> Self {
        self.item_type = item_type;
        self
    }

    fn refine(mut self, level: u8) -> Self {
        self.refine = level;
        self
    }

    fn grade(mut self, level: u8) -> Self {
        self.grade = level;
        self
    }

    fn slots(mut self, slot: [u32; 4]) -> Self {
        self.slot = slot;
        self
    }

    fn equip(mut self, position: EquipPosition) -> Self {
        self.equip = position;
        self
    }

    fn worn(mut self, position: EquipPosition) -> Self {
        self.equipped = position;
        self
    }

    fn option(mut self, index: u16, value: u16) -> Self {
        let slot = self.option_count as usize;
        if slot < self.options.len() {
            self.options[slot] = ItemOptions {
                index,
                value,
                parameter: 0,
            };
            self.option_count += 1;
        }
        self
    }

    fn description(mut self, text: &str) -> Self {
        self.description = text.to_owned();
        self
    }

    fn unidentified(mut self) -> Self {
        self.identified = false;
        self
    }

    fn build(self) -> InventoryItem<ResourceMetadata> {
        let flags = if self.identified {
            EquippableItemFlags::IDENTIFIED
        } else {
            EquippableItemFlags::empty()
        };
        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: self.name,
                description: self.description,
            },
            index: InventoryIndex(self.index),
            item_id: ItemId(self.id),
            item_type: self.item_type,
            slot: self.slot,
            hire_expiration_date: 0,
            details: InventoryItemDetails::Equippable {
                equip_position: self.equip,
                equipped_position: self.equipped,
                bind_on_equip_type: 0,
                w_item_sprite_number: 0,
                option_count: self.option_count,
                option_data: self.options,
                forge_data: None,
                refinement_level: self.refine,
                enchantment_level: self.grade,
                flags,
            },
        }
    }
}

fn stack(name: &str, description: &str, id: u32, index: u16, amount: u16) -> InventoryItem<ResourceMetadata> {
    InventoryItem {
        metadata: ResourceMetadata {
            texture: None,
            name: name.to_owned(),
            description: description.to_owned(),
        },
        index: InventoryIndex(index),
        item_id: ItemId(id),
        item_type: 0,
        slot: [0; 4],
        hire_expiration_date: 0,
        details: InventoryItemDetails::Regular {
            amount,
            equipped_position: EquipPosition::empty(),
            flags: RegularItemFlags::IDENTIFIED,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_view_covers_refine_cards_option_weight_and_compare() {
        let view = offline_tooltip_view();
        assert!(view.weapon.contains("+7"), "{}", view.weapon);
        assert!(view.weapon.contains("Grade 2"), "{}", view.weapon);
        assert!(view.weapon.contains("Sword [2]"), "{}", view.weapon);
        assert!(view.weapon.contains("Poring Card"), "{}", view.weapon);
        assert!(view.weapon.contains("Drops Card"), "{}", view.weapon);
        assert!(view.weapon.contains("STR +5"), "{}", view.weapon);
        assert!(view.weapon.contains("Weight: 50"), "{}", view.weapon);
        assert!(view.weapon.contains("Required Level: 4"), "{}", view.weapon);
        assert!(view.weapon.contains("Jobs: Swordsman classes"), "{}", view.weapon);
        assert!(view.weapon.contains("Sell: 120z"), "{}", view.weapon);
        assert!(view.weapon.contains("A sharp blade."), "{}", view.weapon);
        assert!(!view.weapon.contains("Equipped"), "{}", view.weapon);
        assert!(!view.weapon.contains("Quantity:"), "{}", view.weapon);

        assert!(view.armor_compare.contains("Equipped"), "{}", view.armor_compare);
        assert!(view.armor_compare.contains("+5"), "{}", view.armor_compare);
        assert!(view.armor_compare.contains("Cotton Shirt"), "{}", view.armor_compare);

        assert!(view.potion.contains("Red Potion"), "{}", view.potion);
        assert!(view.potion.contains("Quantity: 12"), "{}", view.potion);
        assert!(view.potion.contains("Restores HP."), "{}", view.potion);
        assert!(!view.potion.contains("Equipped"), "{}", view.potion);
        assert!(!view.potion.contains("Cotton Shirt"), "{}", view.potion);

        assert_eq!(view.unidentified, "Unidentified Sword\nYou can't tell.");
    }

    #[test]
    fn accessories_list_both_worn_sides_and_storage_index_still_matches() {
        let mut catalog = ItemTooltipCatalog::default();
        catalog.insert_item(2601, "Clip".to_owned(), 0, 1, 0, String::new());
        catalog.insert_item(2602, "Rosary".to_owned(), 0, 1, 0, String::new());
        let left = Gear::identified("Clip", 2601)
            .index(1)
            .equip(EquipPosition::LEFT_ACCESSORY)
            .worn(EquipPosition::LEFT_ACCESSORY)
            .build();
        let right = Gear::identified("Rosary", 2602)
            .index(2)
            .equip(EquipPosition::RIGTH_ACCESSORY)
            .worn(EquipPosition::RIGTH_ACCESSORY)
            .build();
        let hovered = Gear::identified("Glove", 2610)
            .index(3)
            .equip(EquipPosition::LEFT_RIGHT_ACCESSORY)
            .build();
        let lines = item_tooltip_lines(&hovered, &catalog, &[left, right]);
        let compare = tooltip_plain(&lines, LineRole::Compare);
        assert!(compare.contains("Equipped"), "{compare}");
        assert!(compare.contains("Clip"), "{compare}");
        assert!(compare.contains("Rosary"), "{compare}");

        let stored = Gear::identified("Sword", 1101).index(1).equip(EquipPosition::RIGHT_HAND).build();
        let worn_same_index = Gear::identified("Blade", 1201)
            .index(1)
            .equip(EquipPosition::RIGHT_HAND)
            .worn(EquipPosition::RIGHT_HAND)
            .build();
        let worn = [worn_same_index];
        let targets = compare_targets(&stored, &worn);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].metadata.name, "Blade");
    }

    #[test]
    fn forge_markers_and_missing_cards_follow_the_client_rules() {
        let mut catalog = ItemTooltipCatalog::default();
        catalog.insert_item(1101, "Sword".to_owned(), 2, 10, 0, String::new());
        catalog.set_option_template(4, "ATK +%d%%");
        let forged = Gear::identified("Sword", 1101)
            .slots([FORGE_CARD, 4001, 0, 0])
            .equip(EquipPosition::RIGHT_HAND)
            .option(FORGE_OPTION, 9)
            .option(4, 10)
            .option(8, 3)
            .build();
        let lines = item_tooltip_lines(&forged, &catalog, &[]);
        let text = tooltip_plain(&lines, LineRole::Item);
        assert!(text.contains("Sword"), "{text}");
        assert!(!text.contains("[2]"), "{text}");
        assert!(!text.contains("Card"), "{text}");
        assert!(text.contains("ATK +10%"), "{text}");
        assert!(text.contains("Option #8 +3"), "{text}");
        assert!(!text.contains("32766"), "{text}");

        let mut slotted = Gear::identified("Sword", 1101).slots([9999, 0, 0, 0]).build();
        if let InventoryItemDetails::Equippable { option_count, .. } = &mut slotted.details {
            *option_count = 0;
        }
        let text = tooltip_plain(&item_tooltip_lines(&slotted, &catalog, &[]), LineRole::Item);
        assert!(text.contains("Sword [2]"), "{text}");
        assert!(text.contains("Card #9999"), "{text}");
    }

    #[test]
    fn hovering_the_worn_piece_does_not_compare_with_itself() {
        let worn = Gear::identified("Cotton Shirt", 2301)
            .equip(EquipPosition::ARMOR)
            .worn(EquipPosition::ARMOR)
            .build();
        assert!(compare_targets(&worn, std::slice::from_ref(&worn)).is_empty());
    }

    #[test]
    fn option_lua_reads_name_table_var() {
        let names = option_names_from_chunks(&[b"VAR_STR = 3\nNameTable_VAR = { [VAR_STR] = 'STR +%d' }\n"]);
        assert_eq!(names.get(&3).map(String::as_str), Some("STR +%d"));
    }
}
