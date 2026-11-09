//! Turoran paperdoll + inventory gear panel (Warbound-inspired layout goals
//! only).
//!
//! Left: paperdoll sockets (body, costume, shadow). Right: existing 4x10
//! inventory icon grid. Top: compact level/stat summary. No third-party art or
//! packet changes.

use korangar_components::item_box;
use korangar_interface::element::Element;
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::EquipPosition;
use rust_state::{ManuallyAssertExt, Path, PathExt, Selector, VecIndexExt};

use crate::ItemSource;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, this_player};
use crate::world::{PlayerPathExt, ResourceMetadata};

/// One cell in the paperdoll grid. Empty cells reserve a column so head, shoes,
/// and ammo stay in the center.
///
/// Layout contract for tests / future builders. The window body is hand-built
/// so `gap` and `equip_box` opaque element types can sit in the same `split!`
/// row.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaperdollCell {
    Empty,
    Slot(EquipPosition),
}

#[allow(dead_code)]
pub(crate) const PAPERDOLL_COLUMN_COUNT: usize = 3;

/// Inventory grid reused from the standalone inventory window.
pub(crate) const INVENTORY_ROWS: usize = 4;
pub(crate) const INVENTORY_COLUMNS: usize = 10;

/// Top to bottom. Body gear, then costume, then shadow.
#[allow(dead_code)]
pub(crate) const PAPERDOLL_ROWS: [[PaperdollCell; PAPERDOLL_COLUMN_COUNT]; 14] = [
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
    [
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::COSTUME_HEAD_TOP),
        PaperdollCell::Empty,
    ],
    [
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::COSTUME_HEAD_MIDDLE),
        PaperdollCell::Empty,
    ],
    [
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::COSTUME_HEAD_LOWER),
        PaperdollCell::Empty,
    ],
    [
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::COSTUME_GARMENT),
        PaperdollCell::Empty,
    ],
    [
        PaperdollCell::Slot(EquipPosition::SHADOW_WEAPON),
        PaperdollCell::Slot(EquipPosition::SHADOW_ARMOR),
        PaperdollCell::Slot(EquipPosition::SHADOW_SHIELD),
    ],
    [
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::SHADOW_SHOES),
        PaperdollCell::Empty,
    ],
    [
        PaperdollCell::Slot(EquipPosition::SHADOW_LEFT_ACCESSORY),
        PaperdollCell::Empty,
        PaperdollCell::Slot(EquipPosition::SHADOW_RIGHT_ACCESSORY),
    ],
];

/// Label shown under an equipment socket. Match order is historical.
pub(crate) fn equip_slot_label(equip_position: EquipPosition) -> &'static str {
    // More specific costume/shadow flags first so a bad/legacy multi-bit value
    // cannot fall through to a body-slot arm via `contains`.
    match equip_position {
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_TOP) => "Costume head top",
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_MIDDLE) => "Costume head middle",
        _ if equip_position.contains(EquipPosition::COSTUME_HEAD_LOWER) => "Costume head lower",
        _ if equip_position.contains(EquipPosition::COSTUME_GARMENT) => "Costume garment",
        _ if equip_position.contains(EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY) => "Shadow accessory",
        _ if equip_position.contains(EquipPosition::SHADOW_RIGHT_ACCESSORY) => "Shadow right accessory",
        _ if equip_position.contains(EquipPosition::SHADOW_LEFT_ACCESSORY) => "Shadow left accessory",
        _ if equip_position.contains(EquipPosition::SHADOW_WEAPON) => "Shadow weapon",
        _ if equip_position.contains(EquipPosition::SHADOW_SHIELD) => "Shadow shield",
        _ if equip_position.contains(EquipPosition::SHADOW_SHOES) => "Shadow shoes",
        _ if equip_position.contains(EquipPosition::SHADOW_ARMOR) => "Shadow ammo",
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
        _ if equip_position.contains(EquipPosition::AMMO) => "Ammo",
        _ if equip_position.contains(EquipPosition::LEFT_RIGHT_ACCESSORY) => "Accessory",
        _ if equip_position.contains(EquipPosition::LEFT_RIGHT_HAND) => "Two hand weapon",
        _ => panic!("no display name for equip position"),
    }
}

