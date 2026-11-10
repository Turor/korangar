use std::sync::Arc;

use korangar_interface::application::Clip;
use ragnarok_packets::ClientTick;

use crate::graphics::{Color, ScreenClip, ScreenPosition, ScreenSize};
use crate::input::Grabbed;
use crate::loaders::{ActionLoader, FALLBACK_SPRITE_FILE, Sprite, SpriteLoader};
use crate::renderer::{GameInterfaceRenderer, SpriteRenderer};
use crate::world::{Actions, SpriteAnimationState};

const PICKUP_DURATION_MS: u32 = 150;
/// How often a missing `cursors.spr` is fetched again.
const CURSOR_RETRY_MS: u32 = 3000;
const CURSOR_SPRITE_FILE: &str = "cursors.spr";
const CURSOR_ACTIONS_FILE: &str = "cursors.act";

/// What the cursor draws on the top layer, the only layer above every window.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CursorDraw {
    /// The `cursors.spr` animation.
    Sprite,
    /// A small arrow. Used while `cursors.spr` is missing, so the placeholder
    /// character sprite never shows on top of a window.
    Arrow,
}

pub fn cursor_draw(uses_fallback: bool) -> CursorDraw {
    if uses_fallback { CursorDraw::Arrow } else { CursorDraw::Sprite }
}

/// `true` when the loader returned the shared placeholder instead of `path`.
fn is_fallback(sprite_loader: &SpriteLoader, sprite: &Arc<Sprite>) -> bool {
    sprite_loader
        .get(FALLBACK_SPRITE_FILE)
        .is_some_and(|fallback| Arc::ptr_eq(&fallback, sprite))
}

/// Rows of the fallback arrow as `(left, top, width, outline)` in unscaled
/// pixels from the hotspot. The tip sits on the mouse position.
pub fn fallback_arrow_rows() -> Vec<(f32, f32, f32, bool)> {
    const ROWS: usize = 13;
    let mut rows = Vec::with_capacity(ROWS * 2);
    for row in 0..ROWS {
        let width = (row as f32 * 0.75).floor() + 1.0;
        rows.push((0.0, row as f32, width + 2.0, true));
        if row > 0 && row + 1 < ROWS {
            rows.push((1.0, row as f32, width, false));
        }
    }
    rows
}

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MouseCursorState {
    Default = 0,
    Dialog = 1,
    Click = 2,
    Unsure0 = 3,
    RotateCamera = 4,
    Attack = 5,
    Attack1 = 6,
    Warp = 7,
    NoAction = 8,
    Grab = 9,
    Unsure1 = 10,
    Unsure2 = 11,
    WarpFast = 12,
    Unsure3 = 13,
    /// Not an orignial state, represented as part of this enum to make the API
    /// more ergonomic. Will likely be refactored with this entire module at
    /// some point.
    HoverItem = 900,
    GrabResource,
    PickUpItem,
}

impl From<MouseCursorState> for usize {
    fn from(value: MouseCursorState) -> Self {
        value as usize
    }
}

pub struct MouseCursor {
    sprite: Arc<Sprite>,
    actions: Arc<Actions>,
    cursor_state: MouseCursorState,
    animation_state: SpriteAnimationState,
    locked_until: ClientTick,
    shown: bool,
    uses_fallback: bool,
    retry_at: ClientTick,
}

impl MouseCursor {
    pub fn new(sprite_loader: &SpriteLoader, action_loader: &ActionLoader) -> Self {
        let sprite = sprite_loader.get_or_load(CURSOR_SPRITE_FILE).unwrap();
        let actions = action_loader.get_or_load(CURSOR_ACTIONS_FILE).unwrap();
        let uses_fallback = is_fallback(sprite_loader, &sprite);
        let animation_state = SpriteAnimationState::new(ClientTick(0));
        let locked_until = ClientTick(0);
        let shown = true;

        Self {
            sprite,
            actions,
            cursor_state: MouseCursorState::Default,
            animation_state,
            locked_until,
            shown,
            uses_fallback,
            retry_at: ClientTick(CURSOR_RETRY_MS),
        }
    }

    pub fn draw_mode(&self) -> CursorDraw {
        cursor_draw(self.uses_fallback)
    }

    /// Try `cursors.spr` again while the placeholder is in use (the remote
    /// asset host may not have answered at startup).
    pub fn retry_load(&mut self, sprite_loader: &SpriteLoader, action_loader: &ActionLoader, client_tick: ClientTick) {
        if !self.uses_fallback || client_tick.0 < self.retry_at.0 {
            return;
        }
        self.retry_at = ClientTick(client_tick.0.saturating_add(CURSOR_RETRY_MS));
        let Ok(sprite) = sprite_loader.get_or_load(CURSOR_SPRITE_FILE) else {
            return;
        };
        if is_fallback(sprite_loader, &sprite) {
            return;
        }
        if let Ok(actions) = action_loader.get_or_load(CURSOR_ACTIONS_FILE) {
            self.sprite = sprite;
            self.actions = actions;
            self.uses_fallback = false;
        }
    }

