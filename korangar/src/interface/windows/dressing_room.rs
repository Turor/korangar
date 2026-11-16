//! Dressing room. Ctrl+click in the bag, storage, or cart layers one view
//! sprite on the player's own paperdoll. Closing the window sends nothing.

use std::sync::Arc;

use korangar_interface::element::store::{ElementStore, ElementStoreMut};
use korangar_interface::element::{BaseLayoutInfo, Element};
use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::alignment::{HorizontalAlignment, VerticalAlignment};
use korangar_interface::layout::{MouseButton, Resolvers, WindowLayout, with_single_resolver};
use korangar_interface::window::{CustomWindow, Window};
use korangar_networking::InventoryItem;
use ragnarok_packets::EntityId;
use rust_state::State;

use super::WindowClass;
use super::equipment::{fit_sprite_part, part_is_drawable, sprite_part_top_left};
use crate::graphics::{CornerDiameter, ShadowPadding};
use crate::interface::resource::ItemSource;
use crate::loaders::{AsyncLoader, FontSize, OverflowBehavior, SpriteLoader};
use crate::renderer::LayoutExt;
use crate::state::dressing_room::{self, DressingRoom};
use crate::state::inventory::InventoryPathExt;
use crate::state::theme::{InterfaceThemePathExt, InterfaceThemeType};
use crate::state::{ClientState, ClientStatePathExt, client_state, client_theme, this_player};
use crate::world::{AnimationData, AnimationFrame, EntityType, Library, ResourceMetadata};

const PORTRAIT: f32 = 200.0;
const NOTICE: f32 = 18.0;
const BUTTON: f32 = 28.0;
const GAP: f32 = 6.0;
const VIEW_HEIGHT: f32 = PORTRAIT + NOTICE + BUTTON + BUTTON + GAP * 3.0;

const TURN_LEFT: u8 = 0;
const TURN_RIGHT: u8 = 1;
const RESET: u8 = 2;
const TOGGLE_HIDE: u8 = 3;

/// Ctrl+left-click in the bag, storage, or cart tries the item on.
pub fn queues_preview(ctrl_down: bool, source: ItemSource) -> bool {
    ctrl_down && matches!(source, ItemSource::Inventory | ItemSource::Storage | ItemSource::Cart)
}

/// Layer `item` on the local character. No packet is sent.
pub fn apply_item_preview(
    state: &mut State<ClientState>,
    library: &Library,
    sprites: &SpriteLoader,
    item: &InventoryItem<ResourceMetadata>,
) {
    let worn = {
        let player = state.try_follow(this_player());
        let items = state.follow(client_state().inventory().items());
        dressing_room::worn_look_from_player(player, items)
    };
    state.follow_mut(client_state().dressing_room()).apply_item(
        &worn,
        item,
        |view_id| library.accessory_sprite_suffix(view_id).map(str::to_owned),
        |path| sprite_exists(sprites, path),
    );
}

fn sprite_exists(sprites: &SpriteLoader, path: &str) -> bool {
    sprites.sprite_file_exists(&format!("{path}.spr")) && sprites.sprite_file_exists(&format!("{path}.act"))
}

/// Ask the animation loader for the portrait while the window is open.
/// A closed window drops the portrait and still sends nothing.
pub fn sync_dressing_room(room: &mut DressingRoom, async_loader: &AsyncLoader, window_open: bool) {
    if !window_open {
        room.clear_loaded();
        return;
    }
    let Some((entity_id, files)) = room.take_load_request() else {
        return;
    };
    if let Some(animation) = async_loader.request_animation_data_load(EntityId(entity_id), EntityType::Player, files) {
        room.store_animation(entity_id, animation);
    }
}

/// Returns the animation unless this dressing-room preview accepts it.
/// A dressing-room id is never applied to a world entity.
pub fn take_dressing_animation(room: &mut DressingRoom, entity_id: EntityId, animation: Arc<AnimationData>) -> Option<Arc<AnimationData>> {
    if !dressing_room::is_dressing_preview(entity_id.0) {
        return Some(animation);
    }
    room.store_animation(entity_id.0, Arc::clone(&animation));
    None
}

struct DressingClick {
    action: u8,
}

