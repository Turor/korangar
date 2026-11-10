mod clipboard;
mod escape;
mod event;
mod key;
mod mode;

use std::mem::variant_count;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ragnarok_packets::ClientTick;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};
use winit::keyboard::KeyCode;

pub(crate) use self::clipboard::{is_paste_shortcut, read_clipboard_text};
pub use self::escape::{EscapeAction, escape_action};
pub use self::event::InputEvent;
pub use self::key::Key;
pub use self::mode::{Grabbed, MouseInputMode, MouseModeExt};
use crate::graphics::{PickerTarget, ScreenPosition, ScreenSize};

const MOUSE_SCOLL_MULTIPLIER: f32 = 30.0;
const KEY_COUNT: usize = variant_count::<KeyCode>();
const DOUBLE_CLICK_TIME_MS: u32 = 250;

#[derive(Debug, Clone, Copy)]
struct PreviousMouseButton {
    button: MouseButton,
    tick: ClientTick,
}

// TODO: Rename
pub struct InputReport {
    pub mouse_click: Option<korangar_interface::layout::MouseButton>,
    pub mouse_position: ScreenPosition,
    pub mouse_delta: ScreenSize,
    pub mouse_button_released: bool,
    pub left_mouse_button_down: bool,
    pub scroll: Option<f32>,
    pub drag: Option<ScreenSize>,
    pub characters: Vec<char>,
    pub mouse_target: PickerTarget,
}

pub struct InputSystem {
    previous_mouse_position: ScreenPosition,
    new_mouse_position: ScreenPosition,
    mouse_delta: ScreenSize,
    previous_scroll_position: f32,
    new_scroll_position: f32,
    scroll_delta: f32,
    left_mouse_button: Key,
    right_mouse_button: Key,
    keys: [Key; KEY_COUNT],
    input_buffer: Vec<char>,
    picker_value: Arc<AtomicU64>,
    previous_mouse_button: Option<PreviousMouseButton>,
    /// Keys and mouse buttons the autopilot must release on the next frame.
    synthetic_release: Vec<KeyCode>,
    synthetic_mouse: bool,
    synthetic_double: bool,
    /// Left button stays down across frames while a scripted drag is running.
    synthetic_hold: bool,
    drag_phase: SyntheticDragPhase,
    drag_end: ScreenPosition,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SyntheticDragPhase {
    Idle,
    MoveNext,
    ReleaseNext,
}

impl InputSystem {
    pub fn new(picker_value: Arc<AtomicU64>) -> Self {
        let previous_mouse_position = ScreenPosition::default();
        let new_mouse_position = ScreenPosition::default();
        let mouse_delta = ScreenSize::default();

        let previous_scroll_position = 0.0;
        let new_scroll_position = 0.0;
        let scroll_delta = 0.0;

        let left_mouse_button = Key::default();
        let right_mouse_button = Key::default();
        let keys = [Key::default(); KEY_COUNT];

        let input_buffer = Vec::new();
        let previous_mouse_button = None;

        Self {
            previous_mouse_position,
            new_mouse_position,
            mouse_delta,
            previous_scroll_position,
            new_scroll_position,
            scroll_delta,
            left_mouse_button,
            right_mouse_button,
            keys,
            input_buffer,
            picker_value,
            previous_mouse_button,
            synthetic_release: Vec::new(),
            synthetic_mouse: false,
            synthetic_double: false,
            synthetic_hold: false,
            drag_phase: SyntheticDragPhase::Idle,
            drag_end: ScreenPosition::default(),
        }
    }

    /// Release keys and buttons armed by the previous autopilot command.
    pub fn expire_synthesized(&mut self) {
        for code in self.synthetic_release.drain(..) {
            self.keys[code as usize].set_down(false);
        }
        if self.synthetic_mouse && !self.synthetic_hold {
            self.left_mouse_button.set_down(false);
            self.right_mouse_button.set_down(false);
            self.synthetic_mouse = false;
        }
    }

    /// Press at `start`, move to `end` on the next frame, and release on the
    /// frame after that. Coordinates are physical pixels.
    pub fn begin_synthetic_drag(&mut self, start_x: f32, start_y: f32, end_x: f32, end_y: f32) {
        let start = ScreenPosition {
            left: start_x,
            top: start_y,
        };
        self.new_mouse_position = start;
        self.previous_mouse_position = start;
        self.drag_end = ScreenPosition { left: end_x, top: end_y };
        self.left_mouse_button.arm_press();
        self.synthetic_hold = true;
        self.synthetic_mouse = false;
        self.drag_phase = SyntheticDragPhase::MoveNext;
    }

