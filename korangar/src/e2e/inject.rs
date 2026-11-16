//! Proves injected input and a winit-shaped frame meet in `InputSystem`.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use ragnarok_packets::ClientTick;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton as WinitButton};
use winit::keyboard::KeyCode;

use crate::input::{InputEvent, InputReport, InputSystem};

pub struct InputPathParity {
    pub winit_escape: bool,
    pub injected_escape: bool,
    pub winit_text: String,
    pub injected_text: String,
    pub winit_click: bool,
    pub injected_click: bool,
    pub winit_drag: bool,
    pub injected_drag: bool,
}

/// One winit frame and one autopilot-shaped frame for keyboard, text, click,
/// and drag.
pub fn input_path_parity() -> InputPathParity {
    let (winit_escape, winit_text, winit_click, winit_drag) = winit_frames();
    let (injected_escape, injected_text, injected_click, injected_drag) = injected_frames();
    InputPathParity {
        winit_escape,
        injected_escape,
        winit_text,
        injected_text,
        winit_click,
        injected_click,
        winit_drag,
        injected_drag,
    }
}

fn fresh() -> InputSystem {
    InputSystem::new(Arc::new(AtomicU64::new(0)))
}

fn keyboard_events(input: &mut InputSystem, events: &mut Vec<InputEvent>) {
    let _ = input.handle_keyboard_input(
        events,
        false,
        #[cfg(feature = "debug")]
        false,
        #[cfg(feature = "debug")]
        false,
    );
}

fn finish(input: &mut InputSystem, tick: u32) -> (InputReport, Vec<InputEvent>) {
    let report = input.update_delta(ClientTick(tick));
    let mut events = Vec::new();
    keyboard_events(input, &mut events);
    (report, events)
}

fn has_escape(events: &[InputEvent]) -> bool {
    events.iter().any(|event| matches!(event, InputEvent::Escape))
}

fn winit_frames() -> (bool, String, bool, bool) {
    let mut input = fresh();
    input.update_keyboard(KeyCode::Escape, ElementState::Pressed);
    for character in ['H', 'i'] {
        input.buffer_character(character);
    }
    let (report, events) = finish(&mut input, 0);
    let escape = has_escape(&events);
    let text: String = report.characters.iter().collect();

    let mut input = fresh();
    input.update_mouse_position(PhysicalPosition { x: 12.0, y: 18.0 });
    input.update_mouse_buttons(WinitButton::Left, ElementState::Pressed);
    let (pressed, _) = finish(&mut input, 16);
    let click = pressed.mouse_click == Some(korangar_interface::layout::MouseButton::Left);
    input.update_mouse_position(PhysicalPosition { x: 40.0, y: 28.0 });
    let (moved, _) = finish(&mut input, 32);
    let drag = moved.drag.is_some_and(|delta| delta.width.abs() + delta.height.abs() > 0.0);
    (escape, text, click, drag)
}

fn injected_frames() -> (bool, String, bool, bool) {
    let mut input = fresh();
    input.expire_synthesized();
    input.advance_synthetic_drag();
    input.synthesize_key(KeyCode::Escape, false, false, false);
    for character in ['H', 'i'] {
        input.buffer_character(character);
    }
    let (report, events) = finish(&mut input, 0);
    let escape = has_escape(&events);
    let text: String = report.characters.iter().collect();

    let mut input = fresh();
    input.expire_synthesized();
    input.advance_synthetic_drag();
    input.synthesize_click(12.0, 18.0, korangar_interface::layout::MouseButton::Left);
    let (pressed, _) = finish(&mut input, 16);
    let click = pressed.mouse_click == Some(korangar_interface::layout::MouseButton::Left);
    input.expire_synthesized();
    input.advance_synthetic_drag();
    let mut input_drag = fresh();
    input_drag.expire_synthesized();
    input_drag.advance_synthetic_drag();
    input_drag.begin_synthetic_drag(12.0, 18.0, 40.0, 28.0);
    let _ = finish(&mut input_drag, 16);
    input_drag.expire_synthesized();
    input_drag.advance_synthetic_drag();
    let (moved, _) = finish(&mut input_drag, 32);
    let drag = moved.drag.is_some_and(|delta| delta.width.abs() + delta.height.abs() > 0.0);
    (escape, text, click, drag)
}

#[cfg(test)]
mod tests {
    use super::input_path_parity;

    #[test]
    fn injected_input_matches_the_winit_path() {
        let parity = input_path_parity();
        assert!(parity.winit_escape && parity.injected_escape);
        assert_eq!(parity.winit_text, "Hi");
        assert_eq!(parity.injected_text, parity.winit_text);
        assert!(parity.winit_click && parity.injected_click);
        assert!(parity.winit_drag && parity.injected_drag);
    }
}
