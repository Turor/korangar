//! Upper-left status panel modeled on roBrowser BasicInfo, painted from theme tokens.

use std::cell::{Cell, UnsafeCell};

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::Element;
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use ragnarok_packets::ClientTick;
use rust_state::{Path, PathExt, Selector, State};

use super::equipment::paint_portrait;
use crate::graphics::{Color, CornerDiameter, ShadowPadding};
use crate::interface::components::exp_bar::ExpBar;
use crate::interface::windows::basic_info_view::{WeightColor, WeightLabel, ZenyLabel};
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::status_effects::{ActiveStatusEffect, StatusEffects};
use crate::state::status_names::{status_effect_description, status_effect_name};
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt, InterfaceThemeType, ThemeTokensPathExt};
use crate::state::{client_state, client_theme, ClientState, ClientStatePathExt};
use crate::world::{BasicInfoStatsPathExt, CommonPathExt, Player, PlayerPathExt};

const PORTRAIT_SIDE: f32 = 72.0;
const BAR_HEIGHT: f32 = 8.0;
const ICON_SIDE: f32 = 24.0;
const ICON_GAP: f32 = 3.0;

pub struct PairLabel<A, B> {
    current: A,
    maximum: B,
    prefix: &'static str,
    last: Cell<Option<(u64, u64)>>,
    text: UnsafeCell<String>,
}

