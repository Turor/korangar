//! Hotbar slots. Skills and items share one grid. F-key rows are visual;
//! [`stored_slot`](crate::state::hotbar::stored_slot) maps them through the row
//! shift.
//!
//! The grid fills the region the (resizable) hotbar window gives it and
//! reflows its rows and columns with [`hotbar_metrics`] (Matthew t274u).
//! Each slot shows its hotkey hint top-left, level or stack bottom-right, the
//! cooldown overlay, and its name under it.

use korangar_interface::MouseMode;
use korangar_interface::element::Element;
use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::event::{ClickHandler, DropHandler, Event, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use ragnarok_packets::{HotbarSlot, HotbarTab};
use rust_state::{Path, State};

use crate::graphics::Color;
use crate::input::{InputEvent, MouseInputMode};
use crate::interface::components::cooldown_overlay::add_skill_cooldown;
use crate::interface::components::hotbar_layout::{
    HOTBAR_HINT_FONT_PX, HOTBAR_LABEL_FONT_PX, HOTBAR_SLOT_GAP, HotbarMetrics, drawn_visual_slots, ellipsize, hotbar_metrics,
};
use crate::interface::font_scale::{self, from_theme_body};
use crate::interface::resource::{ItemSource, SkillSource};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::hotbar::{
    CELL_ITEM, CELL_SKILL, HOTBAR_COLUMNS, HOTBAR_ROWS, HOTBAR_SLOTS, Hotbar, HotbarCell, cell_item_id, cell_skill_level,
    hotbar_stack_label,
};
use crate::state::inventory::Inventory;
use crate::state::theme::{GlobalThemePathExt, HotbarThemePathExt, InterfaceThemePathExt, SkillTreeThemePathExt, ThemeTokensPathExt};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme};

struct SlotLocate<H> {
    visual: u16,
    hotbar_path: H,
}

impl<H> SlotLocate<H>
where
    H: Path<ClientState, Hotbar>,
{
    fn tab_slot(&self, state: &State<ClientState>) -> (HotbarTab, HotbarSlot) {
        state.get(&self.hotbar_path).visual_to_stored(self.visual)
    }
}

struct ActivateHandler<H> {
    slot: SlotLocate<H>,
}

impl<H> ClickHandler<ClientState> for ActivateHandler<H>
where
    H: Path<ClientState, Hotbar>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let (tab, slot) = self.slot.tab_slot(state);
        queue.queue(InputEvent::ActivateHotbar { tab, slot });
    }
}

struct ClearHandler<H> {
    slot: SlotLocate<H>,
}

impl<H> ClickHandler<ClientState> for ClearHandler<H>
where
    H: Path<ClientState, Hotbar>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let (tab, slot) = self.slot.tab_slot(state);
        queue.queue(InputEvent::ClearHotbar { tab, slot });
    }
}

struct DragHandler<H, I> {
    slot: SlotLocate<H>,
    inventory_path: I,
}

impl<H, I> ClickHandler<ClientState> for DragHandler<H, I>
where
    H: Path<ClientState, Hotbar>,
    I: Path<ClientState, Inventory>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let (tab, slot) = self.slot.tab_slot(state);
        let Some(cell) = state.get(&self.slot.hotbar_path).cell(tab, slot) else {
            return;
        };
        if cell.kind == CELL_SKILL
            && let Some(skill) = cell.skill.clone()
        {
            queue.queue(Event::SetMouseMode {
                mouse_mode: MouseMode::Custom {
                    mode: MouseInputMode::MoveSkill {
                        skill,
                        source: SkillSource::Hotbar { tab, slot },
                    },
                },
            });
        } else if let Some(item_id) = cell_item_id(cell)
            && let Some(item) = state.get(&self.inventory_path).find_by_item_id(item_id).cloned()
        {
            if crate::state::item_link::shift_links() {
                queue.queue(InputEvent::LinkItemInChat { item });
                return;
            }
            queue.queue(Event::SetMouseMode {
                mouse_mode: MouseMode::Custom {
                    mode: MouseInputMode::MoveItem {
                        item,
                        source: ItemSource::Hotbar { tab, slot },
                    },
                },
            });
        }
    }
}