    fn render_arrow(&self, renderer: &GameInterfaceRenderer, mouse_position: ScreenPosition, color: Color, scaling: f32) {
        let outline = Color::rgba_u8(0, 0, 0, 220);
        for (left, top, width, is_outline) in fallback_arrow_rows() {
            renderer.render_rectangle(
                ScreenPosition {
                    left: mouse_position.left + left * scaling,
                    top: mouse_position.top + top * scaling,
                },
                ScreenSize {
                    width: width * scaling,
                    height: scaling,
                },
                if is_outline { outline } else { color },
            );
        }
    }

    pub fn hide(&mut self) {
        self.shown = false;
    }

    pub fn show(&mut self) {
        self.shown = true;
    }

    pub fn update(&mut self, client_tick: ClientTick) {
        self.animation_state.update(client_tick);
    }

    pub fn set_state(&mut self, state: MouseCursorState, client_tick: ClientTick) {
        if client_tick.0 > self.locked_until.0 {
            // Cursor is unlocked
            if self.cursor_state != state {
                self.cursor_state = state;

                let base_offset = match state {
                    MouseCursorState::PickUpItem => {
                        // Lock the cursor.
                        self.locked_until = ClientTick(client_tick.0 + PICKUP_DURATION_MS);

                        usize::from(MouseCursorState::Grab)
                    }
                    MouseCursorState::GrabResource => usize::from(MouseCursorState::Grab),
                    MouseCursorState::HoverItem => usize::from(MouseCursorState::Grab),
                    regular => usize::from(regular),
                };

                self.animation_state.action_base_offset = base_offset;
                self.animation_state.start_time = client_tick;
            }
        } else if self.cursor_state == state {
            // Cursor is locked, but we can still extend the duration of the state.
            self.locked_until = ClientTick(client_tick.0 + PICKUP_DURATION_MS);
        }
    }

    #[cfg_attr(feature = "debug", korangar_debug::profile("render mouse cursor"))]
    pub fn render(
        &self,
        renderer: &GameInterfaceRenderer,
        mouse_position: ScreenPosition,
        grabbed: Option<Grabbed>,
        color: Color,
        scaling: f32,
    ) {
        if !self.shown {
            return;
        }

        // Adjust the position of the mouse cursor based on the interface scale. At 1.0
        // the cursos is in the perfect position but for everything else the
        // sprite drifts from the mouse position. This might be cause by how we
        // scale sprites, needs further investigation.
        //
        // Values picked by testing. Can this be derived somehow?
        let mouse_position = ScreenPosition {
            left: mouse_position.left + 10.0 * (scaling - 1.0),
            top: mouse_position.top + 14.0 * (scaling - 1.0),
        };

        if let Some(grabbed) = grabbed {
            match grabbed {
                Grabbed::Texture(texture) => renderer.render_sprite(
                    texture.clone(),
                    mouse_position - ScreenSize::uniform(15.0 * scaling),
                    ScreenSize::uniform(30.0 * scaling),
                    ScreenClip::unbound(),
                    Color::WHITE,
                    false,
                ),
                Grabbed::Action(sprite, actions, animation_state) => actions.render_sprite(
                    renderer,
                    &sprite,
                    &animation_state,
                    mouse_position,
                    0,
                    ScreenClip::unbound(),
                    Color::WHITE,
                    scaling,
                ),
            }
        }

        if self.draw_mode() == CursorDraw::Arrow {
            self.render_arrow(renderer, mouse_position, color, scaling);
            return;
        }

        // TODO: Figure out how this is actually supposed to work
        let direction = match self.cursor_state {
            MouseCursorState::Default | MouseCursorState::Click | MouseCursorState::RotateCamera => 0,
            _ => 7,
        };

        // TODO: Is there some deeper logic here?
        const HOVER_ITEM_FRAME: usize = 0;
        const PICKUP_FRAME: usize = 2;

        let frame_index = match self.cursor_state {
            MouseCursorState::HoverItem => Some(HOVER_ITEM_FRAME),
            MouseCursorState::PickUpItem => Some(PICKUP_FRAME),
            MouseCursorState::GrabResource => Some(PICKUP_FRAME),
            _ => None,
        };

        if let Some(frame_index) = frame_index {
            self.actions.render_sprite_frame(
                renderer,
                &self.sprite,
                self.animation_state.get_action_index(direction),
                frame_index,
                mouse_position,
                ScreenClip::unbound(),
                color,
                scaling,
            );
        } else {
            self.actions.render_sprite(
                renderer,
                &self.sprite,
                &self.animation_state,
                mouse_position,
                direction,
                ScreenClip::unbound(),
                color,
                scaling,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CursorDraw, cursor_draw, fallback_arrow_rows};

    #[test]
    fn a_missing_cursor_sprite_draws_an_arrow_not_the_placeholder() {
        assert_eq!(cursor_draw(true), CursorDraw::Arrow);
        assert_eq!(cursor_draw(false), CursorDraw::Sprite);
    }

    #[test]
    fn the_fallback_arrow_is_small_and_starts_at_the_hotspot() {
        let rows = fallback_arrow_rows();
        assert!(!rows.is_empty());
        assert!(rows.iter().any(|row| row.0 == 0.0 && row.1 == 0.0));
        for (left, top, width, _) in rows {
            assert!(left >= 0.0 && top >= 0.0);
            assert!(left + width <= 16.0 && top < 16.0, "arrow row exceeds 16px");
        }
    }
}