    /// Advance a drag started on an earlier frame. Call this before
    /// [`Self::begin_synthetic_drag`] in the same frame.
    pub fn advance_synthetic_drag(&mut self) {
        match self.drag_phase {
            SyntheticDragPhase::Idle => {}
            SyntheticDragPhase::MoveNext => {
                self.new_mouse_position = self.drag_end;
                self.drag_phase = SyntheticDragPhase::ReleaseNext;
            }
            SyntheticDragPhase::ReleaseNext => {
                self.left_mouse_button.set_down(false);
                self.synthetic_hold = false;
                self.drag_phase = SyntheticDragPhase::Idle;
            }
        }
    }

    /// Press a key through the normal keyboard state. `alt+KeyE` reaches the
    /// same bindings as a real Alt+E.
    pub fn synthesize_key(&mut self, code: KeyCode, alt: bool, ctrl: bool, shift: bool) {
        if alt {
            self.arm_key(KeyCode::AltLeft);
        }
        if ctrl {
            self.arm_key(KeyCode::ControlLeft);
        }
        if shift {
            self.arm_key(KeyCode::ShiftLeft);
        }
        self.arm_key(code);
    }

    fn arm_key(&mut self, code: KeyCode) {
        self.keys[code as usize].arm_press();
        self.synthetic_release.push(code);
    }

    /// Move the pointer and click in physical pixels. Callers convert interface
    /// pixels by the interface scale. Double-clicks are forced on this report
    /// because the 250 ms detector depends on the frame rate.
    pub fn synthesize_click(&mut self, x: f32, y: f32, button: korangar_interface::layout::MouseButton) {
        self.new_mouse_position = ScreenPosition { left: x, top: y };
        self.synthetic_double = button == korangar_interface::layout::MouseButton::DoubleLeft;
        match button {
            korangar_interface::layout::MouseButton::Right | korangar_interface::layout::MouseButton::DoubleRight => {
                self.right_mouse_button.arm_press();
            }
            _ => self.left_mouse_button.arm_press(),
        }
        self.synthetic_mouse = true;
    }

    pub fn reset(&mut self) {
        self.left_mouse_button.reset();
        self.right_mouse_button.reset();
        self.keys.iter_mut().for_each(|key| key.reset());
    }

    pub fn update_mouse_position(&mut self, position: PhysicalPosition<f64>) {
        self.new_mouse_position = ScreenPosition {
            left: position.x as f32,
            top: position.y as f32,
        };
    }

    pub fn update_mouse_buttons(&mut self, button: MouseButton, state: ElementState) {
        let pressed = matches!(state, ElementState::Pressed);

        match button {
            MouseButton::Left => self.left_mouse_button.set_down(pressed),
            MouseButton::Right => self.right_mouse_button.set_down(pressed),
            _ignored => {}
        }
    }

    pub fn update_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        match delta {
            MouseScrollDelta::LineDelta(_x, y) => self.new_scroll_position += y * MOUSE_SCOLL_MULTIPLIER,
            MouseScrollDelta::PixelDelta(position) => self.new_scroll_position += position.y as f32,
        }
    }

    pub fn update_keyboard(&mut self, key_code: KeyCode, state: ElementState) {
        let pressed = matches!(state, ElementState::Pressed);
        self.keys[key_code as usize].set_down(pressed);
    }

    pub fn buffer_character(&mut self, character: char) {
        self.input_buffer.push(character);
    }

    pub(crate) fn paste_if_focused(&mut self, focused: bool, read: impl FnOnce() -> Option<String>) {
        if focused && let Some(text) = read() {
            // Pasting must never synthesize Enter/Tab/Escape/Backspace actions.
            // Bound queued characters even for unexpectedly large clipboards.
            self.input_buffer
                .extend(text.chars().filter(|character| !character.is_control()).take(4096));
        }
    }

