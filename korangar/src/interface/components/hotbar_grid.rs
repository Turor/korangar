//! Hotbar slots. Skills and items share one grid. F-key rows are visual;
//! [`stored_slot`](crate::state::hotbar::stored_slot) maps them through the row
//! shift.

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
use crate::interface::resource::{ItemSource, SkillSource};
use crate::loaders::{FontSize, OverflowBehavior};
use crate::renderer::LayoutExt;
use crate::state::hotbar::{
    CELL_ITEM, CELL_SKILL, HOTBAR_COLUMNS, HOTBAR_ROWS, HOTBAR_SLOTS, Hotbar, cell_item_id, cell_skill_level, hotbar_stack_label,
};
use crate::state::inventory::Inventory;
use crate::state::theme::{GlobalThemePathExt, InterfaceThemePathExt, SkillTreeThemePathExt};
use crate::state::{ClientState, client_theme};

const CELL: f32 = 40.0;

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

pub struct HotbarGrid<H, I> {
    hotbar_path: H,
    inventory_path: I,
    activates: [ActivateHandler<H>; HOTBAR_SLOTS],
    clears: [ClearHandler<H>; HOTBAR_SLOTS],
    drags: [DragHandler<H, I>; HOTBAR_SLOTS],
    drops: [SlotDrop<H>; HOTBAR_SLOTS],
    labels: [String; HOTBAR_SLOTS],
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
        }
    }
}

pub struct HotbarGridLayout {
    area: Area,
    rows: u8,
    show_quick: bool,
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
            let rows = hotbar.visible_rows();
            let show_quick = rows == HOTBAR_ROWS as u8;
            let drawn_rows = u32::from(rows) + u32::from(show_quick);
            for visual in 0..HOTBAR_SLOTS {
                let (tab, slot) = hotbar.visual_to_stored(visual as u16);
                let Some(cell) = hotbar.cell(tab, slot) else {
                    self.labels[visual].clear();
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
            }
            HotbarGridLayout {
                area: resolver.with_height(CELL * drawn_rows as f32),
                rows,
                show_quick,
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
        let column_width = layout_info.area.width / HOTBAR_COLUMNS as f32;
        let mut draw = |visual: u16, column: usize, row: usize| {
            let area = Area {
                left: layout_info.area.left + column_width * column as f32,
                top: layout_info.area.top + CELL * row as f32,
                width: column_width,
                height: CELL,
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
            let Some(cell) = cell else {
                return;
            };
            if cell.kind == CELL_SKILL
                && let Some(skill) = cell.skill.as_ref()
            {
                if let (Some(actions), Some(sprite)) = (skill.actions.as_ref(), skill.sprite.as_ref()) {
                    layout.add_sprite(area, actions, sprite, &skill.animation_state, Color::WHITE, 1.0);
                }
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
                    FontSize(12.0),
                    *state.get(&client_theme().text().color()),
                    *state.get(&client_theme().text().highlight_color()),
                    HorizontalAlignment::Right { offset: 2.0, border: 2.0 },
                    VerticalAlignment::Bottom { offset: 1.0 },
                    OverflowBehavior::Shrink,
                );
            }
        };

        for row in 0..layout_info.rows as usize {
            for column in 0..HOTBAR_COLUMNS {
                draw((row * HOTBAR_COLUMNS + column) as u16, column, row);
            }
        }
        if layout_info.show_quick {
            let row = layout_info.rows as usize;
            draw(36, 0, row);
            draw(37, 1, row);
        }
    }
}
