use std::sync::Arc;

use korangar_interface::components::text_box::DefaultHandler;
use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::window::{CustomWindow, Window};
use rust_state::{Path, PathExt, State};

use crate::graphics::{Color, CornerDiameter, ShadowPadding, Texture};
use crate::input::InputEvent;
use crate::interface::components::item_box::{paint_item_slot, slot_border_and_fill};
use crate::interface::windows::WindowClass;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::eden::{EDEN_PANE_SHOP, EdenButton, EdenWindowState, EdenWindowStatePathExt, eden_button_packets};
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

const TITLE_H: f32 = 22.0;
const NOTE_H: f32 = 36.0;
const MISSION_H: f32 = 88.0;
const CELL_W: f32 = 132.0;
const CELL_H: f32 = 128.0;
const GAP: f32 = 6.0;
const BUTTON_W: f32 = 78.0;
const BUTTON_H: f32 = 22.0;

struct EdenAmountBox;

enum BoardClick {
    Send(EdenButton),
    Prompt(i32),
}

impl BoardClick {
    fn send(button: EdenButton) -> Self {
        Self::Send(button)
    }
}

impl ClickHandler<ClientState> for BoardClick {
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        match self {
            BoardClick::Send(button) => queue.queue(InputEvent::EdenPackets {
                packets: eden_button_packets(*button),
            }),
            BoardClick::Prompt(slot) => {
                let slot = *slot;
                state.update_value_with(client_state().eden_window(), move |window| window.begin_buy(slot));
            }
        }
    }
}

enum BoardBlock {
    Title(String),
    Note(String),
    Mission {
        band: usize,
        mission: usize,
        accept: Option<BoardClick>,
        complete: Option<BoardClick>,
        abandon: Option<BoardClick>,
    },
    ShopRow {
        cells: Vec<ShopCell>,
    },
}

struct ShopCell {
    index: usize,
    buy: Option<BoardClick>,
}

struct EdenBoard<A> {
    path: A,
    blocks: Vec<BoardBlock>,
}

impl<A> EdenBoard<A> {
    fn new(path: A) -> Self {
        Self { path, blocks: Vec::new() }
    }
}

impl<A> EdenBoard<A>
where
    A: Path<ClientState, EdenWindowState>,
{
    fn rebuild(&mut self, window: &EdenWindowState, width: f32) {
        self.blocks.clear();
        if window.pane() == EDEN_PANE_SHOP {
            self.rebuild_shop(window, width);
        } else {
            self.rebuild_missions(window);
        }
        if self.blocks.is_empty() {
            self.blocks.push(BoardBlock::Note("Nothing to show.".to_owned()));
        }
    }

    fn rebuild_missions(&mut self, window: &EdenWindowState) {
        if window.bands().is_empty() {
            let text = if window.base_level() < 11 {
                format!(
                    "No hunts for base level {}. Eden hunting starts at level 11.",
                    window.base_level()
                )
            } else {
                format!("No Eden missions for base level {}.", window.base_level())
            };
            self.blocks.push(BoardBlock::Note(text));
            return;
        }
        for (band_index, band) in window.bands().iter().enumerate() {
            self.blocks.push(BoardBlock::Title(band.label.clone()));
            for (mission_index, mission) in band.missions.iter().enumerate() {
                let accept = mission.can_accept.then(|| BoardClick::send(EdenButton::Accept(mission.quest_id)));
                let complete = mission
                    .can_complete
                    .then(|| BoardClick::send(EdenButton::Complete(mission.quest_id)));
                let abandon = mission.can_abandon.then(|| BoardClick::send(EdenButton::Abandon(mission.quest_id)));
                self.blocks.push(BoardBlock::Mission {
                    band: band_index,
                    mission: mission_index,
                    accept,
                    complete,
                    abandon,
                });
            }
        }
    }

    fn rebuild_shop(&mut self, window: &EdenWindowState, width: f32) {
        if window.offers().is_empty() {
            self.blocks.push(BoardBlock::Note("Nothing in the Eden shop.".to_owned()));
            return;
        }
        let columns = shop_columns(width);
        let mut current_category = u8::MAX;
        let mut row: Vec<ShopCell> = Vec::new();
        for (index, offer) in window.offers().iter().enumerate() {
            if offer.category != current_category {
                if !row.is_empty() {
                    self.blocks.push(BoardBlock::ShopRow {
                        cells: std::mem::take(&mut row),
                    });
                }
                current_category = offer.category;
                self.blocks.push(BoardBlock::Title(offer.category_label.clone()));
            }
            let buy = (!offer.locked).then_some(BoardClick::Prompt(offer.slot));
            row.push(ShopCell { index, buy });
            if row.len() == columns {
                self.blocks.push(BoardBlock::ShopRow {
                    cells: std::mem::take(&mut row),
                });
            }
        }
        if !row.is_empty() {
            self.blocks.push(BoardBlock::ShopRow { cells: row });
        }
    }

    fn content_height(&self) -> f32 {
        let mut height = 0.0;
        for block in &self.blocks {
            if height > 0.0 {
                height += GAP;
            }
            height += block_height(block);
        }
        height.max(NOTE_H)
    }
}