    #[cfg_attr(feature = "debug", korangar_debug::profile("update input system"))]
    pub fn update_delta(&mut self, client_tick: ClientTick) -> InputReport {
        self.mouse_delta = self.new_mouse_position - self.previous_mouse_position;
        self.previous_mouse_position = self.new_mouse_position;

        self.scroll_delta = self.new_scroll_position - self.previous_scroll_position;
        self.previous_scroll_position = self.new_scroll_position;

        self.left_mouse_button.update();
        self.right_mouse_button.update();
        self.keys.iter_mut().for_each(|key| key.update());

        let mouse_button_released = self.left_mouse_button.released() || self.right_mouse_button.released();

        let last_pixel_value = self.picker_value.load(Ordering::Acquire);
        let mouse_target = PickerTarget::from(last_pixel_value);

        let mut mouse_click = None;

        if self.left_mouse_button.pressed() {
            if let Some(previous_mouse_button) = self.previous_mouse_button
                && previous_mouse_button.button == MouseButton::Left
                && client_tick.0.wrapping_sub(previous_mouse_button.tick.0) <= DOUBLE_CLICK_TIME_MS
            {
                self.previous_mouse_button = None;

                mouse_click = Some(korangar_interface::layout::MouseButton::DoubleLeft);
            } else {
                self.previous_mouse_button = Some(PreviousMouseButton {
                    button: MouseButton::Left,
                    tick: client_tick,
                });

                mouse_click = Some(korangar_interface::layout::MouseButton::Left);
            }
        } else if self.right_mouse_button.pressed() {
            if let Some(previous_mouse_button) = self.previous_mouse_button
                && previous_mouse_button.button == MouseButton::Right
                && client_tick.0.wrapping_sub(previous_mouse_button.tick.0) <= DOUBLE_CLICK_TIME_MS
            {
                self.previous_mouse_button = None;

                mouse_click = Some(korangar_interface::layout::MouseButton::DoubleRight);
            } else {
                self.previous_mouse_button = Some(PreviousMouseButton {
                    button: MouseButton::Right,
                    tick: client_tick,
                });

                mouse_click = Some(korangar_interface::layout::MouseButton::Right);
            }
        }

        if self.synthetic_double {
            self.synthetic_double = false;
            if matches!(
                mouse_click,
                Some(korangar_interface::layout::MouseButton::Left | korangar_interface::layout::MouseButton::DoubleLeft)
            ) {
                mouse_click = Some(korangar_interface::layout::MouseButton::DoubleLeft);
            }
        }

        InputReport {
            mouse_click,
            mouse_position: self.new_mouse_position,
            mouse_delta: self.mouse_delta,
            mouse_button_released,
            left_mouse_button_down: self.left_mouse_button.down(),
            scroll: (self.scroll_delta != 0.0).then_some(self.scroll_delta),
            drag: self.left_mouse_button.down().then_some(self.mouse_delta),
            characters: self.input_buffer.drain(..).collect(),
            mouse_target,
        }
    }

    fn get_key(&self, key_code: KeyCode) -> &Key {
        &self.keys[key_code as usize]
    }

    pub fn key_down(&self, key_code: KeyCode) -> bool {
        self.get_key(key_code).down()
    }

    pub fn key_pressed(&self, key_code: KeyCode) -> bool {
        self.get_key(key_code).pressed()
    }

    pub fn key_released(&self, key_code: KeyCode) -> bool {
        self.get_key(key_code).released()
    }

