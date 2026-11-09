//! First-pass Turoran paperdoll for the equipment window.
//!
//! Inventory stays a separate 4×10 icon grid in `inventory.rs`. This window
//! does not absorb it. Costume and shadow positions keep display names for
//! later, but they are not placed on the paperdoll yet.

use korangar_interface::element::Element;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::EquipPosition;
use rust_state::{Path, PathExt, Selector};

use crate::ItemSource;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::ClientState;
use crate::state::theme::InterfaceThemeType;
use crate::world::ResourceMetadata;

/// One cell in the paperdoll grid. Empty cells reserve a column so head, shoes,
/// and ammo stay in the center.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaperdollCell {
    Empty,
    Slot(EquipPosition),
}

pub(crate) const PAPERDOLL_COLUMN_COUNT: usize = 3;

/// Top to bottom. Hands flank armor; accessories flank the garment.
/// Head, shoes, and ammo are center-biased. Costume and shadow gear are
/// omitted.
pub(crate) const PAPERDOLL_ROWS: [[PaperdollCell; PAPERDOLL_COLUMN_COUNT]; 7] = [
    [
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::HEAD_TOP),
        PaperdollCell::Empty,
    ],
    [
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::HEAD_MIDDLE),
        PaperdollCell::Empty,
    ],
    [
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::HEAD_LOWER),
        PaperdollCell::Empty,
    ],
    [
        PaperdollCell::Slot(EquipPosition::LEFT_HAND),
        PaperdollCell::Slot(EquipPosition::ARMOR),
        PaperdollCell::Slot(EquipPosition::RIGHT_HAND),
    ],
    [
        PaperdollCell::Slot(EquipPosition::LEFT_ACCESSORY),
        PaperdollCell::Slot(EquipPosition::GARMENT),
        PaperdollCell::Slot(EquipPosition::RIGTH_ACCESSORY),
    ],
    [
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::SHOES),
        PaperdollCell::Empty,
    ],
    [PaperdollCell::Empty, PaperdollCell::Slot(EquipPosition::AMMO), PaperdollCell::Empty],
];

/// Label shown under an equipment socket. Match order is the historical one:
/// combined masks such as [`EquipPosition::LEFT_RIGHT_HAND`] hit the earlier
/// single-hand arm.
pub(crate) fn equip_slot_label(equip_position: EquipPosition) -> &'static str {
    match equip_position {
        _ if equip_position.contains(EquipPosition::HEAD_LOWER) => "Head lower",
        _ if equip_position.contains(EquipPosition::HEAD_MIDDLE) => "Head middle",
        _ if equip_position.contains(EquipPosition::HEAD_TOP) => "Head top",
        _ if equip_position.contains(EquipPosition::RIGHT_HAND) => "Right hand",
        _ if equip_position.contains(EquipPosition::LEFT_HAND) => "Left hand",
        _ if equip_position.contains(EquipPosition::ARMOR) => "Armor",
        _ if equip_position.contains(EquipPosition::SHOES) => "Shoes",
        _ if equip_position.contains(EquipPosition::GARMENT) => "Garment",
        _ if equip_position.contains(EquipPosition::LEFT_ACCESSORY) => "Left accessory",
        _ if equip_position.contains(EquipPosition::RIGTH_ACCESSORY) => "Right accessory",
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_TOP) => "Costume head top",
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_MIDDLE) => "Costume head middle",
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_LOWER) => "Costume head lower",
        _ if equip_position.contains(EquipPosition::COSTUME_GARMENT) => "Costume garment",
        _ if equip_position.contains(EquipPosition::AMMO) => "Ammo",
        _ if equip_position.contains(EquipPosition::SHADOW_ARMOR) => "Shadow ammo",
        _ if equip_position.contains(EquipPosition::SHADOW_WEAPON) => "Shadow weapon",
        _ if equip_position.contains(EquipPosition::SHADOW_SHIELD) => "Shadow shield",
        _ if equip_position.contains(EquipPosition::SHADOW_SHOES) => "Shadow shoes",
        _ if equip_position.contains(EquipPosition::SHADOW_RIGHT_ACCESSORY) => "Shadow right accessory",
        _ if equip_position.contains(EquipPosition::SHADOW_LEFT_ACCESSORY) => "Shadow left accessory",
        _ if equip_position.contains(EquipPosition::LEFT_RIGHT_ACCESSORY) => "Accessory",
        _ if equip_position.contains(EquipPosition::LEFT_RIGHT_HAND) => "Two hand weapon",
        _ if equip_position.contains(EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY) => "Shadow accessory",
        _ => panic!("no display name for equip position"),
    }
}