fn shop_columns(width: f32) -> usize {
    let columns = ((width + GAP) / (CELL_W + GAP)).floor() as usize;
    columns.clamp(1, 5)
}

fn block_height(block: &BoardBlock) -> f32 {
    match block {
        BoardBlock::Title(_) => TITLE_H,
        BoardBlock::Note(_) => NOTE_H,
        BoardBlock::Mission { .. } => MISSION_H,
        BoardBlock::ShopRow { .. } => CELL_H,
    }
}

struct EdenLayout {
    area: Area,
}

impl<A> Element<ClientState> for EdenBoard<A>
where
    A: Path<ClientState, EdenWindowState>,
{
    type LayoutInfo = EdenLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let width = with_single_resolver(resolvers, |resolver| resolver.clone().with_height(1.0).width);
        let path = self.path;
        let window = state.get(&path);
        self.rebuild(window, width);
        let height = self.content_height();
        let area = with_single_resolver(resolvers, |resolver| resolver.with_height(height));
        EdenLayout { area }
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let window = state.get(&self.path);
        let token_path = client_theme().tokens();
        let tokens = state.get(&token_path);
        let font = tokens.font_size;
        let mut top = layout_info.area.top;
        for block in &self.blocks {
            let height = block_height(block);
            let area = Area {
                left: layout_info.area.left,
                top,
                width: layout_info.area.width,
                height,
            };
            match block {
                BoardBlock::Title(text) => paint_line(layout, area, text, font, tokens.text, tokens.text, true),
                BoardBlock::Note(text) => paint_line(layout, area, text, font, tokens.muted, tokens.muted, false),
                BoardBlock::Mission {
                    band,
                    mission,
                    accept,
                    complete,
                    abandon,
                } => {
                    if let Some(mission) = window.bands().get(*band).and_then(|band| band.missions.get(*mission)) {
                        paint_mission(
                            layout,
                            area,
                            mission,
                            &MissionActions { accept, complete, abandon },
                            tokens,
                            font,
                        );
                    }
                }
                BoardBlock::ShopRow { cells } => paint_shop_row(layout, state, area, window, cells, tokens, font),
            }
            top += height + GAP;
        }
    }
}

