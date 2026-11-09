//! Gear window.
//!
//! Left half, top to bottom: equipment slots around a preview cell, a costume
//! strip, then saved switch-set loadouts. Right half: base stats with point
//! buttons and combat stats with a base + bonus split. Inventory stays in a
//! column further right. Base level and job level are not shown here.

use std::cell::{Cell, UnsafeCell};
use korangar_components::item_box;
use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{Element, ElementBox};
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::{InventoryItem, InventoryItemDetails};
use ragnarok_packets::{EquipPosition, StatUpType};
use rust_state::{ManuallyAssertExt, Path, PathExt, Selector, State, VecIndexExt};

use crate::ItemSource;
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::equip_switch::{EquipLoadoutPathExt, EquipSwitch, EquipSwitchPathExt};
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, this_player};
use crate::world::{PlayerPathExt, ResourceMetadata};

/// Gear does not repeat the character-overview level lines.
pub(crate) const GEAR_SHOWS_LEVELS: bool = false;
const _: () = assert!(!GEAR_SHOWS_LEVELS);

/// Combat rows on the right half, in display order.
pub(crate) const COMBAT_STAT_LABELS: [&str; 8] = ["ATK", "MATK", "DEF", "MDEF", "HIT", "FLEE", "CRIT", "ASPD"];
const _: usize = COMBAT_STAT_LABELS.len();

/// One cell in the equipment ring around the preview.
///
/// Layout contract for tests. The window body is hand-built so `gap` and
/// `equip_box` can share a `split!` row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaperdollCell {
    Preview,
    Slot(EquipPosition),
}

pub(crate) const PAPERDOLL_COLUMN_COUNT: usize = 3;

/// Costume sockets in one horizontal strip under the preview.
pub(crate) const COSTUME_SLOTS: [EquipPosition; 4] = [
    EquipPosition::COSTUME_HEAD_TOP,
    EquipPosition::COSTUME_HEAD_MIDDLE,
    EquipPosition::COSTUME_HEAD_LOWER,
    EquipPosition::COSTUME_GARMENT,
];

/// Shadow sockets kept available under the costume strip.
pub(crate) const SHADOW_SLOTS: [EquipPosition; 6] = [
    EquipPosition::SHADOW_WEAPON,
    EquipPosition::SHADOW_ARMOR,
    EquipPosition::SHADOW_SHIELD,
    EquipPosition::SHADOW_SHOES,
    EquipPosition::SHADOW_LEFT_ACCESSORY,
    EquipPosition::SHADOW_RIGHT_ACCESSORY,
];

/// Inventory grid reused from the standalone inventory window.
pub(crate) const INVENTORY_ROWS: usize = 4;
pub(crate) const INVENTORY_COLUMNS: usize = 10;