struct SlotDrop<H> {
    slot: SlotLocate<H>,
}

impl<H> DropHandler<ClientState> for SlotDrop<H>
where
    H: Path<ClientState, Hotbar>,
{
    fn handle_drop(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>, mouse_mode: &MouseMode<ClientState>) {
        let (tab, slot) = self.slot.tab_slot(state);
        match mouse_mode {
            MouseMode::Custom {
                mode: MouseInputMode::MoveSkill { source, skill },
            } => {
                queue.queue(InputEvent::MoveSkill {
                    source: *source,
                    destination: SkillSource::Hotbar { tab, slot },
                    skill: skill.clone(),
                });
            }
            MouseMode::Custom {
                mode: MouseInputMode::MoveItem { source, item },
            } => {
                queue.queue(InputEvent::MoveItem {
                    source: *source,
                    destination: ItemSource::Hotbar { tab, slot },
                    item: item.clone(),
                });
            }
            _ => {}
        }
    }
}

/// One slot's name label, cached so the text is only re-measured when the
/// name, the slot width or the font change.
#[derive(Default)]
struct NameLabel {
    source: String,
    width: f32,
    font: f32,
    shown: String,
}

pub struct HotbarGrid<H, I> {
    hotbar_path: H,
    inventory_path: I,
    activates: [ActivateHandler<H>; HOTBAR_SLOTS],
    clears: [ClearHandler<H>; HOTBAR_SLOTS],
    drags: [DragHandler<H, I>; HOTBAR_SLOTS],
    drops: [SlotDrop<H>; HOTBAR_SLOTS],
    /// Skill level or stack amount, bottom-right inside the slot.
    labels: [String; HOTBAR_SLOTS],
    /// Skill or item name under the slot, ellipsized to the slot width.
    names: [NameLabel; HOTBAR_SLOTS],
    /// Hotkey hint, top-left inside the slot.
    hints: [String; HOTBAR_SLOTS],
}

impl<H, I> HotbarGrid<H, I>
where
    H: Copy,
    I: Copy,
{
    pub fn new(hotbar_path: H, inventory_path: I) -> Self {
        Self {
            hotbar_path,
            inventory_path,
            activates: std::array::from_fn(|visual| ActivateHandler {
                slot: SlotLocate {
                    visual: visual as u16,
                    hotbar_path,
                },
            }),
            clears: std::array::from_fn(|visual| ClearHandler {
                slot: SlotLocate {
                    visual: visual as u16,
                    hotbar_path,
                },
            }),
            drags: std::array::from_fn(|visual| DragHandler {
                slot: SlotLocate {
                    visual: visual as u16,
                    hotbar_path,
                },
                inventory_path,
            }),
            drops: std::array::from_fn(|visual| SlotDrop {
                slot: SlotLocate {
                    visual: visual as u16,
                    hotbar_path,
                },
            }),
            labels: std::array::from_fn(|_| String::new()),
            names: std::array::from_fn(|_| NameLabel::default()),
            hints: std::array::from_fn(|_| String::new()),
        }
    }
}

pub struct HotbarGridLayout {
    area: Area,
    metrics: HotbarMetrics,
    /// Visual slots in drawing order.
    drawn: Vec<u16>,
    /// Left edge of the (horizontally centred) grid.
    grid_left: f32,
}

/// Name shown under a slot, before truncation.
fn cell_name(cell: &HotbarCell) -> &str {
    match cell.kind {
        CELL_SKILL => cell.skill.as_ref().map(|skill| skill.skill_name.as_str()).unwrap_or(""),
        CELL_ITEM => cell.item_name.as_str(),
        _ => "",
    }
}

