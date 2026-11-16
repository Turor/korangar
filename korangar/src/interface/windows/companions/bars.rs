//! Hunger, intimacy, HP, SP, and EXP bars painted from theme tokens.

use std::sync::Arc;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element, ErasedElement};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use rust_state::{Path, State};

use super::state::{CompanionWindowState, CompanionWindowStatePathExt};
use crate::graphics::{Color, CornerDiameter, ShadowPadding, Texture};
use crate::interface::font_scale::from_theme_body;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::theme::{InterfaceThemePathExt, ThemeTokens};
use crate::state::{ClientState, client_theme};

const BAR_HEIGHT: f32 = 18.0;
const BAR_INSET: f32 = 2.0;

#[derive(Clone, Copy)]
pub(crate) enum MeterKind {
    PetHunger,
    PetIntimacy,
    HomHp,
    HomSp,
    HomExp,
    HomHunger,
    HomIntimacy,
    MercHp,
    MercSp,
    ElHp,
    ElSp,
}

pub(crate) struct Meter<P, L> {
    path: P,
    line: L,
    kind: MeterKind,
}

impl<P, L> Meter<P, L> {
    pub(crate) fn new(path: P, line: L, kind: MeterKind) -> Self {
        Self { path, line, kind }
    }
}

impl<P, L> Element<ClientState> for Meter<P, L>
where
    P: Path<ClientState, CompanionWindowState> + 'static,
    L: Path<ClientState, String> + 'static,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| BaseLayoutInfo {
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
        let theme = client_theme().tokens();
        let tokens = state.get(&theme);
        let (ratio, fill) = meter_sample(state, &self.path, self.kind);
        let label = state.get(&self.line).as_str();
        paint_bar(layout, layout_info.area, ratio, fill, tokens, label);
    }
}

pub(crate) struct AccessoryIcon<L, T> {
    label: L,
    texture: T,
}

impl<L, T> AccessoryIcon<L, T> {
    pub(crate) fn new(label: L, texture: T) -> Self {
        Self { label, texture }
    }
}