fn paint_line<'a>(
    layout: &mut WindowLayout<'a, ClientState>,
    area: Area,
    text: &'a str,
    font: f32,
    color: Color,
    highlight: Color,
    bold_bar: bool,
) {
    if bold_bar {
        layout.add_rectangle(
            Area {
                left: area.left,
                top: area.top + area.height - 2.0,
                width: area.width,
                height: 2.0,
            },
            CornerDiameter::uniform(1.0),
            color,
            color,
            ShadowPadding::diagonal(0.0, 0.0),
        );
    }
    layout.add_text(
        area,
        text,
        FontSize(font),
        color,
        highlight,
        HorizontalAlignment::Left { offset: 2.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
}

struct MissionActions<'a> {
    accept: &'a Option<BoardClick>,
    complete: &'a Option<BoardClick>,
    abandon: &'a Option<BoardClick>,
}

fn paint_mission<'a>(
    layout: &mut WindowLayout<'a, ClientState>,
    area: Area,
    mission: &'a crate::state::eden::EdenMission,
    actions: &MissionActions<'a>,
    tokens: &crate::state::theme::ThemeTokens,
    font: f32,
) {
    let border = if mission.state == crate::state::eden::EDEN_STATE_TURNIN {
        tokens.accent
    } else {
        tokens.border
    };
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(tokens.corner_radius),
        border,
        tokens.bg,
        ShadowPadding::diagonal(0.0, 0.0),
    );
    let inner = inset(area, 2.0);
    layout.add_rectangle(
        inner,
        CornerDiameter::uniform((tokens.corner_radius - 1.0).max(0.0)),
        tokens.raised,
        tokens.bg,
        ShadowPadding::diagonal(0.0, 0.0),
    );
    layout.add_text(
        Area {
            left: inner.left + 6.0,
            top: inner.top + 2.0,
            width: inner.width - 12.0,
            height: 18.0,
        },
        mission.name.as_str(),
        FontSize(font),
        tokens.text,
        tokens.text,
        HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
    layout.add_text(
        Area {
            left: inner.left + 6.0,
            top: inner.top + 2.0,
            width: inner.width - 12.0,
            height: 18.0,
        },
        mission.state_label.as_str(),
        FontSize((font - 1.0).max(11.0)),
        if mission.state == crate::state::eden::EDEN_STATE_TURNIN {
            tokens.accent
        } else {
            tokens.muted
        },
        tokens.accent,
        HorizontalAlignment::Right { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
    layout.add_text(
        Area {
            left: inner.left + 6.0,
            top: inner.top + 22.0,
            width: inner.width - 12.0,
            height: 16.0,
        },
        mission.progress.as_str(),
        FontSize((font - 1.0).max(11.0)),
        tokens.muted,
        tokens.muted,
        HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
    layout.add_text(
        Area {
            left: inner.left + 6.0,
            top: inner.top + 38.0,
            width: inner.width - 12.0,
            height: 16.0,
        },
        mission.rewards.as_str(),
        FontSize((font - 1.0).max(11.0)),
        tokens.text,
        tokens.text,
        HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );

    let mut buttons: Vec<(&str, &BoardClick)> = Vec::new();
    if let Some(click) = actions.accept {
        buttons.push(("Accept", click));
    }
    if let Some(click) = actions.complete {
        buttons.push(("Complete", click));
    }
    if let Some(click) = actions.abandon {
        buttons.push(("Abandon", click));
    }
    let mut left = inner.left + 6.0;
    let button_top = inner.top + inner.height - BUTTON_H - 4.0;
    for (label, click) in buttons {
        let button = Area {
            left,
            top: button_top,
            width: BUTTON_W,
            height: BUTTON_H,
        };
        paint_button(layout, button, label, click, tokens, font);
        left += BUTTON_W + 4.0;
    }
}

fn paint_shop_row<'a>(
    layout: &mut WindowLayout<'a, ClientState>,
    state: &State<ClientState>,
    area: Area,
    window: &'a EdenWindowState,
    cells: &'a [ShopCell],
    tokens: &crate::state::theme::ThemeTokens,
    font: f32,
) {
    for (column, cell) in cells.iter().enumerate() {
        let Some(offer) = window.offers().get(cell.index) else {
            continue;
        };
        let cell_area = Area {
            left: area.left + column as f32 * (CELL_W + GAP),
            top: area.top,
            width: CELL_W,
            height: CELL_H,
        };
        let hovered = cell_area.check().dont_mark().run(layout);
        let border = if hovered { tokens.hover } else { tokens.border };
        layout.add_rectangle(
            cell_area,
            CornerDiameter::uniform(tokens.corner_radius),
            border,
            tokens.bg,
            ShadowPadding::diagonal(0.0, 0.0),
        );
        let inner = inset(cell_area, 2.0);
        layout.add_rectangle(
            inner,
            CornerDiameter::uniform((tokens.corner_radius - 1.0).max(0.0)),
            tokens.raised,
            tokens.bg,
            ShadowPadding::diagonal(0.0, 0.0),
        );
        let icon = Area {
            left: inner.left + (inner.width - tokens.slot_size.min(40.0)) * 0.5,
            top: inner.top + 4.0,
            width: tokens.slot_size.min(40.0),
            height: tokens.slot_size.min(40.0),
        };
        let (slot_border, slot_fill) = slot_border_and_fill(state, hovered, false);
        paint_item_slot(layout, icon, slot_border, slot_fill, tokens.corner_radius, true);
        if let Some(texture) = offer.texture.clone() {
            paint_icon(layout, inset(icon, 4.0), texture);
        }
        layout.add_text(
            Area {
                left: inner.left + 4.0,
                top: inner.top + 46.0,
                width: inner.width - 8.0,
                height: 16.0,
            },
            offer.name.as_str(),
            FontSize((font - 1.0).max(11.0)),
            if offer.locked { tokens.muted } else { tokens.text },
            tokens.text,
            HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        layout.add_text(
            Area {
                left: inner.left + 4.0,
                top: inner.top + 62.0,
                width: inner.width - 8.0,
                height: 14.0,
            },
            offer.detail.as_str(),
            FontSize((font - 2.0).max(10.0)),
            tokens.muted,
            tokens.muted,
            HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        layout.add_text(
            Area {
                left: inner.left + 4.0,
                top: inner.top + 76.0,
                width: inner.width - 8.0,
                height: 14.0,
            },
            offer.cost.as_str(),
            FontSize((font - 1.0).max(11.0)),
            tokens.text,
            tokens.text,
            HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
            VerticalAlignment::Center { offset: 0.0 },
            OverflowBehavior::Shrink,
        );
        let button = Area {
            left: inner.left + 8.0,
            top: inner.top + inner.height - BUTTON_H - 4.0,
            width: inner.width - 16.0,
            height: BUTTON_H,
        };
        if let Some(click) = &cell.buy {
            paint_button(layout, button, "Buy", click, tokens, font);
        } else {
            paint_line(
                layout,
                button,
                "Locked",
                (font - 1.0).max(11.0),
                tokens.muted,
                tokens.muted,
                false,
            );
        }
    }
}

fn paint_icon(layout: &mut WindowLayout<ClientState>, area: Area, texture: Arc<Texture>) {
    layout.add_texture(area, texture, Color::WHITE, false);
}

fn paint_button<'a>(
    layout: &mut WindowLayout<'a, ClientState>,
    area: Area,
    label: &'a str,
    click: &'a BoardClick,
    tokens: &crate::state::theme::ThemeTokens,
    font: f32,
) {
    let hovered = area.check().run(layout);
    let border = if hovered { tokens.hover } else { tokens.border };
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(4.0),
        border,
        tokens.bg,
        ShadowPadding::diagonal(0.0, 0.0),
    );
    layout.add_text(
        area,
        label,
        FontSize((font - 1.0).max(11.0)),
        if hovered { tokens.hover } else { tokens.text },
        tokens.hover,
        HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
    if hovered {
        layout.register_click_handler(MouseButton::Left, click);
    }
}

fn inset(area: Area, pad: f32) -> Area {
    Area {
        left: area.left + pad,
        top: area.top + pad,
        width: (area.width - pad * 2.0).max(1.0),
        height: (area.height - pad * 2.0).max(1.0),
    }
}

pub struct EdenWindow;

impl EdenWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for EdenWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Eden)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        let state_path = client_state().eden_window();
        let ignore = |_state: &State<ClientState>, _queue: &mut EventQueue<ClientState>| {};

        window! {
            title: "Eden Group",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_width: 520.0,
            maximum_width: 860.0,
            minimum_height: 360.0,
            maximum_height: 720.0,
            elements: (
                text! {
                    text: state_path.headline(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                split! {
                    gaps: theme().window().gaps(),
                    children: (
                        button! {
                            text: "Missions",
                            event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                state.update_value_with(client_state().eden_window(), |window| window.set_pane(0));
                            },
                        },
                        button! {
                            text: "Merit shop",
                            event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                state.update_value_with(client_state().eden_window(), |window| window.set_pane(1));
                            },
                        },
                        button! {
                            text: "Refresh",
                            event: InputEvent::OpenEden,
                        },
                    ),
                },
                scroll_view! {
                    children: EdenBoard::new(state_path),
                },
                either! {
                    selector: ComputedSelector::new_default(|state: &ClientState| {
                        client_state().eden_window().follow_safe(state).buying()
                    }),
                    on_true: split! {
                        gaps: theme().window().gaps(),
                        children: (
                            text! {
                                text: state_path.buy_prompt(),
                                overflow_behavior: OverflowBehavior::Shrink,
                            },
                            text_box! {
                                ghost_text: state_path.buy_ghost(),
                                state: state_path.buy_amount_text(),
                                input_handler: DefaultHandler::<_, _, 4>::new(state_path.buy_amount_text(), ignore),
                                focus_id: EdenAmountBox,
                                overflow_behavior: OverflowBehavior::Shrink,
                            },
                            button! {
                                text: "Confirm buy",
                                event: move |state: &State<ClientState>, queue: &mut EventQueue<ClientState>| {
                                    let decision = state.get(&client_state().eden_window()).confirm_buy();
                                    match decision {
                                        Ok(packets) => queue.queue(InputEvent::EdenPackets { packets }),
                                        Err(message) => {
                                            let message = message.to_owned();
                                            state.update_value_with(client_state().eden_window(), move |window| {
                                                window.set_status(message.clone());
                                            });
                                        }
                                    }
                                },
                            },
                            button! {
                                text: "Cancel",
                                event: move |state: &State<ClientState>, _: &mut EventQueue<ClientState>| {
                                    state.update_value_with(client_state().eden_window(), |window| window.clear_buy());
                                },
                            },
                        ),
                    },
                    on_false: text! {
                        text: "",
                        height: 0.0,
                    },
                },
            ),
        }
    }
}