/// Equip positions that have a socket on the first-pass paperdoll, top to
/// bottom and left to right. Empty cells are skipped.
pub(crate) fn shown_equip_positions() -> impl Iterator<Item = EquipPosition> {
    PAPERDOLL_ROWS.into_iter().flatten().filter_map(|cell| match cell {
        PaperdollCell::Slot(position) => Some(position),
        PaperdollCell::Empty => None,
    })
}

struct EquipmentPath<P> {
    equip_position: EquipPosition,
    path: P,
}

impl<P: Copy> Clone for EquipmentPath<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: Copy> Copy for EquipmentPath<P> {}

impl<P> Selector<ClientState, InventoryItem<ResourceMetadata>, false> for EquipmentPath<P>
where
    P: Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a InventoryItem<ResourceMetadata>> {
        self.follow(state)
    }
}

impl<P> Path<ClientState, InventoryItem<ResourceMetadata>, false> for EquipmentPath<P>
where
    P: Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    fn follow<'a>(&self, state: &'a ClientState) -> Option<&'a InventoryItem<ResourceMetadata>> {
        self.path.follow_safe(state).iter().find(|item| {
            if let InventoryItemDetails::Equippable { equipped_position, .. } = &item.details {
                return equipped_position.contains(self.equip_position);
            }

            false
        })
    }

    fn follow_mut<'a>(&self, state: &'a mut ClientState) -> Option<&'a mut InventoryItem<ResourceMetadata>> {
        self.path.follow_mut_safe(state).iter_mut().find(|item| {
            if let InventoryItemDetails::Equippable { equipped_position, .. } = item.details {
                return equipped_position.contains(self.equip_position);
            }

            false
        })
    }
}

pub struct EquipmentWindow<A> {
    items_path: A,
}

impl<A> EquipmentWindow<A> {
    pub fn new(items_path: A) -> Self {
        Self { items_path }
    }
}

