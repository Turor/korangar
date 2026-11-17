//! Hotbar slots. Skills and items share one grid of rows of nine slots;
//! visual rows map through the row shift with
//! [`stored_slot`](crate::state::hotbar::stored_slot).
//!
//! The grid fills the region the (resizable) hotbar window gives it, sized by
//! [`hotbar_metrics`] (Matthew t274u, t280u): rows never wrap, the icon fills
//! the slot, the level (multi-level skills only) or stack sits bottom-right,
//! the hotkey sits in a strip under the slot, and a `+` / `-` pair at the
//! right end of the F-key row adds or removes rows.

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
use crate::interface::components::hotbar_actions::{hotbar_info_event, hotbar_item};
use crate::interface::components::hotbar_layout::{
    HOTBAR_HINT_FONT_PX, HOTBAR_ROW_CONTROLS_WIDTH, HotbarMetrics, hint_strip_height, hotbar_metrics, shows_skill_level, visual_slot,
};
use crate::interface::components::item_tooltip::inventory_tooltips;
use crate::interface::font_scale::{self, from_theme_body};
use crate::interface::resource::{ItemSource, SkillSource};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::hotbar::{
    CELL_ITEM, CELL_SKILL, HOTBAR_COLUMNS, HOTBAR_ROWS, HOTBAR_SLOTS, Hotbar, cell_item_id, cell_skill_level, hotbar_stack_label,
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

/// Right-click: the skill description or the item details, never a clear
/// (Matthew t278u). Removing is a drag off the bar.
struct InfoHandler<H, I> {
    slot: SlotLocate<H>,
    inventory_path: I,
}

impl<H, I> ClickHandler<ClientState> for InfoHandler<H, I>
where
    H: Path<ClientState, Hotbar>,
    I: Path<ClientState, Inventory>,
{
    fn handle_click(&self, state: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        let (tab, slot) = self.slot.tab_slot(state);
        let cell = state.get(&self.slot.hotbar_path).cell(tab, slot);
        let inventory = state.get(&self.inventory_path);
        let event = hotbar_info_event(
            cell,
            |item_id| inventory.find_by_item_id(item_id).cloned(),
            |item| inventory_tooltips(state, item, ItemSource::Inventory),
        );
        if let Some(event) = event {
            queue.queue(event);
        }
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
            && let Some(item) = hotbar_item(cell, state.get(&self.inventory_path).find_by_item_id(item_id))
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

/// `+` or `-` at the right end of the F-key row.
struct RowControl {
    add: bool,
}

impl ClickHandler<ClientState> for RowControl {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(match self.add {
            true => InputEvent::AddHotbarRow,
            false => InputEvent::RemoveHotbarRow,
        });
    }
}

pub struct HotbarGrid<H, I> {
    hotbar_path: H,
    inventory_path: I,
    activates: [ActivateHandler<H>; HOTBAR_SLOTS],
    infos: [InfoHandler<H, I>; HOTBAR_SLOTS],
    drags: [DragHandler<H, I>; HOTBAR_SLOTS],
    drops: [SlotDrop<H>; HOTBAR_SLOTS],
    add_row: RowControl,
    remove_row: RowControl,
    /// Skill level (multi-level skills) or stack amount, bottom-right.
    labels: [String; HOTBAR_SLOTS],
    /// Hotkey, in the strip under the slot.
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
            infos: std::array::from_fn(|visual| InfoHandler {
                slot: SlotLocate {
                    visual: visual as u16,
                    hotbar_path,
                },
                inventory_path,
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
            add_row: RowControl { add: true },
            remove_row: RowControl { add: false },
            labels: std::array::from_fn(|_| String::new()),
            hints: std::array::from_fn(|_| String::new()),
        }
    }
}

pub struct HotbarGridLayout {
    area: Area,
    metrics: HotbarMetrics,
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
        with_single_resolver(resolvers, |resolver| {
            let hotbar = state.get(&self.hotbar_path);
            let inventory = state.get(&self.inventory_path);
            let keybinds_path = client_state().keybinds();
            let keybinds = state.get(&keybinds_path);

            let body_font = *state.get(&client_theme().tokens().font_size());
            let hint_font = from_theme_body(body_font, HOTBAR_HINT_FONT_PX);
            // Drawing applies the global UI font scale, so the strip grows
            // with it.
            let hint_height = hint_strip_height(hint_font * font_scale::active_factor());

            // Peek at the area without claiming it: claiming it and then
            // asking for its height again added the gap twice, and the window
            // grew by that gap every frame (Matthew t280u).
            let available = resolver.clone().push_available_area();
            let metrics = hotbar_metrics(
                usize::from(hotbar.visible_rows()),
                available.width,
                available.height,
                hint_height,
            );

            for visual in 0..HOTBAR_SLOTS {
                let (tab, slot) = hotbar.visual_to_stored(visual as u16);
                self.hints[visual] = keybinds.map().slot_hint(visual as u16).unwrap_or_default();
                let Some(cell) = hotbar.cell(tab, slot) else {
                    self.labels[visual].clear();
                    continue;
                };
                match cell.kind {
                    CELL_SKILL => {
                        let maximum = cell.skill.as_ref().map(|skill| skill.maximum_level.0).unwrap_or(0);
                        self.labels[visual] = match shows_skill_level(maximum) {
                            true => cell_skill_level(cell).0.to_string(),
                            false => String::new(),
                        };
                    }
                    CELL_ITEM => {
                        let known = cell_item_id(cell).and_then(|item_id| inventory.known_amount(item_id));
                        let amount = hotbar_stack_label(cell.amount, known);
                        self.labels[visual] = amount.to_string();
                    }
                    _ => self.labels[visual].clear(),
                }
            }

            let area = resolver.with_height(metrics.height);
            HotbarGridLayout { area, metrics }
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
        let origin = layout_info.area;
        let body_font = *state.get(&client_theme().tokens().font_size());
        let hint_font = FontSize(from_theme_body(body_font, HOTBAR_HINT_FONT_PX));
        let amount_font = FontSize(from_theme_body(body_font, 12.0));
        let hint_color = *state.get(&client_theme().hotbar().hint_color());
        let text_color = *state.get(&client_theme().text().color());
        let text_highlight = *state.get(&client_theme().text().highlight_color());

        for row in 0..metrics.rows {
            for column in 0..HOTBAR_COLUMNS {
                let visual = visual_slot(row, column);
                let (left, top) = metrics.slot_origin(column, row);
                let area = Area {
                    left: origin.left + left,
                    top: origin.top + top,
                    width: metrics.slot,
                    height: metrics.slot,
                };
                let hint_area = Area {
                    left: area.left - metrics.gap_x / 2.0,
                    top: area.top + metrics.slot,
                    width: metrics.slot + metrics.gap_x,
                    height: metrics.hint_height,
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
                        layout.register_click_handler(MouseButton::Right, &self.infos[visual as usize]);
                    }
                }
                let hint = self.hints[visual as usize].as_str();
                if !hint.is_empty() {
                    layout.add_text(
                        hint_area,
                        hint,
                        hint_font,
                        hint_color,
                        hint_color,
                        HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                        VerticalAlignment::Top { offset: 0.0 },
                        OverflowBehavior::Shrink,
                    );
                }
                let Some(cell) = cell else {
                    continue;
                };
                // The icon fills the slot, inside a hairline of the slot
                // background.
                let icon_area = Area {
                    left: area.left + 1.0,
                    top: area.top + 1.0,
                    width: (area.width - 2.0).max(0.0),
                    height: (area.height - 2.0).max(0.0),
                };
                if cell.kind == CELL_SKILL
                    && let Some(skill) = cell.skill.as_ref()
                {
                    if let (Some(actions), Some(sprite)) = (skill.actions.as_ref(), skill.sprite.as_ref()) {
                        // Skill icons are 24px sprites; scale them up to the
                        // slot.
                        let scale = icon_area.width / 24.0;
                        layout.add_sprite(icon_area, actions, sprite, &skill.animation_state, Color::WHITE, scale);
                    }
                    add_skill_cooldown(layout, state, area, skill.skill_id);
                    if is_hovered {
                        struct HotbarTooltip;
                        layout.add_tooltip(skill.skill_name.as_str(), HotbarTooltip.tooltip_id());
                    }
                } else if cell.kind == CELL_ITEM
                    && let Some(texture) = cell.item_texture.as_ref()
                {
                    layout.add_texture(icon_area, texture.clone(), Color::WHITE, false);
                    if is_hovered && !cell.item_name.is_empty() {
                        struct HotbarTooltip;
                        layout.add_tooltip(cell.item_name.as_str(), HotbarTooltip.tooltip_id());
                    }
                }
                let amount = self.labels[visual as usize].as_str();
                if !amount.is_empty() {
                    // Above the icon (and the cooldown sweep), or the icon
                    // hides it.
                    layout.with_layer(|layout| {
                        layout.add_text(
                            area,
                            amount,
                            amount_font,
                            text_color,
                            text_highlight,
                            HorizontalAlignment::Right { offset: 2.0, border: 2.0 },
                            VerticalAlignment::Bottom { offset: 1.0 },
                            OverflowBehavior::Shrink,
                        );
                    });
                }
            }
        }

        // `+` over `-`, a column at the right end of the F-key row.
        let (_, f_row_top) = metrics.slot_origin(0, 0);
        let half = ((metrics.slot - 2.0) / 2.0).max(1.0);
        let rows = usize::from(hotbar.visible_rows());
        for (index, (control, text, enabled)) in [(&self.add_row, "+", rows < HOTBAR_ROWS), (&self.remove_row, "-", rows > 1)]
            .into_iter()
            .enumerate()
        {
            let area = Area {
                left: origin.left + metrics.controls_left(),
                top: origin.top + f_row_top + index as f32 * (half + 2.0),
                width: HOTBAR_ROW_CONTROLS_WIDTH,
                height: half,
            };
            let hovered = enabled && area.check().run(layout);
            let background = match hovered {
                true => *state.get(&client_theme().button().hovered_background_color()),
                false => *state.get(&client_theme().skill_tree().slot_background_color()),
            };
            layout.add_rectangle(
                area,
                *state.get(&client_theme().skill_tree().slot_corner_diameter()),
                background,
                *state.get(&client_theme().button().shadow_color()),
                *state.get(&client_theme().skill_tree().slot_outline()),
            );
            let color = match enabled {
                true => text_color,
                false => hint_color,
            };
            layout.add_text(
                area,
                text,
                amount_font,
                color,
                color,
                HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
            if hovered {
                layout.set_hovered();
                layout.register_click_handler(MouseButton::Left, control);
            }
        }
    }
}