impl ClickHandler<ClientState> for DressingClick {
    fn handle_click(&self, state: &State<ClientState>, _: &mut EventQueue<ClientState>) {
        let action = self.action;
        state.update_value_with(client_state().dressing_room(), move |room| match action {
            TURN_LEFT => room.rotate(1),
            TURN_RIGHT => room.rotate(-1),
            RESET => room.reset(),
            TOGGLE_HIDE => room.toggle_hide_other_headgear(),
            _ => {}
        });
    }
}

struct DressingRoomView {
    turn_left: DressingClick,
    turn_right: DressingClick,
    reset: DressingClick,
    toggle: DressingClick,
    /// Copied in layout so the text instruction can borrow it for the frame.
    notice: String,
}

impl DressingRoomView {
    fn new() -> Self {
        Self {
            turn_left: DressingClick { action: TURN_LEFT },
            turn_right: DressingClick { action: TURN_RIGHT },
            reset: DressingClick { action: RESET },
            toggle: DressingClick { action: TOGGLE_HIDE },
            notice: String::new(),
        }
    }
}

impl Element<ClientState> for DressingRoomView {
    type LayoutInfo = BaseLayoutInfo;

    fn create_layout_info(
        &mut self,
        state: &State<ClientState>,
        _: ElementStoreMut,
        resolvers: &mut dyn Resolvers<ClientState>,
    ) -> Self::LayoutInfo {
        let room_path = client_state().dressing_room();
        self.notice = state.get(&room_path).notice().to_owned();
        with_single_resolver(resolvers, |resolver| {
            let area = resolver.with_height(VIEW_HEIGHT);
            BaseLayoutInfo { area }
        })
    }

    fn lay_out<'a>(
        &'a self,
        state: &'a State<ClientState>,
        _: ElementStore<'a>,
        layout_info: &'a Self::LayoutInfo,
        layout: &mut WindowLayout<'a, ClientState>,
    ) {
        let room_path = client_state().dressing_room();
        let token_path = client_theme().tokens();
        let room = state.get(&room_path);
        let tokens = state.get(&token_path);
        let area = layout_info.area;
        let portrait = korangar_interface::layout::area::Area {
            left: area.left,
            top: area.top,
            width: area.width,
            height: PORTRAIT,
        };
        layout.add_rectangle(
            portrait,
            CornerDiameter::uniform(tokens.corner_radius),
            tokens.panel,
            tokens.border,
            ShadowPadding::diagonal(1.0, 2.0),
        );
        if let Some(animation) = room.preview() {
            paint_frame(layout, portrait, animation, room.direction());
        }

        let mut top = area.top + PORTRAIT + GAP;
        if !self.notice.is_empty() {
            layout.add_text(
                korangar_interface::layout::area::Area {
                    left: area.left,
                    top,
                    width: area.width,
                    height: NOTICE,
                },
                &self.notice,
                FontSize(tokens.font_size),
                tokens.muted,
                tokens.border,
                HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
                VerticalAlignment::Center { offset: 0.0 },
                OverflowBehavior::Shrink,
            );
        }
        top += NOTICE;

        let button_width = ((area.width - GAP * 2.0) / 3.0).max(1.0);
        paint_button(
            layout,
            state,
            ButtonBox {
                left: area.left,
                top,
                width: button_width,
                height: BUTTON,
            },
            "Turn left",
            false,
            &self.turn_left,
        );
        paint_button(
            layout,
            state,
            ButtonBox {
                left: area.left + button_width + GAP,
                top,
                width: button_width,
                height: BUTTON,
            },
            "Reset",
            false,
            &self.reset,
        );
        paint_button(
            layout,
            state,
            ButtonBox {
                left: area.left + (button_width + GAP) * 2.0,
                top,
                width: button_width,
                height: BUTTON,
            },
            "Turn right",
            false,
            &self.turn_right,
        );
        top += BUTTON + GAP;
        paint_button(
            layout,
            state,
            ButtonBox {
                left: area.left,
                top,
                width: area.width,
                height: BUTTON,
            },
            "Hide other headgear",
            room.hide_other_headgear(),
            &self.toggle,
        );
    }
}