impl<A> CustomWindow<ClientState> for EquipmentWindow<A>
where
    A: Copy + Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Equipment)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        fn equip_box(
            items_path: impl Path<ClientState, Vec<InventoryItem<ResourceMetadata>>>,
            equip_position: EquipPosition,
        ) -> impl Element<ClientState> {
            use korangar_components::item_box;
            use korangar_interface::prelude::*;

            let equipment_path = EquipmentPath {
                equip_position,
                path: items_path,
            };

            // Icon above its slot name. The label stays visible when the socket is empty.
            // `item_box` still owns click-drag and drop (`ItemSource::Equipment`).
            fragment! {
                gaps: 2.0,
                children: (
                    item_box! {
                        item_path: equipment_path,
                        source: ItemSource::Equipment { position: equip_position },
                    },
                    text! {
                        text: equip_slot_label(equip_position),
                        height: 16.0,
                        horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                ),
            }
        }

        fn gap() -> impl Element<ClientState> {
            use korangar_interface::prelude::*;

            fragment! {
                children: (),
            }
        }

        // Same cells as `PAPERDOLL_ROWS`. Heads, shoes, and ammo use a center column.
        window! {
            title: "Equipment",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 520.0,
            maximum_width: 600.0,
            elements: (
                split! {
                    gaps: theme().window().gaps(),
                    children: (gap(), equip_box(self.items_path, EquipPosition::HEAD_TOP), gap()),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (gap(), equip_box(self.items_path, EquipPosition::HEAD_MIDDLE), gap()),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (gap(), equip_box(self.items_path, EquipPosition::HEAD_LOWER), gap()),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        equip_box(self.items_path, EquipPosition::LEFT_HAND),
                        equip_box(self.items_path, EquipPosition::ARMOR),
                        equip_box(self.items_path, EquipPosition::RIGHT_HAND),
                    ),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        equip_box(self.items_path, EquipPosition::LEFT_ACCESSORY),
                        equip_box(self.items_path, EquipPosition::GARMENT),
                        equip_box(self.items_path, EquipPosition::RIGTH_ACCESSORY),
                    ),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (gap(), equip_box(self.items_path, EquipPosition::SHOES), gap()),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (gap(), equip_box(self.items_path, EquipPosition::AMMO), gap()),
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::EquipPosition;

    use super::{PAPERDOLL_COLUMN_COUNT, PAPERDOLL_ROWS, PaperdollCell, equip_slot_label, shown_equip_positions};

    #[test]
    fn paperdoll_shows_body_slots_and_both_accessories() {
        let shown: Vec<_> = shown_equip_positions().collect();

        assert_eq!(shown, vec![
            EquipPosition::HEAD_TOP,
            EquipPosition::HEAD_MIDDLE,
            EquipPosition::HEAD_LOWER,
            EquipPosition::LEFT_HAND,
            EquipPosition::ARMOR,
            EquipPosition::RIGHT_HAND,
            EquipPosition::LEFT_ACCESSORY,
            EquipPosition::GARMENT,
            EquipPosition::RIGTH_ACCESSORY,
            EquipPosition::SHOES,
            EquipPosition::AMMO,
        ]);
    }

    #[test]
    fn paperdoll_keeps_head_shoes_and_ammo_centered() {
        let center = |row: usize| PAPERDOLL_ROWS[row][1];

        assert_eq!(center(0), PaperdollCell::Slot(EquipPosition::HEAD_TOP));
        assert_eq!(center(1), PaperdollCell::Slot(EquipPosition::HEAD_MIDDLE));
        assert_eq!(center(2), PaperdollCell::Slot(EquipPosition::HEAD_LOWER));
        assert_eq!(center(5), PaperdollCell::Slot(EquipPosition::SHOES));
        assert_eq!(center(6), PaperdollCell::Slot(EquipPosition::AMMO));

        assert!(PAPERDOLL_ROWS.iter().all(|row| row.len() == PAPERDOLL_COLUMN_COUNT));
        assert_eq!(PAPERDOLL_ROWS[0][0], PaperdollCell::Empty);
        assert_eq!(PAPERDOLL_ROWS[0][2], PaperdollCell::Empty);
        assert_eq!(PAPERDOLL_ROWS[3][0], PaperdollCell::Slot(EquipPosition::LEFT_HAND));
        assert_eq!(PAPERDOLL_ROWS[3][2], PaperdollCell::Slot(EquipPosition::RIGHT_HAND));
        assert_eq!(PAPERDOLL_ROWS[4][0], PaperdollCell::Slot(EquipPosition::LEFT_ACCESSORY));
        assert_eq!(PAPERDOLL_ROWS[4][2], PaperdollCell::Slot(EquipPosition::RIGTH_ACCESSORY));
    }

    #[test]
    fn paperdoll_omits_costume_and_shadow_gear() {
        let shown: Vec<_> = shown_equip_positions().collect();
        let hidden = [
            EquipPosition::COSTUME_HEAD_TOP,
            EquipPosition::COSTUME_HEAD_MIDDLE,
            EquipPosition::COSTUME_HEAD_LOWER,
            EquipPosition::COSTUME_GARMENT,
            EquipPosition::SHADOW_ARMOR,
            EquipPosition::SHADOW_WEAPON,
            EquipPosition::SHADOW_SHIELD,
            EquipPosition::SHADOW_SHOES,
            EquipPosition::SHADOW_RIGHT_ACCESSORY,
            EquipPosition::SHADOW_LEFT_ACCESSORY,
            EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY,
        ];

        for position in hidden {
            assert!(!shown.contains(&position), "{position:?} is not on the first-pass paperdoll");
        }
    }

    #[test]
    fn every_shown_slot_has_a_stable_label() {
        let expected = [
            (EquipPosition::HEAD_TOP, "Head top"),
            (EquipPosition::HEAD_MIDDLE, "Head middle"),
            (EquipPosition::HEAD_LOWER, "Head lower"),
            (EquipPosition::LEFT_HAND, "Left hand"),
            (EquipPosition::RIGHT_HAND, "Right hand"),
            (EquipPosition::ARMOR, "Armor"),
            (EquipPosition::SHOES, "Shoes"),
            (EquipPosition::GARMENT, "Garment"),
            (EquipPosition::LEFT_ACCESSORY, "Left accessory"),
            (EquipPosition::RIGTH_ACCESSORY, "Right accessory"),
            (EquipPosition::AMMO, "Ammo"),
        ];

        for (position, label) in expected {
            assert_eq!(equip_slot_label(position), label);
        }

        for position in shown_equip_positions() {
            assert!(!equip_slot_label(position).is_empty());
        }
    }

    #[test]
    fn unshown_positions_keep_their_display_names() {
        assert_eq!(equip_slot_label(EquipPosition::COSTUME_HEAD_TOP), "Costume head top");
        assert_eq!(equip_slot_label(EquipPosition::COSTUME_GARMENT), "Costume garment");
        assert_eq!(equip_slot_label(EquipPosition::SHADOW_WEAPON), "Shadow weapon");
        // Pre-existing label. `SHADOW_ARMOR` is not "Shadow armor".
        assert_eq!(equip_slot_label(EquipPosition::SHADOW_ARMOR), "Shadow ammo");
        assert_eq!(equip_slot_label(EquipPosition::LEFT_RIGHT_HAND), "Right hand");
    }
}