/// Body slots around the preview. Weapon and shield flank the preview cell.
pub(crate) const PAPERDOLL_ROWS: [[PaperdollCell; PAPERDOLL_COLUMN_COUNT]; 4] = [
    [
        PaperdollCell::Slot(EquipPosition::HEAD_TOP),
        PaperdollCell::Slot(EquipPosition::HEAD_MIDDLE),
        PaperdollCell::Slot(EquipPosition::HEAD_LOWER),
    ],
    [
        PaperdollCell::Slot(EquipPosition::LEFT_HAND),
        PaperdollCell::Preview,
        PaperdollCell::Slot(EquipPosition::RIGHT_HAND),
    ],
    [
        PaperdollCell::Slot(EquipPosition::LEFT_ACCESSORY),
        PaperdollCell::Slot(EquipPosition::ARMOR),
        PaperdollCell::Slot(EquipPosition::RIGTH_ACCESSORY),
    ],
    [
        PaperdollCell::Slot(EquipPosition::GARMENT),
        PaperdollCell::Slot(EquipPosition::SHOES),
        PaperdollCell::Slot(EquipPosition::AMMO),
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
    PAPERDOLL_ROWS
        .into_iter()
        .flatten()
        .filter_map(|cell| match cell {
            PaperdollCell::Slot(position) => Some(position),
            PaperdollCell::Preview => None,
        })
        .chain(COSTUME_SLOTS)
        .chain(SHADOW_SLOTS)
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

struct BonusText<A> {
    bonus_path: A,
    last_value: Cell<Option<i32>>,
    text: UnsafeCell<String>,
}

impl<A> BonusText<A> {
    fn new(bonus_path: A) -> Self {
        Self {
            bonus_path,
            last_value: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for BonusText<A>
where
    A: Path<ClientState, i32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let bonus = *self.bonus_path.follow_safe(state);
        if self.last_value.get() != Some(bonus) {
            unsafe {
                *self.text.get() = format!("^000001{bonus:+}^000000");
            }
            self.last_value.set(Some(bonus));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct PairText<A, B> {
    base_path: A,
    bonus_path: B,
    last_value: Cell<Option<(u32, u32)>>,
    text: UnsafeCell<String>,
}

impl<A, B> PairText<A, B> {
    fn new(base_path: A, bonus_path: B) -> Self {
        Self {
            base_path,
            bonus_path,
            last_value: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A, B> Selector<ClientState, String> for PairText<A, B>
where
    A: Path<ClientState, u32>,
    B: Path<ClientState, u32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let base = *self.base_path.follow_safe(state);
        let bonus = *self.bonus_path.follow_safe(state);
        if self.last_value.get() != Some((base, bonus)) {
            unsafe {
                *self.text.get() = format!("{base} ^000001+ {bonus}^000000");
            }
            self.last_value.set(Some((base, bonus)));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct CostText<A> {
    cost_path: A,
    last_value: Cell<Option<u8>>,
    text: UnsafeCell<String>,
}

impl<A> CostText<A> {
    fn new(cost_path: A) -> Self {
        Self {
            cost_path,
            last_value: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for CostText<A>
where
    A: Path<ClientState, u8>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let cost = *self.cost_path.follow_safe(state);
        if self.last_value.get() != Some(cost) {
            unsafe {
                *self.text.get() = match cost {
                    0 => "^000001max^000000".to_string(),
                    cost => format!("+ ({cost})"),
                };
            }
            self.last_value.set(Some(cost));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

/// Focus id for the loadout name box.
struct GearSetName;

struct ShowString<A> {
    path: A,
    text: UnsafeCell<String>,
}

impl<A> ShowString<A> {
    fn new(path: A) -> Self {
        Self {
            path,
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for ShowString<A>
where
    A: Path<ClientState, String, false>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let value = self.path.follow(state)?;
        unsafe {
            if &*self.text.get() != value {
                *self.text.get() = value.clone();
            }
            Some(self.text.as_ref_unchecked())
        }
    }
}

/// Saved switch-set names. Each name wears that set in one click.
struct LoadoutStrip<A> {
    switch_path: A,
    elements: Vec<ElementBox<ClientState>>,
    shown: usize,
}

impl<A> LoadoutStrip<A> {
    fn new(switch_path: A) -> Self {
        Self {
            switch_path,
            elements: Vec::new(),
            shown: usize::MAX,
        }
    }
}

impl<A> Element<ClientState> for LoadoutStrip<A>
where
    A: Path<ClientState, EquipSwitch>,
{
    type LayoutInfo = ();

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        mut store: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| {
            use korangar_interface::prelude::*;

            let count = state.get(&self.switch_path.loadouts()).len();
            if count != self.shown {
                self.elements.clear();
                self.shown = count;
                if count == 0 {
                    self.elements.push(ErasedElement::new(text! {
                        text: "No saved sets.",
                        overflow_behavior: OverflowBehavior::Shrink,
                    }));
                } else {
                    for index in 0..count {
                        let switch_path = self.switch_path;
                        self.elements.push(ErasedElement::new(split! {
                            gaps: theme().window().gaps(),
                            children: (
                                text! {
                                    text: ShowString::new(switch_path.loadouts().index(index).name()),
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                button! {
                                    text: "Wear",
                                    event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                        state.update_value_with(switch_path.selected(), move |selected| *selected = index);
                                        queue.queue(InputEvent::ApplyEquipLoadout { index, wear: true });
                                    },
                                },
                            ),
                        }));
                    }
                }
            }

            self.elements.iter_mut().enumerate().for_each(|(index, element)| {
                element.create_layout_info(state, store.child_store(index as u64), resolver);
            });
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        store: ElementStore<'a>,
        _: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        self.elements.iter().enumerate().for_each(|(index, element)| {
            element.lay_out(state, store.child_store(index as u64), &(), layout);
        });
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

        fn disabled_cutoff<P, C>(stat_points_path: P, cost_path: C) -> impl Selector<ClientState, bool>
        where
            P: Path<ClientState, u32>,
            C: Path<ClientState, u8>,
        {
            ComputedSelector::new_default(move |state: &ClientState| {
                let stat_points = stat_points_path.follow_safe(state);
                let cost = cost_path.follow_safe(state);
                *cost == 0 || *stat_points < *cost as u32
            })
        }

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

        let items = self.items_path;
        let player = this_player().manually_asserted();
        let switch_set = crate::state::client_state().equip_switch();

        macro_rules! base_stat {
            ($label:literal, $value:ident, $bonus:ident, $cost:ident, $kind:ident) => {
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text! {
                            text: $label,
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PartialEqDisplaySelector::new(player.$value()),
                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: BonusText::new(player.$bonus()),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        button! {
                            text: CostText::new(player.$cost()),
                            disabled: disabled_cutoff(player.stat_points(), player.$cost()),
                            event: InputEvent::StatUp { stat_type: StatUpType::$kind { amount: 1 } },
                        },
                    ),
                }
            };
        }

        macro_rules! combat_pair {
            ($label:literal, $base:ident, $bonus:ident) => {
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        text! {
                            text: $label,
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        text! {
                            text: PairText::new(player.$base(), player.$bonus()),
                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                    ),
                }
            };
        }

        window! {
            title: "Gear",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 980.0,
            maximum_width: 1180.0,
            maximum_height: 560.0,
            elements: (
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
                                    children: (
                                        equip_box(items, EquipPosition::HEAD_TOP),
                                        equip_box(items, EquipPosition::HEAD_MIDDLE),
                                        equip_box(items, EquipPosition::HEAD_LOWER),
                                    ),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        equip_box(items, EquipPosition::LEFT_HAND),
                                        fragment! {
                                            gaps: 2.0,
                                            children: (
                                                text! {
                                                    text: "Preview",
                                                    height: 36.0,
                                                    horizontal_alignment: HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                                                    overflow_behavior: OverflowBehavior::Shrink,
                                                },
                                            ),
                                        },
                                        equip_box(items, EquipPosition::RIGHT_HAND),
                                    ),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        equip_box(items, EquipPosition::LEFT_ACCESSORY),
                                        equip_box(items, EquipPosition::ARMOR),
                                        equip_box(items, EquipPosition::RIGTH_ACCESSORY),
                                    ),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        equip_box(items, EquipPosition::GARMENT),
                                        equip_box(items, EquipPosition::SHOES),
                                        equip_box(items, EquipPosition::AMMO),
                                    ),
                                },
                                text! {
                                    text: "Costume",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        equip_box(items, EquipPosition::COSTUME_HEAD_TOP),
                                        equip_box(items, EquipPosition::COSTUME_HEAD_MIDDLE),
                                        equip_box(items, EquipPosition::COSTUME_HEAD_LOWER),
                                        equip_box(items, EquipPosition::COSTUME_GARMENT),
                                    ),
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
                                    children: (
                                        equip_box(items, EquipPosition::SHADOW_SHOES),
                                        equip_box(items, EquipPosition::SHADOW_LEFT_ACCESSORY),
                                        equip_box(items, EquipPosition::SHADOW_RIGHT_ACCESSORY),
                                    ),
                                },
                                text! {
                                    text: "Sets",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                text_box! {
                                    ghost_text: "Set name",
                                    state: switch_set.draft_name(),
                                    input_handler: DefaultHandler::<_, _, 24>::new(switch_set.draft_name(), |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {}),
                                    focus_id: GearSetName,
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        button! {
                                            text: "Save",
                                            event: InputEvent::SaveEquipLoadout,
                                        },
                                        button! {
                                            text: "Load",
                                            event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                                let index = *state.get(&switch_set.selected());
                                                queue.queue(InputEvent::ApplyEquipLoadout { index, wear: false });
                                            },
                                        },
                                        button! {
                                            text: "Rename",
                                            event: InputEvent::RenameEquipLoadout,
                                        },
                                    ),
                                },
                                LoadoutStrip::new(switch_set),
                            ),
                        },
                        fragment! {
                            gaps: theme().window().gaps(),
                            children: (
                                text! {
                                    text: "Base",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                text! {
                                    text: PartialEqDisplaySelector::new(player.stat_points()),
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                base_stat!("STR", strength, bonus_strength, strength_stat_points_cost, Strength),
                                base_stat!("AGI", agility, bonus_agility, agility_stat_points_cost, Agility),
                                base_stat!("VIT", vitality, bonus_vitality, vitality_stat_points_cost, Vitality),
                                base_stat!("INT", intelligence, bonus_intelligence, intelligence_stat_points_cost, Intelligence),
                                base_stat!("DEX", dexterity, bonus_dexterity, dexterity_stat_points_cost, Dexterity),
                                base_stat!("LUK", luck, bonus_luck, luck_stat_points_cost, Luck),
                                text! {
                                    text: "Combat",
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                combat_pair!("ATK", attack_base, attack_bonus),
                                combat_pair!("MATK", magic_attack_base, magic_attack_bonus),
                                combat_pair!("DEF", defense_base, defense_bonus),
                                combat_pair!("MDEF", magic_defense_base, magic_defense_bonus),
                                combat_pair!("FLEE", flee_base, flee_bonus),
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        text! {
                                            text: "HIT",
                                            overflow_behavior: OverflowBehavior::Shrink,
                                        },
                                        text! {
                                            text: PartialEqDisplaySelector::new(player.hit()),
                                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                                            overflow_behavior: OverflowBehavior::Shrink,
                                        },
                                    ),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        text! {
                                            text: "CRIT",
                                            overflow_behavior: OverflowBehavior::Shrink,
                                        },
                                        text! {
                                            text: PartialEqDisplaySelector::new(player.critical()),
                                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                                            overflow_behavior: OverflowBehavior::Shrink,
                                        },
                                    ),
                                },
                                split! {
                                    gaps: theme().window().gaps(),
                                    children: (
                                        text! {
                                            text: "ASPD",
                                            overflow_behavior: OverflowBehavior::Shrink,
                                        },
                                        text! {
                                            text: PartialEqDisplaySelector::new(player.attack_speed()),
                                            horizontal_alignment: HorizontalAlignment::Right { offset: 0.0, border: 3.0 },
                                            overflow_behavior: OverflowBehavior::Shrink,
                                        },
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
        COMBAT_STAT_LABELS, COSTUME_SLOTS, INVENTORY_COLUMNS, INVENTORY_ROWS, PAPERDOLL_COLUMN_COUNT, PAPERDOLL_ROWS,
        PaperdollCell, SHADOW_SLOTS, equip_slot_label, shown_equip_positions,
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
    fn preview_sits_between_weapon_and_shield() {
        assert_eq!(PAPERDOLL_ROWS[1][0], PaperdollCell::Slot(EquipPosition::LEFT_HAND));
        assert_eq!(PAPERDOLL_ROWS[1][1], PaperdollCell::Preview);
        assert_eq!(PAPERDOLL_ROWS[1][2], PaperdollCell::Slot(EquipPosition::RIGHT_HAND));
        assert!(PAPERDOLL_ROWS.iter().all(|row| row.len() == PAPERDOLL_COLUMN_COUNT));
        assert_eq!(PAPERDOLL_ROWS[0][0], PaperdollCell::Slot(EquipPosition::HEAD_TOP));
        assert_eq!(PAPERDOLL_ROWS[0][1], PaperdollCell::Slot(EquipPosition::HEAD_MIDDLE));
        assert_eq!(PAPERDOLL_ROWS[0][2], PaperdollCell::Slot(EquipPosition::HEAD_LOWER));
        assert_eq!(PAPERDOLL_ROWS[2][1], PaperdollCell::Slot(EquipPosition::ARMOR));
        assert_eq!(PAPERDOLL_ROWS[3][0], PaperdollCell::Slot(EquipPosition::GARMENT));
        assert_eq!(PAPERDOLL_ROWS[3][1], PaperdollCell::Slot(EquipPosition::SHOES));
        assert_eq!(COSTUME_SLOTS.len(), 4);
        assert_eq!(SHADOW_SLOTS.len(), 6);
        assert_eq!(
            COMBAT_STAT_LABELS,
            ["ATK", "MATK", "DEF", "MDEF", "HIT", "FLEE", "CRIT", "ASPD"]
        );
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
