use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{InventoryItem, InventoryItemDetails, ShopItem};

use super::WindowClass;
use crate::graphics::Color;
use crate::interface::resource::ItemSource;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::ClientState;
use crate::state::theme::InterfaceThemeType;
use crate::world::ResourceMetadata;

/// Stable id so a second right-click focuses the existing details window.
pub fn item_details_key(source: ItemSource, item: &InventoryItem<ResourceMetadata>) -> u64 {
    let source_tag = match source {
        ItemSource::Inventory => 1,
        ItemSource::Cart => 2,
        ItemSource::Storage => 3,
        ItemSource::Equipment { position } => 16 + position.bits() as u64,
    };
    ((item.index.0 as u64) << 32) ^ ((item.item_id.0 as u64) << 8) ^ source_tag
}

#[allow(dead_code)]
pub fn shop_item_details_key(item: &ShopItem<ResourceMetadata>) -> u64 {
    0x8000_0000_0000_0000 | item.item_id.0 as u64
}

fn gold_markup(gold: Color) -> String {
    format!("^{:02X}{:02X}{:02X}", gold.red_as_u8(), gold.green_as_u8(), gold.blue_as_u8())
}

/// Hover tooltip and pinned-window body: name, gold refine/grade accents, then
/// description.
pub fn format_item_details(item: &InventoryItem<ResourceMetadata>, gold: Color) -> String {
    format_named_details(&item.metadata, Some(&item.details), None, gold)
}

#[allow(dead_code)]
pub fn format_shop_item_details(item: &ShopItem<ResourceMetadata>, gold: Color) -> String {
    let qty = match item.quantity {
        korangar_networking::ItemQuantity::Fixed(amount) => Some(amount as u16),
        korangar_networking::ItemQuantity::Infinite => None,
    };
    format_named_details(&item.metadata, None, qty, gold)
}

fn format_named_details(
    metadata: &ResourceMetadata,
    details: Option<&InventoryItemDetails>,
    shop_amount: Option<u16>,
    gold: Color,
) -> String {
    let mut lines = Vec::new();
    if metadata.name.is_empty() {
        lines.push("Unknown item".to_owned());
    } else {
        lines.push(metadata.name.clone());
    }

    let gold = gold_markup(gold);
    if let Some(InventoryItemDetails::Equippable {
        refinement_level,
        enchantment_level,
        ..
    }) = details
    {
        if *refinement_level > 0 {
            lines.push(format!("{gold}+{refinement_level}^000000"));
        }
        if *enchantment_level > 0 {
            lines.push(format!("{gold}Grade {enchantment_level}^000000"));
        }
    }
    if let Some(InventoryItemDetails::Regular { amount, .. }) = details {
        lines.push(format!("Quantity: {amount}"));
    }
    if let Some(amount) = shop_amount {
        lines.push(format!("Quantity: {amount}"));
    }
    if !metadata.description.is_empty() {
        lines.push(metadata.description.clone());
    }
    lines.join("\n")
}

/// Floating, closable, draggable item inspect window. Class is `None` so
/// several can be open.
pub struct ItemDetailsWindow {
    pub title: String,
    pub body: String,
}

impl CustomWindow<ClientState> for ItemDetailsWindow {
    fn window_class() -> Option<WindowClass> {
        None
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: self.title,
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 220.0,
            elements: (
                text! {
                    text: self.body,
                    font_size: FontSize(13.0),
                    overflow_behavior: OverflowBehavior::LineBreak,
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use korangar_networking::{InventoryItem, InventoryItemDetails};
    use ragnarok_packets::{EquipPosition, EquippableItemFlags, InventoryIndex, ItemId, ItemOptions, RegularItemFlags};

    use super::{format_item_details, item_details_key};
    use crate::graphics::Color;
    use crate::interface::resource::ItemSource;
    use crate::world::ResourceMetadata;

    fn option() -> ItemOptions {
        ItemOptions {
            index: 0,
            value: 0,
            parameter: 0,
        }
    }

    fn equip(name: &str, refine: u8, grade: u8, description: &str) -> InventoryItem<ResourceMetadata> {
        InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: name.to_owned(),
                description: description.to_owned(),
            },
            index: InventoryIndex(4),
            item_id: ItemId(1101),
            item_type: 5,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: InventoryItemDetails::Equippable {
                equip_position: EquipPosition::empty(),
                equipped_position: EquipPosition::empty(),
                bind_on_equip_type: 0,
                w_item_sprite_number: 0,
                option_count: 0,
                option_data: [option(), option(), option(), option(), option()],
                forge_data: None,
                refinement_level: refine,
                enchantment_level: grade,
                flags: EquippableItemFlags::empty(),
            },
        }
    }

    #[test]
    fn details_include_gold_refine_and_description() {
        let item = equip("Sword", 7, 2, "A sharp blade.");
        let text = format_item_details(&item, Color::rgb_u8(0xCF, 0xAE, 0x5A));
        assert!(text.starts_with("Sword"));
        assert!(text.contains("^CFAE5A+7^000000"));
        assert!(text.contains("^CFAE5AGrade 2^000000"));
        assert!(text.contains("A sharp blade."));
        assert!(!text.contains("Quantity:"));
    }

    #[test]
    fn regular_items_show_quantity() {
        let item = InventoryItem {
            metadata: ResourceMetadata {
                texture: None,
                name: "Red Potion".to_owned(),
                description: "Restores HP.".to_owned(),
            },
            index: InventoryIndex(1),
            item_id: ItemId(501),
            item_type: 0,
            slot: [0; 4],
            hire_expiration_date: 0,
            details: InventoryItemDetails::Regular {
                amount: 12,
                equipped_position: EquipPosition::empty(),
                flags: RegularItemFlags::IDENTIFIED,
            },
        };
        let text = format_item_details(&item, Color::rgb_u8(0xCF, 0xAE, 0x5A));
        assert!(text.contains("Quantity: 12"));
        assert!(text.contains("Restores HP."));
    }

    #[test]
    fn details_key_differs_by_source_and_index() {
        let item = equip("Sword", 0, 0, "");
        assert_ne!(
            item_details_key(ItemSource::Inventory, &item),
            item_details_key(ItemSource::Storage, &item)
        );
    }

    #[test]
    fn shop_details_include_quantity_and_key() {
        use korangar_networking::{ItemQuantity, ShopItem};

        use super::{format_shop_item_details, shop_item_details_key};
        let item = ShopItem {
            metadata: ResourceMetadata {
                texture: None,
                name: "Red Potion".to_owned(),
                description: "Restores HP.".to_owned(),
            },
            item_id: ItemId(501),
            item_type: 0,
            price: ragnarok_packets::Price(10),
            quantity: ItemQuantity::Fixed(7),
            weight: 0,
            location: 0,
        };
        let text = format_shop_item_details(&item, Color::rgb_u8(0xCF, 0xAE, 0x5A));
        assert!(text.contains("Red Potion"));
        assert!(text.contains("Quantity: 7"));
        assert!(shop_item_details_key(&item) != 0);
    }
}