impl<H, I> Element<ClientState> for HotbarGrid<H, I>
where
    H: Path<ClientState, Hotbar>,
    I: Path<ClientState, Inventory>,
{
    type LayoutInfo = HotbarGridLayout;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        use korangar_interface::prelude::HorizontalAlignment;

        with_single_resolver(resolvers, |resolver| {
            let hotbar = state.get(&self.hotbar_path);
            let inventory = state.get(&self.inventory_path);
            let keybinds_path = client_state().keybinds();
            let keybinds = state.get(&keybinds_path);
            let drawn = drawn_visual_slots(hotbar.visible_rows(), HOTBAR_COLUMNS, HOTBAR_ROWS, &[36, 37]);

            let body_font = *state.get(&client_theme().tokens().font_size());
            let name_font = from_theme_body(body_font, HOTBAR_LABEL_FONT_PX);
            // Measurement and drawing apply the global UI font scale, so the
            // label line grows with it.
            let label_height = (name_font * font_scale::active_factor()).ceil() + 2.0;

            let available = resolver.push_available_area();
            let metrics = hotbar_metrics(drawn.len(), available.width, available.height, label_height);

            let text_color = *state.get(&client_theme().hotbar().label_color());
            for visual in 0..HOTBAR_SLOTS {
                let (tab, slot) = hotbar.visual_to_stored(visual as u16);
                self.hints[visual] = keybinds.map().slot_hint(visual as u16).unwrap_or_default();
                let Some(cell) = hotbar.cell(tab, slot) else {
                    self.labels[visual].clear();
                    self.names[visual] = NameLabel::default();
                    continue;
                };
                match cell.kind {
                    CELL_SKILL => {
                        self.labels[visual] = cell_skill_level(cell).0.to_string();
                    }
                    CELL_ITEM => {
                        let known = cell_item_id(cell).and_then(|item_id| inventory.known_amount(item_id));
                        let amount = hotbar_stack_label(cell.amount, known);
                        self.labels[visual] = amount.to_string();
                    }
                    _ => self.labels[visual].clear(),
                }

                let name = cell_name(cell);
                let label = &mut self.names[visual];
                if label.source != name || label.width != metrics.slot || label.font != name_font {
                    label.source.clear();
                    label.source.push_str(name);
                    label.width = metrics.slot;
                    label.font = name_font;
                    label.shown = ellipsize(name, metrics.slot, |text| {
                        resolver
                            .get_text_dimensions(
                                text,
                                text_color,
                                text_color,
                                FontSize(name_font),
                                HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                                OverflowBehavior::Shrink,
                            )
                            .0
                            .width
                    });
                }
            }

            let grid_height = metrics.grid_height();
            let height = available.height.map_or(grid_height, |height| height.max(grid_height));
            let area = resolver.with_height(height);
            let grid_left = area.left + ((area.width - metrics.grid_width()) / 2.0).max(0.0);
            HotbarGridLayout {
                area,
                metrics,
                drawn,
                grid_left,
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
        use korangar_interface::prelude::*;

        let hotbar = state.get(&self.hotbar_path);
        let metrics = layout_info.metrics;
        let body_font = *state.get(&client_theme().tokens().font_size());
        let name_font = FontSize(from_theme_body(body_font, HOTBAR_LABEL_FONT_PX));
        let hint_font = FontSize(from_theme_body(body_font, HOTBAR_HINT_FONT_PX));
        let amount_font = FontSize(from_theme_body(body_font, 12.0));
        let label_color = *state.get(&client_theme().hotbar().label_color());
        let hint_color = *state.get(&client_theme().hotbar().hint_color());

        for (index, &visual) in layout_info.drawn.iter().enumerate() {
            let (column, row) = metrics.cell_position(index);
            let area = Area {
                left: layout_info.grid_left + (metrics.slot + HOTBAR_SLOT_GAP) * column as f32,
                top: layout_info.area.top + (metrics.cell_height() + HOTBAR_SLOT_GAP) * row as f32,
                width: metrics.slot,
                height: metrics.slot,
            };
            let label_area = Area {
                left: area.left - HOTBAR_SLOT_GAP / 2.0,
                top: area.top + metrics.slot,
                width: metrics.slot + HOTBAR_SLOT_GAP,
                height: metrics.label_height,
            };
            let (tab, slot) = hotbar.visual_to_stored(visual);
            let cell = hotbar.cell(tab, slot);
            let occupied = cell.is_some_and(|cell| cell.kind == CELL_SKILL || cell.kind == CELL_ITEM);
            let is_drop_target = match layout.get_mouse_mode() {
                MouseMode::Custom {
                    mode: MouseInputMode::MoveSkill { source, .. },
                } => *source != SkillSource::Hotbar { tab, slot },
                MouseMode::Custom {
                    mode: MouseInputMode::MoveItem { source, .. },
                } => *source != ItemSource::Hotbar { tab, slot },
                _ => false,
            };
            let is_hovered = match is_drop_target {
                true => area.check().any_mouse_mode().run(layout),
                false => area.check().run(layout),
            };
            let background = if is_drop_target {
                let color = match is_hovered {
                    true => *state.get(&client_theme().global().hovered_drop_area_color()),
                    false => *state.get(&client_theme().global().drop_area_color()),
                };
                color.multiply_alpha(*state.get(&client_theme().global().fill_alpha()))
            } else if is_hovered {
                *state.get(&client_theme().button().hovered_background_color())
            } else if occupied {
                *state.get(&client_theme().skill_tree().slot_background_color())
            } else {
                *state.get(&client_theme().skill_tree().unoccupied_slot_color())
            };
            layout.add_rectangle(
                area,
                *state.get(&client_theme().skill_tree().slot_corner_diameter()),
                background,
                *state.get(&client_theme().button().shadow_color()),
                *state.get(&client_theme().skill_tree().slot_outline()),
            );
            if is_hovered {
                layout.set_hovered();
                layout.register_drop_handler(&self.drops[visual as usize]);
                if occupied {
                    layout.register_click_handler(MouseButton::Left, &self.drags[visual as usize]);
                    layout.register_click_handler(MouseButton::DoubleLeft, &self.activates[visual as usize]);
                    layout.register_click_handler(MouseButton::Right, &self.clears[visual as usize]);
                }
            }
            if !self.hints[visual as usize].is_empty() {
                layout.add_text(
                    area,
                    self.hints[visual as usize].as_str(),
                    hint_font,
                    hint_color,
                    hint_color,
                    HorizontalAlignment::Left { offset: 2.0, border: 2.0 },
                    VerticalAlignment::Top { offset: 1.0 },
                    OverflowBehavior::Shrink,
                );
            }
            let Some(cell) = cell else {
                continue;
            };
            if cell.kind == CELL_SKILL
                && let Some(skill) = cell.skill.as_ref()
            {
                if let (Some(actions), Some(sprite)) = (skill.actions.as_ref(), skill.sprite.as_ref()) {
                    layout.add_sprite(area, actions, sprite, &skill.animation_state, Color::WHITE, 1.0);
                }
                add_skill_cooldown(layout, state, area, skill.skill_id);
                if is_hovered {
                    struct HotbarTooltip;
                    layout.add_tooltip(skill.skill_name.as_str(), HotbarTooltip.tooltip_id());
                }
            } else if cell.kind == CELL_ITEM
                && let Some(texture) = cell.item_texture.as_ref()
            {
                let side = area.width.min(area.height) * 0.8;
                let texture_area = Area {
                    left: area.left + (area.width - side) / 2.0,
                    top: area.top + (area.height - side) / 2.0,
                    width: side,
                    height: side,
                };
                layout.add_texture(texture_area, texture.clone(), Color::WHITE, false);
                if is_hovered && !cell.item_name.is_empty() {
                    struct HotbarTooltip;
                    layout.add_tooltip(cell.item_name.as_str(), HotbarTooltip.tooltip_id());
                }
            }
            if !self.labels[visual as usize].is_empty() {
                layout.add_text(
                    area,
                    self.labels[visual as usize].as_str(),
                    amount_font,
                    *state.get(&client_theme().text().color()),
                    *state.get(&client_theme().text().highlight_color()),
                    HorizontalAlignment::Right { offset: 2.0, border: 2.0 },
                    VerticalAlignment::Bottom { offset: 1.0 },
                    OverflowBehavior::Shrink,
                );
            }
            let name = self.names[visual as usize].shown.as_str();
            if occupied && !name.is_empty() {
                layout.add_text(
                    label_area,
                    name,
                    name_font,
                    label_color,
                    label_color,
                    HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                    VerticalAlignment::Top { offset: 1.0 },
                    OverflowBehavior::Shrink,
                );
            }
        }
    }
}