impl<A, B> PairLabel<A, B> {
    pub fn new(prefix: &'static str, current: A, maximum: B) -> Self {
        Self {
            current,
            maximum,
            prefix,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A, B> Selector<ClientState, String> for PairLabel<A, B>
where
    A: Path<ClientState, usize>,
    B: Path<ClientState, usize>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let current = *self.current.follow_safe(state) as u64;
        let maximum = *self.maximum.follow_safe(state) as u64;
        if self.last.get() != Some((current, maximum)) {
            unsafe {
                *self.text.get() = format!("{} {current} / {maximum}", self.prefix);
            }
            self.last.set(Some((current, maximum)));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

pub struct LevelLabel<A> {
    prefix: &'static str,
    path: A,
    last: Cell<Option<usize>>,
    text: UnsafeCell<String>,
}

impl<A> LevelLabel<A> {
    pub fn new(prefix: &'static str, path: A) -> Self {
        Self {
            prefix,
            path,
            last: Cell::default(),
            text: UnsafeCell::default(),
        }
    }
}

impl<A> Selector<ClientState, String> for LevelLabel<A>
where
    A: Path<ClientState, usize>,
{
    fn select<'a>(&'a self, state: &'a ClientState) -> Option<&'a String> {
        let value = *self.path.follow_safe(state);
        if self.last.get() != Some(value) {
            unsafe {
                *self.text.get() = format!("{} {value}", self.prefix);
            }
            self.last.set(Some(value));
        }
        unsafe { Some(self.text.as_ref_unchecked()) }
    }
}

struct ResourceBar<C, M> {
    current: C,
    maximum: M,
    is_health: bool,
}

impl<C, M> ResourceBar<C, M> {
    fn health(current: C, maximum: M) -> Self {
        Self {
            current,
            maximum,
            is_health: true,
        }
    }

    fn spell(current: C, maximum: M) -> Self {
        Self {
            current,
            maximum,
            is_health: false,
        }
    }
}

impl<C, M> Element<ClientState> for ResourceBar<C, M>
where
    C: Path<ClientState, usize>,
    M: Path<ClientState, usize>,
{
    type LayoutInfo = korangar_interface::element::BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| korangar_interface::element::BaseLayoutInfo {
            area: resolver.with_height(BAR_HEIGHT),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let current = *state.get(&self.current) as f32;
        let maximum = (*state.get(&self.maximum) as f32).max(1.0);
        let ratio = (current / maximum).clamp(0.0, 1.0);
        let tokens_path = client_theme().tokens();
        let tokens = state.get(&tokens_path);
        let track = tokens.raised.with_alpha_u8(230);
        let fill = if self.is_health { tokens.hp } else { tokens.sp };
        layout.add_rectangle(
            layout_info.area,
            CornerDiameter::uniform(3.0),
            track,
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
        if ratio > 0.0 {
            let mut filled = layout_info.area;
            filled.width = (layout_info.area.width * ratio).clamp(0.0, layout_info.area.width);
            layout.add_rectangle(
                filled,
                CornerDiameter::uniform(3.0),
                fill,
                Color::TRANSPARENT,
                ShadowPadding::uniform(0.0),
            );
        }
    }
}

struct PortraitFrame;

impl Element<ClientState> for PortraitFrame {
    type LayoutInfo = korangar_interface::element::BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| korangar_interface::element::BaseLayoutInfo {
            area: resolver.with_height(PORTRAIT_SIDE),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let mut area = layout_info.area;
        area.width = PORTRAIT_SIDE.min(area.width);
        area.height = PORTRAIT_SIDE;
        paint_portrait(state, layout, area);
    }
}

#[allow(dead_code)]
struct BuffIconTip(u16);

/// Row of active status icons with a remaining-time overlay and a name/description tooltip.
pub struct StatusBuffRow<A> {
    status_effects_path: A,
    marks: Vec<String>,
    times: Vec<String>,
    tips: Vec<String>,
}

impl<A> StatusBuffRow<A> {
    pub fn new(status_effects_path: A) -> Self {
        Self {
            status_effects_path,
            marks: Vec::new(),
            times: Vec::new(),
            tips: Vec::new(),
        }
    }
}

fn remaining_ms(effect: &ActiveStatusEffect, now: ClientTick) -> u32 {
    if effect.remaining_milliseconds == 0 {
        return 0;
    }
    let elapsed = now.0.saturating_sub(effect.updated_at_tick);
    effect.remaining_milliseconds.saturating_sub(elapsed)
}

fn format_remaining(ms: u32) -> String {
    if ms == 0 {
        return String::new();
    }
    let seconds = ms.div_ceil(1000).max(1);
    if seconds >= 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

impl<A> Element<ClientState> for StatusBuffRow<A>
where
    A: Path<ClientState, StatusEffects>,
{
    type LayoutInfo = korangar_interface::element::BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let now = *state.get(&client_state().client_tick());
        let effects = state.get(&self.status_effects_path).effects();
        self.marks.clear();
        self.times.clear();
        self.tips.clear();
        for effect in effects {
            let name = status_effect_name(effect.status_index);
            let mark = name.chars().next().map(|ch| ch.to_uppercase().to_string()).unwrap_or_else(|| "?".into());
            self.marks.push(mark);
            self.times.push(format_remaining(remaining_ms(effect, now)));
            let description = status_effect_description(effect.status_index);
            if description.is_empty() || description == name {
                self.tips.push(name);
            } else {
                self.tips.push(format!("{name}\n{description}"));
            }
        }
        let height = if effects.is_empty() { 2.0 } else { ICON_SIDE + 2.0 };
        with_single_resolver(resolvers, |resolver| korangar_interface::element::BaseLayoutInfo {
            area: resolver.with_height(height),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let effects = state.get(&self.status_effects_path).effects();
        if effects.is_empty() {
            return;
        }
        let tokens_path = client_theme().tokens();
        let tokens = state.get(&tokens_path);
        let mut left = layout_info.area.left;
        for (index, effect) in effects.iter().enumerate() {
            if left + ICON_SIDE > layout_info.area.left + layout_info.area.width {
                break;
            }
            let area = Area {
                left,
                top: layout_info.area.top,
                width: ICON_SIDE,
                height: ICON_SIDE,
            };
            layout.add_rectangle(
                area,
                CornerDiameter::uniform(4.0),
                tokens.raised.with_alpha_u8(230),
                tokens.border,
                ShadowPadding::uniform(0.0),
            );
            if let Some(mark) = self.marks.get(index) {
                layout.add_text(
                    area,
                    mark,
                    FontSize(11.0),
                    tokens.text,
                    tokens.hover,
                    HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                    VerticalAlignment::Center { offset: -3.0 },
                    OverflowBehavior::Shrink,
                );
            }
            if let Some(time) = self.times.get(index).filter(|time| !time.is_empty()) {
                let time_area = Area {
                    left: area.left,
                    top: area.top + area.height - 10.0,
                    width: area.width,
                    height: 10.0,
                };
                layout.add_text(
                    time_area,
                    time,
                    FontSize(8.0),
                    tokens.gold,
                    tokens.hover,
                    HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                    VerticalAlignment::Center { offset: 0.0 },
                    OverflowBehavior::Shrink,
                );
            }
            if area.check().run(layout)
                && let Some(tip) = self.tips.get(index)
            {
                layout.add_tooltip(tip, BuffIconTip(effect.status_index).tooltip_id());
            }
            left += ICON_SIDE + ICON_GAP;
        }
    }
}

pub struct StatusPanelWindow<A, P, S> {
    player_name_path: A,
    player_path: P,
    status_effects_path: S,
}

impl<A, P, S> StatusPanelWindow<A, P, S> {
    pub fn new(player_name_path: A, player_path: P, status_effects_path: S) -> Self {
        Self {
            player_name_path,
            player_path,
            status_effects_path,
        }
    }
}

impl<A, P, S> CustomWindow<ClientState> for StatusPanelWindow<A, P, S>
where
    A: Path<ClientState, String>,
    P: Path<ClientState, Player>,
    S: Path<ClientState, StatusEffects>,
{
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::StatusPanel)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let player = self.player_path;
        let info = player.basic_info();
        let tokens = client_theme().tokens();

        window! {
            title: "Status",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: false,
            border: 6.0,
            gaps: 4.0,
            minimum_width: 220.0,
            maximum_width: 280.0,
            elements: (
                split! {
                    gaps: 6.0,
                    children: (
                        PortraitFrame,
                        text! {
                            text: self.player_name_path,
                            color: tokens.text(),
                            overflow_behavior: OverflowBehavior::Shrink,
                        },
                    ),
                },
                text! {
                    text: PairLabel::new("HP", player.common().health_points(), player.common().maximum_health_points()),
                    color: tokens.hp(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                ResourceBar::health(player.common().health_points(), player.common().maximum_health_points()),
                text! {
                    text: PairLabel::new("SP", player.spell_points(), player.maximum_spell_points()),
                    color: tokens.sp(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                ResourceBar::spell(player.spell_points(), player.maximum_spell_points()),
                text! {
                    text: LevelLabel::new("Base Lv", player.base_level()),
                    color: tokens.text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                ExpBar::base(info.base_experience(), info.next_base_experience()),
                text! {
                    text: LevelLabel::new("Job Lv", player.job_level()),
                    color: tokens.text(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                ExpBar::job(info.job_experience(), info.next_job_experience()),
                text! {
                    text: WeightLabel::new(info.weight(), info.maximum_weight()),
                    color: WeightColor::new(info.weight(), info.maximum_weight()),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: ZenyLabel::new(info.zeny()),
                    color: client_theme().basic_info().zeny_color(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                StatusBuffRow::new(self.status_effects_path),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::ClientTick;

    use super::{format_remaining, remaining_ms};
    use crate::state::status_effects::ActiveStatusEffect;

    #[test]
    fn remaining_time_counts_down_from_last_update() {
        let effect = ActiveStatusEffect {
            status_index: 10,
            remaining_milliseconds: 5000,
            total_duration_milliseconds: 5000,
            updated_at_tick: 1000,
            label: "Blessing 5s".into(),
        };
        assert_eq!(remaining_ms(&effect, ClientTick(1000)), 5000);
        assert_eq!(remaining_ms(&effect, ClientTick(3000)), 3000);
        assert_eq!(remaining_ms(&effect, ClientTick(8000)), 0);
        assert_eq!(format_remaining(1500), "2s");
        assert_eq!(format_remaining(60000), "1m");
        assert_eq!(format_remaining(0), "");
    }
}
