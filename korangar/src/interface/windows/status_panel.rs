//! Upper-left status panel modeled on roBrowser BasicInfo, painted from theme
//! tokens.

use std::cell::{Cell, UnsafeCell};

use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
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
use crate::interface::windows::WindowClass;
use crate::interface::windows::basic_info_view::{WeightColor, WeightLabel, ZenyLabel};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::status_effects::{ActiveStatusEffect, StatusEffects};
use crate::state::status_names::{status_effect_description, status_effect_name};
use crate::state::theme::{BasicInfoThemePathExt, InterfaceThemePathExt, InterfaceThemeType, ThemeTokens, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};
use crate::world::{BasicInfoStatsPathExt, CommonPathExt, Player, PlayerPathExt};

const PORTRAIT_SIDE: f32 = 72.0;
const HEADER_COLUMN_GAP: f32 = 6.0;
const NAME_ROW_HEIGHT: f32 = 16.0;
const RESOURCE_BAR_GAP: f32 = 4.0;
/// Inset of the track inside the neutral border.
const RESOURCE_BAR_INSET: f32 = 2.0;
const LEVEL_ROW_HEIGHT: f32 = 14.0;
const EXP_BAR_HEIGHT: f32 = 8.0;
const FOOTER_ROW_HEIGHT: f32 = 12.0;
/// Smaller than the token body size (13) used by the Base/Job rows.
const FOOTER_FONT_SIZE: f32 = 11.0;
const BAR_TEXT_FONT_SIZE: f32 = 11.0;
const ICON_SIDE: f32 = 24.0;
const ICON_GAP: f32 = 3.0;
/// Header, two level labels, two experience bars, the economy row, buffs.
const STATUS_PANEL_ELEMENT_COUNT: f32 = 7.0;
const STATUS_PANEL_ELEMENT_GAP: f32 = 4.0;

/// Outer height of this window on the in-game token theme (title 22, title gap
/// 2, border 6). The portrait row holds the name and both resource bars, then
/// Base/Job, one zeny+weight row, and the buff row. The first-open HUD
/// reserves this so the left gutter starts below the panel.
pub(crate) const STATUS_PANEL_WINDOW_HEIGHT: f32 = {
    const TITLE_HEIGHT: f32 = 22.0;
    const TITLE_GAP: f32 = 2.0;
    const BORDER: f32 = 6.0;
    const BUFF_ROW: f32 = ICON_SIDE + 2.0;
    const ELEMENT_GAPS: f32 = (STATUS_PANEL_ELEMENT_COUNT - 1.0) * STATUS_PANEL_ELEMENT_GAP;
    const BODY: f32 = PORTRAIT_SIDE + LEVEL_ROW_HEIGHT * 2.0 + EXP_BAR_HEIGHT * 2.0 + FOOTER_ROW_HEIGHT + BUFF_ROW;

    TITLE_HEIGHT + TITLE_GAP + BORDER + BODY + ELEMENT_GAPS
};

/// Axis-aligned rectangle in window pixels. Used by the layout tests.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PanelRect {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

impl PanelRect {
    fn right(self) -> f32 {
        self.left + self.width
    }

    fn bottom(self) -> f32 {
        self.top + self.height
    }
}

/// Portrait on the left. Name, then HP, then SP, fill the column to its right.
#[derive(Clone, Copy, Debug, PartialEq)]
struct StatusHeaderFrames {
    portrait: PanelRect,
    name: PanelRect,
    hp_bar: PanelRect,
    sp_bar: PanelRect,
    height: f32,
}