    /// Escape cancels keybind capture. Otherwise it follows the window stack
    /// (`InputEvent::Escape`). F10 reloads themes and Alt+0 toggles the world
    /// map; the other game chords live in the keybind map. Debug chords stay
    /// hard-coded and are skipped while a row is capturing.
    #[cfg_attr(feature = "debug", korangar_debug::profile)]
    pub fn handle_keyboard_input(
        &mut self,
        events: &mut Vec<InputEvent>,
        capturing: bool,
        #[cfg(feature = "debug")] process_mouse: bool,
        #[cfg(feature = "debug")] use_debug_camera: bool,
    ) -> bool {
        if self.get_key(KeyCode::Escape).pressed() {
            if capturing {
                self.input_buffer.clear();
                return true;
            }
            events.push(InputEvent::Escape);
        }

        if capturing {
            self.input_buffer.clear();
            return false;
        }

        if self.get_key(KeyCode::F10).pressed() {
            events.push(InputEvent::ReloadTheme);
        }

        // roBrowser world map is Alt+0. Either Alt key toggles it.
        let either_alt = self.get_key(KeyCode::AltLeft).down() || self.get_key(KeyCode::AltRight).down();
        if either_alt && self.get_key(KeyCode::Digit0).pressed() {
            events.push(InputEvent::ToggleWorldMap);
        }

        #[cfg(feature = "debug")]
        let control_down = self.get_key(KeyCode::ControlLeft).down();

        #[cfg(feature = "debug")]
        if control_down && self.get_key(KeyCode::KeyM).pressed() {
            events.push(InputEvent::ToggleMapsWindow);
        }

        #[cfg(feature = "debug")]
        if control_down && self.get_key(KeyCode::KeyC).pressed() {
            events.push(InputEvent::ToggleClientStateInspectorWindow);
        }

        #[cfg(feature = "debug")]
        if control_down && self.get_key(KeyCode::KeyR).pressed() {
            events.push(InputEvent::ToggleRenderOptionsWindow);
        }

        #[cfg(feature = "debug")]
        if control_down && self.get_key(KeyCode::KeyP).pressed() {
            events.push(InputEvent::ToggleProfilerWindow);
        }

        #[cfg(feature = "debug")]
        if control_down && self.get_key(KeyCode::KeyO).pressed() {
            events.push(InputEvent::ToggleCommandsWindow);
        }

        #[cfg(feature = "debug")]
        if control_down && self.get_key(KeyCode::KeyN).pressed() {
            events.push(InputEvent::TogglePacketInspectorWindow);
        }

        #[cfg(feature = "debug")]
        if self.get_key(KeyCode::ShiftLeft).pressed() && use_debug_camera {
            events.push(InputEvent::CameraAccelerate);
        }

        #[cfg(feature = "debug")]
        if self.get_key(KeyCode::ShiftLeft).released() && use_debug_camera {
            events.push(InputEvent::CameraDecelerate);
        }

        // TODO: This should be moved.
        #[cfg(feature = "debug")]
        if self.right_mouse_button.down() && !self.right_mouse_button.pressed() && process_mouse && use_debug_camera {
            let offset = -cgmath::Vector2::new(self.mouse_delta.width, self.mouse_delta.height);
            events.push(InputEvent::CameraLookAround { offset });
        }

        #[cfg(feature = "debug")]
        if self.get_key(KeyCode::KeyW).down() && use_debug_camera {
            events.push(InputEvent::CameraMoveForward);
        }

        #[cfg(feature = "debug")]
        if self.get_key(KeyCode::KeyS).down() && use_debug_camera {
            events.push(InputEvent::CameraMoveBackward);
        }

        #[cfg(feature = "debug")]
        if self.get_key(KeyCode::KeyA).down() && use_debug_camera {
            events.push(InputEvent::CameraMoveLeft);
        }

        #[cfg(feature = "debug")]
        if self.get_key(KeyCode::KeyD).down() && use_debug_camera {
            events.push(InputEvent::CameraMoveRight);
        }

        #[cfg(feature = "debug")]
        if self.get_key(KeyCode::Space).down() && use_debug_camera {
            events.push(InputEvent::CameraMoveUp);
        }

        self.input_buffer.clear();
        false
    }
}

#[cfg(test)]
mod clipboard_input_tests {
    use super::*;

    #[test]
    fn paste_is_focus_gated_and_never_reads_clipboard_in_tests() {
        let mut input = InputSystem::new(Arc::new(AtomicU64::new(0)));
        input.paste_if_focused(false, || panic!("must not read without a text field"));
        assert!(input.input_buffer.is_empty());
        input.paste_if_focused(true, || Some("synthetic-한🙂".into()));
        assert_eq!(input.input_buffer.iter().collect::<String>(), "synthetic-한🙂");
    }

    #[test]
    fn pasted_controls_cannot_submit_focus_or_delete() {
        let mut input = InputSystem::new(Arc::new(AtomicU64::new(0)));
        input.paste_if_focused(true, || Some("a\r\n\t\x1b\x08\0b".into()));
        assert_eq!(input.input_buffer, vec!['a', 'b']);
    }

    #[test]
    fn empty_or_unavailable_clipboard_preserves_pending_typing() {
        let mut input = InputSystem::new(Arc::new(AtomicU64::new(0)));
        input.buffer_character('a');
        input.paste_if_focused(true, || None);
        input.paste_if_focused(true, || Some(String::new()));
        assert_eq!(input.input_buffer, vec!['a']);
    }

    #[test]
    fn pasted_input_is_bounded_in_unicode_characters() {
        let mut input = InputSystem::new(Arc::new(AtomicU64::new(0)));
        input.paste_if_focused(true, || Some("🙂".repeat(5000)));
        assert_eq!(input.input_buffer.len(), 4096);
    }
}
