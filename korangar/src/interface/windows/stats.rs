use std::cell::{Cell, UnsafeCell};

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element, StateElement};
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, TextThemePathExt, VerticalAlignment};
use korangar_interface::theme::ThemePathGetter;
use korangar_interface::window::{CustomWindow, Window};
use ragnarok_packets::StatUpType;
use rust_state::{Path, PathExt, RustState, Selector, State};

use super::equipment::{display_aspd, display_packet_stat};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::OverflowBehavior;
use crate::state::localization::LocalizationPathExt;
use crate::state::theme::InterfaceThemeType;
use crate::state::{ClientState, ClientStatePathExt, client_state};
use crate::world::{BasicInfoStatsPathExt, Player, PlayerPathExt, format_armor_reduction};

struct PhysicalReductionTip;
struct MagicalReductionTip;

#[derive(Default, RustState, StateElement)]
pub struct StatsWindowState {
    show_traits: bool,
    notice: String,
}

struct StatTextSelector<A> {
    bonus_path: A,
    last_value: Cell<Option<i32>>,
    text: UnsafeCell<String>,
}

impl<A> StatTextSelector<A> {
    pub fn new(bonus_path: A) -> Self {
        Self {
            bonus_path,
            last_value: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for StatTextSelector<A>
where
    A: Path<ClientState, i32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let bonus_value = self.bonus_path.follow_safe(state);

        unsafe {
            let last_value = self.last_value.get();

            if last_value.is_none() || last_value.as_ref().is_some_and(|last| *last != *bonus_value) {
                *self.text.get() = format!("^000001{bonus_value:+}^000000");
                self.last_value.set(Some(*bonus_value));
            }
        }

        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct CostTextSelector<A> {
    cost_path: A,
    last_value: UnsafeCell<Option<u8>>,
    text: UnsafeCell<String>,
}

impl<A> CostTextSelector<A> {
    pub fn new(cost_path: A) -> Self {
        Self {
            cost_path,
            last_value: UnsafeCell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for CostTextSelector<A>
where
    A: Path<ClientState, u8>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let cost = self.cost_path.follow_safe(state);

        unsafe {
            let last_value = &mut *self.last_value.get();

            if last_value.is_none() || last_value.as_ref().is_some_and(|last| *last != *cost) {
                *self.text.get() = match *cost {
                    0 => "^000001max^000000".to_string(),
                    cost => format!("+1 (^000001{cost}^000000)"),
                };

                *last_value = Some(*cost);
            }
        }

        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct ValueLine<A> {
    label: &'static str,
    path: A,
    last: Cell<Option<u32>>,
    text: UnsafeCell<String>,
}

impl<A> ValueLine<A> {
    fn new(label: &'static str, path: A) -> Self {
        Self {
            label,
            path,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for ValueLine<A>
where
    A: Path<ClientState, u32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let value = display_packet_stat(*self.path.follow_safe(state));
        if self.last.get() != Some(value) {
            unsafe {
                *self.text.get() = format!("{} {value}", self.label);
            }
            self.last.set(Some(value));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct PairLine<A, B> {
    label: &'static str,
    base_path: A,
    bonus_path: B,
    last: Cell<Option<(u32, u32)>>,
    text: UnsafeCell<String>,
}

impl<A, B> PairLine<A, B> {
    fn new(label: &'static str, base_path: A, bonus_path: B) -> Self {
        Self {
            label,
            base_path,
            bonus_path,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A, B> Selector<ClientState, String> for PairLine<A, B>
where
    A: Path<ClientState, u32>,
    B: Path<ClientState, u32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let base = display_packet_stat(*self.base_path.follow_safe(state));
        let bonus = display_packet_stat(*self.bonus_path.follow_safe(state));
        if self.last.get() != Some((base, bonus)) {
            unsafe {
                *self.text.get() = format!("{} {base} ^000001+ {bonus}^000000", self.label);
            }
            self.last.set(Some((base, bonus)));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct AspdLine<A> {
    path: A,
    last: Cell<Option<u32>>,
    text: UnsafeCell<String>,
}

impl<A> AspdLine<A> {
    fn new(path: A) -> Self {
        Self {
            path,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for AspdLine<A>
where
    A: Path<ClientState, u32>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let amotion = *self.path.follow_safe(state);
        if self.last.get() != Some(amotion) {
            unsafe {
                *self.text.get() = format!("ASPD {}", display_aspd(amotion));
            }
            self.last.set(Some(amotion));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

/// Hard DEF or MDEF with the renewal reduction percent on hover.
struct ArmorLine<A, B> {
    label: &'static str,
    base_path: A,
    bonus_path: B,
    magical: bool,
    line: String,
    tip: String,
}

impl<A, B> ArmorLine<A, B> {
    fn new(label: &'static str, base_path: A, bonus_path: B, magical: bool) -> Self {
        Self {
            label,
            base_path,
            bonus_path,
            magical,
            line: String::new(),
            tip: String::new(),
        }
    }
}

impl<A, B> Element<ClientState> for ArmorLine<A, B>
where
    A: Path<ClientState, u32>,
    B: Path<ClientState, u32>,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let base = display_packet_stat(*state.get(&self.base_path));
        let bonus = display_packet_stat(*state.get(&self.bonus_path));
        let kind = if self.magical { "Magical reduction" } else { "Physical reduction" };
        self.line = format!("{} {base} ^000001+ {bonus}^000000", self.label);
        self.tip = format_armor_reduction(kind, base, self.magical);
        with_single_resolver(resolvers, |resolver| {
            let height = *state.get(&korangar_interface::theme::theme().text().height());
            BaseLayoutInfo {
                area: resolver.with_height(height),
            }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let font_size = *state.get(&korangar_interface::theme::theme().text().font_size());
        let color = *state.get(&korangar_interface::theme::theme().text().color());
        let highlight = *state.get(&korangar_interface::theme::theme().text().highlight_color());
        layout.add_text(
            layout_info.area,
            &self.line,
            font_size,
            color,
            highlight,
            HorizontalAlignment::Left { offset: 3.0, border: 3.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        if layout_info.area.check().run(layout) {
            let id = if self.magical {
                MagicalReductionTip.tooltip_id()
            } else {
                PhysicalReductionTip.tooltip_id()
            };
            layout.add_tooltip(&self.tip, id);
        }
    }
}

#[derive(Default)]
pub struct StatsWindow<A> {
    player_path: A,
}

impl<A> StatsWindow<A> {
    pub fn new(player_path: A) -> Self {
        Self { player_path }
    }
}

impl<A> CustomWindow<ClientState> for StatsWindow<A>
where
    A: Path<ClientState, Player>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Stats)
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

                // The cost is 0 if the the player is at the maximum level.
                *cost == 0 || *stat_points < *cost as u32
            })
        }

        macro_rules! stat_row {
            ($points:expr, $label:expr, $value:expr, $bonus:expr, $cost:expr, $event:expr) => {
                split! {
                    children: (
                        text! {
                            text: $label,
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                        split! {
                            children: (
                                text! {
                                    text: PartialEqDisplaySelector::new($value),
                                    horizontal_alignment: HorizontalAlignment::Right { offset: 5.0, border: 5.0 },
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                                text! {
                                    text: StatTextSelector::new($bonus),
                                    horizontal_alignment: HorizontalAlignment::Left { offset: 5.0, border: 5.0 },
                                    overflow_behavior: OverflowBehavior::Shrink,
                                },
                            ),
                        },
                        button! {
                            text: CostTextSelector::new($cost),
                            disabled: disabled_cutoff($points, $cost),
                            event: $event,
                        },
                    ),
                }
            };
        }

        let player = self.player_path;
        let traits = player.basic_info();

        window! {
            title: client_state().localization().stats_window_title(),
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 340.0,
            elements: (
                split! {
                    children: (
                        button! {
                            text: "Status",
                            event: SetToFalse(client_state().stats_window().show_traits()),
                        },
                        button! {
                            text: "Traits",
                            event: SetToTrue(client_state().stats_window().show_traits()),
                        },
                    ),
                },
                either! {
                    selector: client_state().stats_window().show_traits(),
                    on_true: fragment! {
                        gaps: theme().window().gaps(),
                        children: (
                            split! {
                                children: (
                                    text! {
                                        text: "Trait points",
                                        overflow_behavior: OverflowBehavior::Shrink,
                                    },
                                    text! {
                                        text: PartialEqDisplaySelector::new(traits.trait_points()),
                                        horizontal_alignment: HorizontalAlignment::Right { offset: 5.0, border: 5.0 },
                                        overflow_behavior: OverflowBehavior::Shrink,
                                    },
                                ),
                            },
                            // + stays on screen and disables while TraitPoint is 0 or below the cost.
                            stat_row!(traits.trait_points(), "POW", traits.power(), traits.bonus_power(), traits.power_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Power { amount: 1 } }),
                            stat_row!(traits.trait_points(), "STA", traits.stamina(), traits.bonus_stamina(), traits.stamina_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Stamina { amount: 1 } }),
                            stat_row!(traits.trait_points(), "WIS", traits.wisdom(), traits.bonus_wisdom(), traits.wisdom_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Wisdom { amount: 1 } }),
                            stat_row!(traits.trait_points(), "SPL", traits.spell(), traits.bonus_spell(), traits.spell_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Spell { amount: 1 } }),
                            stat_row!(traits.trait_points(), "CON", traits.concentration(), traits.bonus_concentration(), traits.concentration_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Concentration { amount: 1 } }),
                            stat_row!(traits.trait_points(), "CRT", traits.creativity(), traits.bonus_creativity(), traits.creativity_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Creativity { amount: 1 } }),
                            text! { text: ValueLine::new("P.ATK", traits.physical_attack()), overflow_behavior: OverflowBehavior::Shrink },
                            text! { text: ValueLine::new("S.MATK", traits.spell_magic_attack()), overflow_behavior: OverflowBehavior::Shrink },
                            text! { text: ValueLine::new("RES", traits.resistance()), overflow_behavior: OverflowBehavior::Shrink },
                            text! { text: ValueLine::new("MRES", traits.magic_resistance()), overflow_behavior: OverflowBehavior::Shrink },
                            text! { text: ValueLine::new("H.PLUS", traits.healing_plus()), overflow_behavior: OverflowBehavior::Shrink },
                            text! { text: ValueLine::new("C.RATE", traits.critical_damage_rate()), overflow_behavior: OverflowBehavior::Shrink },
                        ),
                    },
                    on_false: fragment! {
                        gaps: theme().window().gaps(),
                        children: (
                            split! {
                                children: (
                                    text! {
                                        text: client_state().localization().available_stat_points_text(),
                                        overflow_behavior: OverflowBehavior::Shrink,
                                    },
                                    text! {
                                        text: PartialEqDisplaySelector::new(player.stat_points()),
                                        horizontal_alignment: HorizontalAlignment::Right { offset: 5.0, border: 5.0 },
                                        overflow_behavior: OverflowBehavior::Shrink,
                                    },
                                ),
                            },
                            stat_row!(player.stat_points(), client_state().localization().strength_text(), player.strength(), player.bonus_strength(), player.strength_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Strength { amount: 1 } }),
                            stat_row!(player.stat_points(), client_state().localization().agility_text(), player.agility(), player.bonus_agility(), player.agility_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Agility { amount: 1 } }),
                            stat_row!(player.stat_points(), client_state().localization().vitality_text(), player.vitality(), player.bonus_vitality(), player.vitality_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Vitality { amount: 1 } }),
                            stat_row!(player.stat_points(), client_state().localization().intelligence_text(), player.intelligence(), player.bonus_intelligence(), player.intelligence_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Intelligence { amount: 1 } }),
                            stat_row!(player.stat_points(), client_state().localization().dexterity_text(), player.dexterity(), player.bonus_dexterity(), player.dexterity_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Dexterity { amount: 1 } }),
                            stat_row!(player.stat_points(), client_state().localization().luck_text(), player.luck(), player.bonus_luck(), player.luck_stat_points_cost(), InputEvent::StatUp { stat_type: StatUpType::Luck { amount: 1 } }),
                            text! { text: PairLine::new("ATK", player.attack_base(), player.attack_bonus()), overflow_behavior: OverflowBehavior::Shrink },
                            text! { text: PairLine::new("MATK", player.magic_attack_base(), player.magic_attack_bonus()), overflow_behavior: OverflowBehavior::Shrink },
                            ArmorLine::new("DEF", player.defense_base(), player.defense_bonus(), false),
                            ArmorLine::new("MDEF", player.magic_defense_base(), player.magic_defense_bonus(), true),
                            text! { text: ValueLine::new("HIT", player.hit()), overflow_behavior: OverflowBehavior::Shrink },
                            text! { text: PairLine::new("FLEE", player.flee_base(), player.flee_bonus()), overflow_behavior: OverflowBehavior::Shrink },
                            text! { text: ValueLine::new("CRIT", player.critical()), overflow_behavior: OverflowBehavior::Shrink },
                            text! { text: AspdLine::new(player.attack_speed()), overflow_behavior: OverflowBehavior::Shrink },
                        ),
                    },
                },
                text! {
                    text: client_state().stats_window().notice(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
            ),
        }
    }
}