struct ButtonBox {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

fn paint_button<'a>(
    layout: &mut WindowLayout<'a, ClientState>,
    state: &'a State<ClientState>,
    button: ButtonBox,
    label: &'static str,
    selected: bool,
    click: &'a dyn ClickHandler<ClientState>,
) {
    let token_path = client_theme().tokens();
    let tokens = state.get(&token_path);
    let area = korangar_interface::layout::area::Area {
        left: button.left,
        top: button.top,
        width: button.width,
        height: button.height,
    };
    let hovered = area.check().run(layout);
    let border = match (selected, hovered) {
        (true, _) => tokens.accent,
        (false, true) => tokens.hover,
        (false, false) => tokens.border,
    };
    layout.add_rectangle(
        area,
        CornerDiameter::uniform(tokens.corner_radius),
        tokens.raised,
        border,
        ShadowPadding::diagonal(1.0, 2.0),
    );
    layout.add_text(
        area,
        label,
        FontSize(tokens.font_size),
        tokens.text,
        tokens.border,
        HorizontalAlignment::Center { offset: 0.0, border: 0.0 },
        VerticalAlignment::Center { offset: 0.0 },
        OverflowBehavior::Shrink,
    );
    if hovered {
        layout.register_click_handler(MouseButton::Left, click);
    }
}

fn paint_frame(
    layout: &mut WindowLayout<ClientState>,
    area: korangar_interface::layout::area::Area,
    animation: &AnimationData,
    direction: u8,
) {
    let Some(frame) = idle_frame(animation, direction) else {
        return;
    };
    let inset = 8.0;
    let box_size = (area.width - inset * 2.0).min(area.height - inset * 2.0);
    if box_size <= 1.0 {
        return;
    }
    let box_left = area.left + (area.width - box_size) / 2.0;
    let box_top = area.top + (area.height - box_size) / 2.0;
    let mut union: Option<(i32, i32, i32, i32)> = None;
    for part in frame.frame_parts.iter().filter(|part| part_is_drawable(part, animation)) {
        let (x, y) = sprite_part_top_left(part.offset.x, part.offset.y, part.size.x, part.size.y);
        union = Some(match union {
            None => (x, y, x + part.size.x, y + part.size.y),
            Some((min_x, min_y, max_x, max_y)) => (
                min_x.min(x),
                min_y.min(y),
                max_x.max(x + part.size.x),
                max_y.max(y + part.size.y),
            ),
        });
    }
    let Some((min_x, min_y, max_x, max_y)) = union else {
        return;
    };
    for part in frame.frame_parts.iter().filter(|part| part_is_drawable(part, animation)) {
        let (x, y) = sprite_part_top_left(part.offset.x, part.offset.y, part.size.x, part.size.y);
        let (left, top, width, height) = fit_sprite_part(
            min_x,
            min_y,
            max_x - min_x,
            max_y - min_y,
            x,
            y,
            part.size.x,
            part.size.y,
            box_left,
            box_top,
            box_size,
        );
        let texture = animation.animation_pair[part.animation_index].sprites.textures[part.sprite_number].clone();
        layout.add_texture(
            korangar_interface::layout::area::Area { left, top, width, height },
            texture,
            part.color,
            false,
        );
    }
}

fn idle_frame(animation: &AnimationData, direction: u8) -> Option<&AnimationFrame> {
    let count = animation.animations.len().min(8);
    if count == 0 {
        return None;
    }
    animation
        .animations
        .get(usize::from(direction) % count)
        .and_then(|animation| animation.frames.first())
}

pub struct DressingRoomWindow;

impl DressingRoomWindow {
    pub fn new() -> Self {
        Self
    }
}

impl CustomWindow<ClientState> for DressingRoomWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::DressingRoom)
    }

    fn to_window<'a>(self) -> impl Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        window! {
            title: "Preview",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            minimum_width: 280.0,
            maximum_width: 320.0,
            elements: (DressingRoomView::new(),),
        }
    }
}

#[cfg(test)]
mod tests {
    use ragnarok_packets::{EquipPosition, HotbarSlot};

    use super::queues_preview;
    use crate::interface::resource::ItemSource;

    #[test]
    fn only_ctrl_click_in_bag_storage_or_cart_previews() {
        assert!(queues_preview(true, ItemSource::Inventory));
        assert!(queues_preview(true, ItemSource::Storage));
        assert!(queues_preview(true, ItemSource::Cart));
        assert!(!queues_preview(false, ItemSource::Inventory));
        assert!(!queues_preview(true, ItemSource::Equipment {
            position: EquipPosition::HEAD_TOP,
        }));
        assert!(!queues_preview(true, ItemSource::Hotbar {
            tab: ragnarok_packets::HotbarTab(0),
            slot: HotbarSlot(0),
        }));
    }
}