fn status_header_frames(origin_left: f32, origin_top: f32, width: f32) -> StatusHeaderFrames {
    let portrait = PanelRect {
        left: origin_left,
        top: origin_top,
        width: PORTRAIT_SIDE,
        height: PORTRAIT_SIDE,
    };
    let column_left = portrait.right() + HEADER_COLUMN_GAP;
    let column_width = (width - PORTRAIT_SIDE - HEADER_COLUMN_GAP).max(0.0);
    let bar_height = ((PORTRAIT_SIDE - NAME_ROW_HEIGHT - RESOURCE_BAR_GAP * 2.0) / 2.0).max(8.0);
    let name = PanelRect {
        left: column_left,
        top: origin_top,
        width: column_width,
        height: NAME_ROW_HEIGHT,
    };
    let hp_bar = PanelRect {
        left: column_left,
        top: name.bottom() + RESOURCE_BAR_GAP,
        width: column_width,
        height: bar_height,
    };
    let sp_bar = PanelRect {
        left: column_left,
        top: hp_bar.bottom() + RESOURCE_BAR_GAP,
        width: column_width,
        height: bar_height,
    };
    let height = PORTRAIT_SIDE.max(sp_bar.bottom() - origin_top);
    StatusHeaderFrames {
        portrait,
        name,
        hp_bar,
        sp_bar,
        height,
    }
}

/// Weight on the left, zeny on the right, one short row.
#[derive(Clone, Copy, Debug, PartialEq)]
struct EconomyRowFrames {
    weight: PanelRect,
    zeny: PanelRect,
    height: f32,
    font_size: f32,
}

fn economy_row_frames(origin_left: f32, origin_top: f32, width: f32) -> EconomyRowFrames {
    let weight_width = (width * 0.56).clamp(0.0, width);
    EconomyRowFrames {
        weight: PanelRect {
            left: origin_left,
            top: origin_top,
            width: weight_width,
            height: FOOTER_ROW_HEIGHT,
        },
        zeny: PanelRect {
            left: origin_left + weight_width,
            top: origin_top,
            width: (width - weight_width).max(0.0),
            height: FOOTER_ROW_HEIGHT,
        },
        height: FOOTER_ROW_HEIGHT,
        font_size: FOOTER_FONT_SIZE,
    }
}

fn resource_ratio(current: u64, maximum: u64) -> f32 {
    if maximum == 0 {
        0.0
    } else {
        (current as f32 / maximum as f32).clamp(0.0, 1.0)
    }
}

/// Width of the colored fill inside the border inset.
fn resource_fill_width(bar_width: f32, ratio: f32) -> f32 {
    let inner = (bar_width - RESOURCE_BAR_INSET * 2.0).max(0.0);
    (inner * ratio.clamp(0.0, 1.0)).min(inner)
}

/// Fill, track, border, and bevel colors taken from the active theme.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ResourceBarColors {
    fill: Color,
    track: Color,
    border: Color,
    highlight: Color,
    text: Color,
    text_shadow: Color,
}

fn resource_bar_colors(tokens: &ThemeTokens, health: bool) -> ResourceBarColors {
    ResourceBarColors {
        fill: if health { tokens.hp } else { tokens.sp },
        track: tokens.raised,
        border: tokens.border,
        highlight: tokens.accent,
        text: tokens.text,
        text_shadow: tokens.bg,
    }
}

fn panel_area(rect: PanelRect) -> Area {
    Area {
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
    }
}

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

    fn amounts(&self, state: &State<ClientState>) -> (usize, usize)
    where
        A: Path<ClientState, usize>,
        B: Path<ClientState, usize>,
    {
        (*state.get(&self.current), *state.get(&self.maximum))
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

struct ShadowedLabel<'a> {
    area: Area,
    text: &'a str,
    font: FontSize,
    color: Color,
    shadow: Color,
    horizontal: HorizontalAlignment,
    vertical: VerticalAlignment,
}

fn paint_shadowed_text<'a>(layout: &mut WindowLayout<'a, ClientState>, label: ShadowedLabel<'a>) {
    let shadow_area = Area {
        left: label.area.left + 1.0,
        top: label.area.top + 1.0,
        width: label.area.width,
        height: label.area.height,
    };
    layout.add_text(
        shadow_area,
        label.text,
        label.font,
        label.shadow,
        label.shadow,
        label.horizontal,
        label.vertical,
        OverflowBehavior::Shrink,
    );
    layout.add_text(
        label.area,
        label.text,
        label.font,
        label.color,
        label.color,
        label.horizontal,
        label.vertical,
        OverflowBehavior::Shrink,
    );
}

