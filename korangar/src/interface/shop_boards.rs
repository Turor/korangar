//! Shop title boards drawn above vending and buying-store players.
//!
//! The board sits above a point `HEAD_OFFSET_Y` over the entity origin. Screen
//! Y grows downward, so the rectangle is placed above the projected head.

use cgmath::{Point3, Vector3};
use ragnarok_packets::AccountId;
use rust_state::State;

use crate::graphics::{ScreenPosition, ScreenSize};
use crate::loaders::FontSize;
use crate::renderer::{AlignHorizontal, GameInterfaceRenderer};
use crate::state::{ClientState, ClientStatePathExt, client_state};
use crate::world::{Camera, EntityType};

/// World-space lift from the entity origin to the head the board hangs over.
pub const HEAD_OFFSET_Y: f32 = 10.0;
const BOARD_HEIGHT: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShopBoardHit {
    pub account_id: AccountId,
    pub buying_store: bool,
}

/// Board centered on `anchor_x` and sitting just above `anchor_y`.
pub fn board_rect(anchor_x: f32, anchor_y: f32, title: &str) -> BoardRect {
    let width = (title.chars().count() as f32 * 7.5 + 16.0).clamp(72.0, 240.0);
    BoardRect {
        left: anchor_x - width * 0.5,
        top: anchor_y - BOARD_HEIGHT - 6.0,
        width,
        height: BOARD_HEIGHT,
    }
}

pub fn board_contains(rect: BoardRect, x: f32, y: f32) -> bool {
    x >= rect.left && y >= rect.top && x <= rect.left + rect.width && y <= rect.top + rect.height
}

pub fn render_shop_boards(
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
    for board in projected_boards(camera, state, screen_size) {
        let fill = if board_contains(board.rect, mouse.left, mouse.top) {
            hover
        } else {
            background
        };
        renderer.render_rectangle(
            ScreenPosition {
                left: board.rect.left,
                top: board.rect.top,
            },
            ScreenSize {
                width: board.rect.width,
                height: board.rect.height,
            },
            fill,
        );
        renderer.render_text(
            &board.title,
            ScreenPosition {
                left: board.rect.left + board.rect.width * 0.5,
                top: board.rect.top + 4.0,
            },
            foreground,
            FontSize(14.0),
            AlignHorizontal::Center,
        );
    }
}

pub fn shop_board_hit(
    camera: &dyn Camera,
    state: &State<ClientState>,
    screen_size: ScreenSize,
    mouse: ScreenPosition,
) -> Option<ShopBoardHit> {
    projected_boards(camera, state, screen_size).into_iter().find_map(|board| {
        board_contains(board.rect, mouse.left, mouse.top).then_some(ShopBoardHit {
            account_id: board.account_id,
            buying_store: board.buying_store,
        })
    })
}

struct ProjectedBoard {
    account_id: AccountId,
    buying_store: bool,
    title: String,
    rect: BoardRect,
}

fn projected_boards(camera: &dyn Camera, state: &State<ClientState>, screen_size: ScreenSize) -> Vec<ProjectedBoard> {
    let shops = state.follow(client_state().player_shops()).clone();
    let entities = state.follow(client_state().entities());
    let mut boards = Vec::new();
    for shop in shops {
        let title = shop.shop_name.trim();
        if title.is_empty() {
            continue;
        }
        let Some(position) = entities.iter().find_map(|entity| {
            (entity.get_entity_type() == EntityType::Player && entity.get_entity_id().0 == shop.account_id.0)
                .then_some(entity.get_position())
        }) else {
            continue;
        };
        let Some((anchor_x, anchor_y)) = project_head(camera, position, screen_size) else {
            continue;
        };
        boards.push(ProjectedBoard {
            account_id: shop.account_id,
            buying_store: shop.buying_store,
            title: title.to_owned(),
            rect: board_rect(anchor_x, anchor_y, title),
        });
    }
    boards
}

pub(crate) fn project_head(camera: &dyn Camera, position: Point3<f32>, screen_size: ScreenSize) -> Option<(f32, f32)> {
    let head = position + Vector3::new(0.0, HEAD_OFFSET_Y, 0.0);
    let clip = camera.view_projection_matrix() * head.to_homogeneous();
    if clip.w <= 0.0 {
        return None;
    }
    let uv = camera.clip_to_screen_space(clip);
    Some((uv.x * screen_size.width, uv.y * screen_size.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_sits_above_the_anchor_and_rejects_outside_clicks() {
        let rect = board_rect(100.0, 100.0, "Hi");
        assert_eq!(rect.width, 72.0);
        assert!((rect.left - 64.0).abs() < f32::EPSILON);
        assert!((rect.top - 72.0).abs() < f32::EPSILON);
        assert!(board_contains(rect, 100.0, 80.0));
        assert!(!board_contains(rect, 10.0, 10.0));
    }
}