impl<L, T> Element<ClientState> for AccessoryIcon<L, T>
where
    L: Path<ClientState, String> + 'static,
    T: Path<ClientState, Option<Arc<Texture>>> + 'static,
{
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        _: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        with_single_resolver(resolvers, |resolver| BaseLayoutInfo {
            area: resolver.with_height(28.0),
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let theme = client_theme().tokens();
        let tokens = state.get(&theme);
        let label = state.get(&self.label).as_str();
        let texture = state.get(&self.texture).clone();
        paint_icon_line(
            layout,
            layout_info.area,
            texture.as_ref(),
            label,
            tokens.text,
            from_theme_body(tokens.font_size, 14.0),
        );
    }
}

fn meter_sample<P>(state: &State<ClientState>, path: &P, kind: MeterKind) -> (f32, Fill)
where
    P: Path<ClientState, CompanionWindowState>,
{
    match kind {
        MeterKind::PetHunger => {
            let hunger = *state.get(&path.pet_hunger());
            (
                ratio_i32(i32::from(hunger), 100),
                if hunger < 25 { Fill::Danger } else { Fill::Health },
            )
        }
        MeterKind::PetIntimacy => (ratio_i32(i32::from(*state.get(&path.pet_intimacy())), 1000), Fill::Spell),
        MeterKind::HomHp => (
            ratio_u32(*state.get(&path.homunculus_hp()), *state.get(&path.homunculus_max_hp())),
            Fill::Health,
        ),
        MeterKind::HomSp => (
            ratio_u32(*state.get(&path.homunculus_sp()), *state.get(&path.homunculus_max_sp())),
            Fill::Spell,
        ),
        MeterKind::HomExp => (
            ratio_i64(*state.get(&path.homunculus_exp()), *state.get(&path.homunculus_next_exp())),
            Fill::Experience,
        ),
        MeterKind::HomHunger => {
            let hunger = *state.get(&path.homunculus_hunger());
            (
                ratio_i32(i32::from(hunger), 100),
                if hunger < 25 { Fill::Danger } else { Fill::Health },
            )
        }
        MeterKind::HomIntimacy => (ratio_i32(i32::from(*state.get(&path.homunculus_intimacy())), 1000), Fill::Spell),
        MeterKind::MercHp => (
            ratio_i32(*state.get(&path.mercenary_hp()), *state.get(&path.mercenary_max_hp())),
            Fill::Health,
        ),
        MeterKind::MercSp => (
            ratio_i32(*state.get(&path.mercenary_sp()), *state.get(&path.mercenary_max_sp())),
            Fill::Spell,
        ),
        MeterKind::ElHp => (
            ratio_u32(*state.get(&path.elemental_hp()), *state.get(&path.elemental_max_hp())),
            Fill::Health,
        ),
        MeterKind::ElSp => (
            ratio_u32(*state.get(&path.elemental_sp()), *state.get(&path.elemental_max_sp())),
            Fill::Spell,
        ),
    }
}

fn ratio_i32(current: i32, maximum: i32) -> f32 {
    if maximum <= 0 {
        0.0
    } else {
        (current.max(0) as f32 / maximum as f32).clamp(0.0, 1.0)
    }
}

fn ratio_u32(current: u32, maximum: u32) -> f32 {
    if maximum == 0 {
        0.0
    } else {
        (current as f32 / maximum as f32).clamp(0.0, 1.0)
    }
}

fn ratio_i64(current: i64, maximum: i64) -> f32 {
    if maximum <= 0 {
        0.0
    } else {
        (current.max(0) as f32 / maximum as f32).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy)]
enum Fill {
    Health,
    Spell,
    Experience,
    Danger,
}

fn fill_color(tokens: &ThemeTokens, fill: Fill) -> Color {
    match fill {
        Fill::Health => tokens.status_hp_bar,
        Fill::Spell => tokens.status_sp_bar,
        Fill::Experience => tokens.status_base_exp_bar,
        Fill::Danger => tokens.danger,
    }
}

fn paint_bar<'a>(layout: &mut WindowLayout<'a, ClientState>, area: Area, ratio: f32, fill: Fill, tokens: &ThemeTokens, text: &'a str) {
    let radius = tokens.corner_radius.min(area.height * 0.5).max(3.0);
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(radius),
        tokens.border,
        Color::TRANSPARENT,
        ShadowPadding::uniform(0.0),
    );
    let inner = Area {
        left: area.left + BAR_INSET,
        top: area.top + BAR_INSET,
        width: (area.width - BAR_INSET * 2.0).max(0.0),
        height: (area.height - BAR_INSET * 2.0).max(0.0),
    };
    let inner_radius = (radius - BAR_INSET).max(1.0);
    layout.add_rectangle(
        inner,
        CornerDiameter::uniform(inner_radius),
        tokens.raised,
        Color::TRANSPARENT,
        ShadowPadding::uniform(0.0),
    );
    if ratio > 0.0 && inner.width > 0.0 {
        let filled = Area {
            width: inner.width * ratio,
            ..inner
        };
        layout.add_rectangle(
            filled,
            CornerDiameter::uniform(inner_radius.min(filled.width * 0.5)),
            fill_color(tokens, fill),
            Color::TRANSPARENT,
            ShadowPadding::uniform(0.0),
        );
    }
    layout.add_text(
        inner,
        text,
        FontSize(from_theme_body(tokens.font_size, 12.0)),
        tokens.text,
        tokens.text,
        HorizontalAlignment::Left { offset: 4.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
}

pub(crate) fn paint_icon_line<'a>(
    layout: &mut WindowLayout<'a, ClientState>,
    area: Area,
    texture: Option<&Arc<Texture>>,
    label: &'a str,
    color: Color,
    font_size: f32,
) {
    let mut text_left = area.left + 4.0;
    if let Some(texture) = texture {
        let side = 24.0_f32.min(area.height - 2.0);
        let icon = Area {
            left: area.left,
            top: area.top + (area.height - side) / 2.0,
            width: side,
            height: side,
        };
        layout.add_texture(icon, texture.clone(), Color::monochrome_u8(255), false);
        text_left = icon.left + side + 6.0;
    }
    layout.add_text(
        Area {
            left: text_left,
            top: area.top,
            width: (area.left + area.width - text_left).max(1.0),
            height: area.height,
        },
        label,
        FontSize(font_size),
        color,
        color,
        HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
}

pub(crate) fn boxed_meter<P, L>(path: P, line: L, kind: MeterKind) -> Box<ErasedElement<ClientState, Meter<P, L>>>
where
    P: Path<ClientState, CompanionWindowState> + 'static,
    L: Path<ClientState, String> + 'static,
{
    ErasedElement::new(Meter::new(path, line, kind))
}

pub(crate) fn boxed_accessory<L, T>(label: L, texture: T) -> Box<ErasedElement<ClientState, AccessoryIcon<L, T>>>
where
    L: Path<ClientState, String> + 'static,
    T: Path<ClientState, Option<Arc<Texture>>> + 'static,
{
    ErasedElement::new(AccessoryIcon::new(label, texture))
}