/// Rounded bar: neutral `border`, `raised` track, `hp`/`sp` fill, `accent`
/// gloss.
fn paint_resource_bar<'a>(
    layout: &mut WindowLayout<'a, ClientState>,
    area: Area,
    ratio: f32,
    colors: ResourceBarColors,
    text: &'a str,
    corner: f32,
) {
    let radius = corner.min(area.height * 0.5).max(3.0);
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(radius),
        colors.border,
        Color::TRANSPARENT,
        ShadowPadding::uniform(0.0),
    );

    let inner = Area {
        left: area.left + RESOURCE_BAR_INSET,
        top: area.top + RESOURCE_BAR_INSET,
        width: (area.width - RESOURCE_BAR_INSET * 2.0).max(0.0),
        height: (area.height - RESOURCE_BAR_INSET * 2.0).max(0.0),
    };
    let inner_radius = (radius - RESOURCE_BAR_INSET).max(1.0);
    layout.add_rectangle(
        inner,
        CornerDiameter::uniform(inner_radius),
        colors.track,
        Color::TRANSPARENT,
        ShadowPadding::uniform(0.0),
    );

    if ratio > 0.0 && inner.width > 0.0 {
        let mut filled = inner;
        filled.width = resource_fill_width(area.width, ratio);
        let fill_radius = inner_radius.min(filled.width * 0.5);
        layout.add_rectangle(
            filled,
            CornerDiameter::uniform(fill_radius),
            colors.fill,
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
        let gloss_height = (filled.height * 0.4).clamp(1.0, 6.0).min(filled.height);
        layout.add_rectangle(
            Area {
                left: filled.left,
                top: filled.top,
                width: filled.width,
                height: gloss_height,
            },
            CornerDiameter::uniform(fill_radius),
            colors.highlight.with_alpha_u8(85),
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
    }

    let lip_height = 2.0_f32.min(inner.height);
    if lip_height > 0.0 && inner.width > 0.0 {
        layout.add_rectangle(
            Area {
                left: inner.left,
                top: inner.top + inner.height - lip_height,
                width: inner.width,
                height: lip_height,
            },
            CornerDiameter::uniform(inner_radius.min(lip_height)),
            colors.border.with_alpha_u8(110),
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
    }

    paint_shadowed_text(layout, ShadowedLabel {
        area: inner,
        text,
        font: FontSize(BAR_TEXT_FONT_SIZE),
        color: colors.text,
        shadow: colors.text_shadow.with_alpha_u8(190),
        horizontal: HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
        vertical: VerticalAlignment::Center { offset: 0.0 },
    });
}

struct StatusHeader<N, H, HM, S, SM> {
    name: N,
    hp_label: PairLabel<H, HM>,
    sp_label: PairLabel<S, SM>,
}

impl<N, H, HM, S, SM> StatusHeader<N, H, HM, S, SM> {
    fn new(name: N, hp_label: PairLabel<H, HM>, sp_label: PairLabel<S, SM>) -> Self {
        Self { name, hp_label, sp_label }
    }
}

impl<N, H, HM, S, SM> Element<ClientState> for StatusHeader<N, H, HM, S, SM>
where
    N: Path<ClientState, String>,
    H: Path<ClientState, usize>,
    HM: Path<ClientState, usize>,
    S: Path<ClientState, usize>,
    SM: Path<ClientState, usize>,
{
    type LayoutInfo = korangar_interface::element::BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let height = status_header_frames(0.0, 0.0, 0.0).height;
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
        let frames = status_header_frames(layout_info.area.left, layout_info.area.top, layout_info.area.width);
        let tokens_path = client_theme().tokens();
        let tokens = state.get(&tokens_path);
        let corner = tokens.corner_radius;
        paint_portrait(state, layout, panel_area(frames.portrait));

        let name_font = FontSize(tokens.font_size);
        layout.add_text(
            panel_area(frames.name),
            state.get(&self.name).as_str(),
            name_font,
            tokens.text,
            tokens.text,
            HorizontalAlignment::Left { offset: 1.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );

        let (health, max_health) = self.hp_label.amounts(state);
        let (spell, max_spell) = self.sp_label.amounts(state);
        paint_resource_bar(
            layout,
            panel_area(frames.hp_bar),
            resource_ratio(health as u64, max_health as u64),
            resource_bar_colors(tokens, true),
            state.get(&self.hp_label).as_str(),
            corner,
        );
        paint_resource_bar(
            layout,
            panel_area(frames.sp_bar),
            resource_ratio(spell as u64, max_spell as u64),
            resource_bar_colors(tokens, false),
            state.get(&self.sp_label).as_str(),
            corner,
        );
    }
}

struct EconomyRow<W, WM, Z, C> {
    weight_label: WeightLabel<W, WM>,
    weight_color: WeightColor<W, WM>,
    zeny_label: ZenyLabel<Z>,
    zeny_color: C,
}

impl<W, WM, Z, C> EconomyRow<W, WM, Z, C> {
    fn new(weight_label: WeightLabel<W, WM>, weight_color: WeightColor<W, WM>, zeny_label: ZenyLabel<Z>, zeny_color: C) -> Self {
        Self {
            weight_label,
            weight_color,
            zeny_label,
            zeny_color,
        }
    }
}

impl<W, WM, Z, C> Element<ClientState> for EconomyRow<W, WM, Z, C>
where
    W: Path<ClientState, u32>,
    WM: Path<ClientState, u32>,
    Z: Path<ClientState, u64>,
    C: Path<ClientState, Color>,
{
    type LayoutInfo = korangar_interface::element::BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| korangar_interface::element::BaseLayoutInfo {
            area: resolver.with_height(FOOTER_ROW_HEIGHT),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let frames = economy_row_frames(layout_info.area.left, layout_info.area.top, layout_info.area.width);
        let font = FontSize(frames.font_size);
        let highlight = *state.get(&client_theme().tokens().accent());
        layout.add_text(
            panel_area(frames.weight),
            state.get(&self.weight_label).as_str(),
            font,
            *state.get(&self.weight_color),
            highlight,
            HorizontalAlignment::Left { offset: 1.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        layout.add_text(
            panel_area(frames.zeny),
            state.get(&self.zeny_label).as_str(),
            font,
            *state.get(&self.zeny_color),
            highlight,
            HorizontalAlignment::Right { offset: 1.0, border: 1.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
    }
}

#[allow(dead_code)]
struct BuffIconTip(u16);

/// Row of active status icons with a remaining-time overlay and a
/// name/description tooltip.
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
            let mark = name
                .chars()
                .next()
                .map(|ch| ch.to_uppercase().to_string())
                .unwrap_or_else(|| "?".into());
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
                StatusHeader::new(
                    self.player_name_path,
                    PairLabel::new("HP", player.common().health_points(), player.common().maximum_health_points()),
                    PairLabel::new("SP", player.spell_points(), player.maximum_spell_points()),
                ),
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
                EconomyRow::new(
                    WeightLabel::new(info.weight(), info.maximum_weight()),
                    WeightColor::new(info.weight(), info.maximum_weight()),
                    ZenyLabel::new(info.zeny()),
                    client_theme().basic_info().zeny_color(),
                ),
                StatusBuffRow::new(self.status_effects_path),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::ClientTick;

    use super::{
        FOOTER_FONT_SIZE, FOOTER_ROW_HEIGHT, LEVEL_ROW_HEIGHT, PORTRAIT_SIDE, STATUS_PANEL_WINDOW_HEIGHT, economy_row_frames,
        format_remaining, remaining_ms, resource_bar_colors, resource_fill_width, resource_ratio, status_header_frames,
    };
    use crate::state::status_effects::ActiveStatusEffect;
    use crate::state::theme::ThemeTokens;

    /// Body size of the Base/Job rows on the bundled token themes.
    const LEVEL_FONT_SIZE: f32 = 13.0;

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

    #[test]
    fn header_puts_name_and_bars_beside_the_portrait() {
        let frames = status_header_frames(8.0, 12.0, 240.0);
        assert!((frames.height - PORTRAIT_SIDE).abs() < 0.01);
        assert!((frames.portrait.width - PORTRAIT_SIDE).abs() < 0.01);
        assert!(frames.name.left >= frames.portrait.right() - 0.01);
        assert!(frames.hp_bar.left >= frames.portrait.right() - 0.01);
        assert!(frames.sp_bar.left >= frames.portrait.right() - 0.01);
        assert!(frames.name.bottom() <= frames.hp_bar.top + 0.01);
        assert!(frames.sp_bar.top >= frames.hp_bar.bottom() - 0.01);
        assert!(frames.hp_bar.top >= frames.portrait.top - 0.01);
        assert!(frames.sp_bar.bottom() <= frames.portrait.bottom() + 0.01);
        assert!(frames.hp_bar.right() <= 8.0 + 240.0 + 0.01);
        assert!(frames.sp_bar.width > 40.0);
        assert!(frames.hp_bar.height > 8.0);
    }

    #[test]
    fn narrow_header_keeps_bars_non_negative() {
        let frames = status_header_frames(0.0, 0.0, 40.0);
        assert!(frames.hp_bar.width >= 0.0);
        assert!(frames.sp_bar.width >= 0.0);
        assert!((frames.height - PORTRAIT_SIDE).abs() < 0.01);
    }

    #[test]
    fn resource_fill_tracks_ratio_inside_the_border() {
        assert_eq!(resource_ratio(0, 0), 0.0);
        assert!((resource_ratio(50, 100) - 0.5).abs() < 0.001);
        assert!((resource_ratio(150, 100) - 1.0).abs() < 0.001);
        assert_eq!(resource_fill_width(100.0, 0.0), 0.0);
        assert!((resource_fill_width(100.0, 1.0) - 96.0).abs() < 0.01);
        assert!((resource_fill_width(100.0, 0.5) - 48.0).abs() < 0.01);
        assert!(resource_fill_width(100.0, 2.0) <= resource_fill_width(100.0, 1.0));
    }

    #[test]
    fn hp_and_sp_colors_come_from_each_theme() {
        for tokens in [
            ThemeTokens::verdant_rune(),
            ThemeTokens::ember_forge(),
            ThemeTokens::arcane_violet(),
        ] {
            let health = resource_bar_colors(&tokens, true);
            let spell = resource_bar_colors(&tokens, false);
            assert_eq!(health.fill, tokens.hp);
            assert_eq!(spell.fill, tokens.sp);
            assert_eq!(health.track, tokens.raised);
            assert_eq!(spell.track, tokens.raised);
            assert_eq!(health.border, tokens.border);
            assert_eq!(spell.border, tokens.border);
            assert_eq!(health.highlight, tokens.accent);
            assert_eq!(health.text, tokens.text);
            assert_eq!(health.text_shadow, tokens.bg);
            assert_ne!(health.border, tokens.gold);
            assert_ne!(health.border, tokens.hover);
            assert_ne!(spell.border, tokens.gold);
            assert_ne!(health.fill, spell.fill);
            assert!(FOOTER_FONT_SIZE < tokens.font_size);
        }
    }

    #[test]
    fn zeny_and_weight_share_a_row_smaller_than_level_rows() {
        let row = economy_row_frames(4.0, 80.0, 200.0);
        assert!((row.weight.top - row.zeny.top).abs() < 0.01);
        assert!((row.weight.height - row.zeny.height).abs() < 0.01);
        assert!((row.weight.right() - row.zeny.left).abs() < 0.01);
        assert!(row.zeny.right() <= 4.0 + 200.0 + 0.01);
        assert!(row.font_size < LEVEL_FONT_SIZE);
        assert!(row.height < LEVEL_ROW_HEIGHT);
        assert!((row.height - FOOTER_ROW_HEIGHT).abs() < 0.01);
    }

    #[test]
    fn window_height_matches_the_stacked_rows() {
        let header = status_header_frames(0.0, 0.0, 240.0).height;
        let body = header + LEVEL_ROW_HEIGHT * 2.0 + 8.0 * 2.0 + FOOTER_ROW_HEIGHT + (24.0 + 2.0);
        let chrome = 22.0 + 2.0 + 6.0;
        let gaps = 6.0 * 4.0;
        assert!((STATUS_PANEL_WINDOW_HEIGHT - (chrome + body + gaps)).abs() < 0.01);
    }
}