#[allow(dead_code)]
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
            let equipment_path = EquipmentPath {
                equip_position,
                path: items_path,
            };

            fragment! {
                gaps: 2.0,
                children: (
                    item_box! {
                        item_path: equipment_path,
                        source: ItemSource::Equipment { position: equip_position },
                    },
                    text! {
                        text: equip_slot_label(equip_position),
                        height: 14.0,
                        horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                        overflow_behavior: OverflowBehavior::Shrink,
                    },
                ),
            }
        }

        fn gap() -> impl Element<ClientState> {
            fragment! {
                children: (),
            }
        }

        let items = self.items_path;

        window! {
            title: "Gear",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 780.0,
            maximum_width: 920.0,
            elements: (
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text! {
                            text: "Base Lv",
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PartialEqDisplaySelector::new(this_player().manually_asserted().base_level()),
                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: "Job Lv",
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PartialEqDisplaySelector::new(this_player().manually_asserted().job_level()),
                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: "STR",
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PartialEqDisplaySelector::new(this_player().manually_asserted().strength()),
                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: "AGI",
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PartialEqDisplaySelector::new(this_player().manually_asserted().agility()),
                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: "VIT",
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PartialEqDisplaySelector::new(this_player().manually_asserted().vitality()),
                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                    ),
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        fragment! {
                            gaps: theme().window().gaps(),
                            children: (
                                text! {
                                    text: "Equipment",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::HEAD_TOP), gap()),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::HEAD_MIDDLE), gap()),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::HEAD_LOWER), gap()),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        equip_box(items, EquipPosition::LEFT_HAND),
                                        equip_box(items, EquipPosition::ARMOR),
                                        equip_box(items, EquipPosition::RIGHT_HAND),
                                    ),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        equip_box(items, EquipPosition::LEFT_ACCESSORY),
                                        equip_box(items, EquipPosition::GARMENT),
                                        equip_box(items, EquipPosition::RIGTH_ACCESSORY),
                                    ),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::SHOES), gap()),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::AMMO), gap()),
                                },
                                text! {
                                    text: "Costume",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::COSTUME_HEAD_TOP), gap()),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::COSTUME_HEAD_MIDDLE), gap()),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::COSTUME_HEAD_LOWER), gap()),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::COSTUME_GARMENT), gap()),
                                },
                                text! {
                                    text: "Shadow",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        equip_box(items, EquipPosition::SHADOW_WEAPON),
                                        equip_box(items, EquipPosition::SHADOW_ARMOR),
                                        equip_box(items, EquipPosition::SHADOW_SHIELD),
                                    ),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (gap(), equip_box(items, EquipPosition::SHADOW_SHOES), gap()),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        equip_box(items, EquipPosition::SHADOW_LEFT_ACCESSORY),
                                        gap(),
                                        equip_box(items, EquipPosition::SHADOW_RIGHT_ACCESSORY),
                                    ),
                                },
                            ),
                        },
                        fragment! {
                            gaps: theme().window().gaps(),
                            children: (
                                text! {
                                    text: "Inventory",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                fragment! {
                                    gaps: theme().window().gaps(),
                                    children: std::array::from_fn::<_, INVENTORY_ROWS, _>(|row| {
                                        split! {
                                            gaps: theme().window().gaps(),
                                            children: std::array::from_fn::<_, INVENTORY_COLUMNS, _>(|column| {
                                                let path = items.index(row * INVENTORY_COLUMNS + column);

                                                item_box! {
                                                    item_path: path,
                                                    source: ItemSource::Inventory,
                                                }
                                            }),
                                        }
                                    }),
                                },
                            ),
                        },
                    ),
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::EquipPosition;

    use super::{
        INVENTORY_COLUMNS, INVENTORY_ROWS, PAPERDOLL_COLUMN_COUNT, PAPERDOLL_ROWS, PaperdollCell, equip_slot_label, shown_equip_positions,
    };

    #[test]
    fn paperdoll_shows_body_slots_and_both_accessories() {
        let shown: Vec<_> = shown_equip_positions().collect();
        for position in [
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
        ] {
            assert!(shown.contains(&position), "missing body slot {position:?}");
        }
    }

    #[test]
    fn paperdoll_includes_costume_and_shadow_slots() {
        let shown: Vec<_> = shown_equip_positions().collect();
        for position in [
            EquipPosition::COSTUME_HEAD_TOP,
            EquipPosition::COSTUME_HEAD_MIDDLE,
            EquipPosition::COSTUME_HEAD_LOWER,
            EquipPosition::COSTUME_GARMENT,
            EquipPosition::SHADOW_WEAPON,
            EquipPosition::SHADOW_ARMOR,
            EquipPosition::SHADOW_SHIELD,
            EquipPosition::SHADOW_SHOES,
            EquipPosition::SHADOW_LEFT_ACCESSORY,
            EquipPosition::SHADOW_RIGHT_ACCESSORY,
        ] {
            assert!(shown.contains(&position), "missing costume/shadow slot {position:?}");
        }
        assert!(!shown.contains(&EquipPosition::SHADOW_LEFT_RIGHT_ACCESSORY));
    }

    #[test]
    fn paperdoll_keeps_head_shoes_and_ammo_centered() {
        let center = |row: usize| PAPERDOLL_ROWS[row][1];
        assert_eq!(center(0), PaperdollCell::Slot(EquipPosition::HEAD_TOP));
        assert_eq!(center(1), PaperdollCell::Slot(EquipPosition::HEAD_MIDDLE));
        assert_eq!(center(2), PaperdollCell::Slot(EquipPosition::HEAD_LOWER));
        assert_eq!(center(5), PaperdollCell::Slot(EquipPosition::SHOES));
        assert_eq!(center(6), PaperdollCell::Slot(EquipPosition::AMMO));
        assert_eq!(center(7), PaperdollCell::Slot(EquipPosition::COSTUME_HEAD_TOP));
        assert_eq!(center(11), PaperdollCell::Slot(EquipPosition::SHADOW_ARMOR));
        assert!(PAPERDOLL_ROWS.iter().all(|row| row.len() == PAPERDOLL_COLUMN_COUNT));
        assert_eq!(PAPERDOLL_ROWS[3][0], PaperdollCell::Slot(EquipPosition::LEFT_HAND));
        assert_eq!(PAPERDOLL_ROWS[3][2], PaperdollCell::Slot(EquipPosition::RIGHT_HAND));
        assert_eq!(PAPERDOLL_ROWS[4][0], PaperdollCell::Slot(EquipPosition::LEFT_ACCESSORY));
        assert_eq!(PAPERDOLL_ROWS[4][2], PaperdollCell::Slot(EquipPosition::RIGTH_ACCESSORY));
    }

    #[test]
    fn inventory_grid_dimensions_match_standalone_window() {
        assert_eq!(INVENTORY_ROWS, 4);
        assert_eq!(INVENTORY_COLUMNS, 10);
    }

    #[test]
    fn every_shown_slot_has_a_stable_label() {
        for position in shown_equip_positions() {
            assert!(!equip_slot_label(position).is_empty());
        }
        assert_eq!(equip_slot_label(EquipPosition::COSTUME_HEAD_TOP), "Costume head top");
        assert_eq!(equip_slot_label(EquipPosition::COSTUME_HEAD_LOWER), "Costume head lower");
        assert_eq!(equip_slot_label(EquipPosition::SHADOW_WEAPON), "Shadow weapon");
        assert_eq!(equip_slot_label(EquipPosition::SHADOW_ARMOR), "Shadow ammo");
        // LEFT_RIGHT_HAND includes RIGHT_HAND; historical body-slot preference.
        assert_eq!(equip_slot_label(EquipPosition::LEFT_RIGHT_HAND), "Right hand");
        assert_eq!(EquipPosition::COSTUME_HEAD_LOWER.bits(), 4096);
    }
}
