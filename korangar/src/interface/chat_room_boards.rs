//! Chat room title boards drawn above room owners, like roBrowser
//! `EntityRoom`. Click a board to join; private rooms ask for a password.

use ragnarok_packets::ChatRoomId;
use rust_state::State;

use crate::graphics::{ScreenPosition, ScreenSize};
use crate::interface::shop_boards::{BoardRect, board_contains, board_rect, project_head};
use crate::loaders::FontSize;
use crate::renderer::{AlignHorizontal, GameInterfaceRenderer};
use crate::state::{ClientState, ClientStatePathExt, client_state};
use crate::world::{Camera, EntityType};

struct ProjectedRoom {
    chat_id: ChatRoomId,
    label: String,
    rect: BoardRect,
}

pub fn render_chat_room_boards(
    renderer: &GameInterfaceRenderer,
    camera: &dyn Camera,
    state: &State<ClientState>,
    screen_size: ScreenSize,
    mouse: ScreenPosition,
) {
    let (background, foreground, hover) = {
        let theme = state.follow(client_state().world_theme());
        (
            theme.status_bar.background_color,
            theme.overlay.foreground_color,
            theme.status_bar.activity_point_color,
        )
    };
    for room in projected_rooms(camera, state, screen_size) {
        let fill = match board_contains(room.rect, mouse.left, mouse.top) {
            true => hover,
            false => background,
        };
        renderer.render_rectangle(
            ScreenPosition {
                left: room.rect.left,
                top: room.rect.top,
            },
            ScreenSize {
                width: room.rect.width,
                height: room.rect.height,
            },
            fill,
        );
        renderer.render_text(
            &room.label,
            ScreenPosition {
                left: room.rect.left + room.rect.width * 0.5,
                top: room.rect.top + 4.0,
            },
            foreground,
            FontSize(14.0),
            AlignHorizontal::Center,
        );
    }
}

pub fn chat_room_board_hit(
    camera: &dyn Camera,
    state: &State<ClientState>,
    screen_size: ScreenSize,
    mouse: ScreenPosition,
) -> Option<ChatRoomId> {
    projected_rooms(camera, state, screen_size)
        .into_iter()
        .find_map(|room| board_contains(room.rect, mouse.left, mouse.top).then_some(room.chat_id))
}

fn projected_rooms(camera: &dyn Camera, state: &State<ClientState>, screen_size: ScreenSize) -> Vec<ProjectedRoom> {
    let boards = state.follow(client_state().chat_room()).boards().to_vec();
    let entities = state.follow(client_state().entities());
    boards
        .into_iter()
        .filter_map(|board| {
            let position = entities.iter().find_map(|entity| {
                (entity.get_entity_type() == EntityType::Player && entity.get_entity_id().0 == board.owner_id.0)
                    .then_some(entity.get_position())
            })?;
            let (anchor_x, anchor_y) = project_head(camera, position, screen_size)?;
            let label = board.label();
            let rect = board_rect(anchor_x, anchor_y, &label);
            Some(ProjectedRoom {
                chat_id: board.chat_id,
                label,
                rect,
            })
        })
        .collect()
}
