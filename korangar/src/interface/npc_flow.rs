//! NPC dialog actions that touch several client fields at once.
//!
//! These are free functions so the network and input loops can call them while
//! they already borrow an event buffer on `Client`.

use std::sync::Arc;

use korangar_audio::AudioEngine;
use korangar_interface::Interface;
use korangar_interface::window::{Anchor, AnchorPoint};
use korangar_networking::{NetworkingSystem, NpcInputReply, apply_npc_bgm, decide_npc_input, sound_effect_path};
use ragnarok_packets::EntityId;
use ragnarok_packets::handler::PacketCallback;
use rust_state::State;

use crate::graphics::{ScreenPosition, ScreenSize};
use crate::input::InputEvent;
use crate::interface::windows::WindowClass;
use crate::loaders::GameFileLoader;
use crate::state::vn_dialog::{self, ClickDecision, EscapeKind, MENU_CANCEL_OPTION, bottom_anchor_offset, dialog_band};
use crate::state::{ClientState, ClientStatePathExt, client_state};
use crate::world::Map;

/// Act 2 asks the client to stop the effect. The audio engine has no
/// stop-by-name handle.
pub(crate) fn play_npc_sound(
    audio: &AudioEngine<GameFileLoader>,
    state: &State<ClientState>,
    file_name: &str,
    action: u8,
    npc_id: EntityId,
) {
    if action == 2 {
        return;
    }
    let Some(path) = sound_effect_path(file_name) else {
        return;
    };
    let position = state
        .follow(client_state().entities())
        .iter()
        .find(|entity| entity.get_entity_id() == npc_id)
        .map(|entity| entity.get_position());
    let key = audio.load(&path);
    if let Some(position) = position {
        audio.play_spatial_sound_effect(key, position, 50.0);
    } else {
        audio.play_sound_effect(key);
    }
}

pub(crate) fn play_npc_bgm(
    audio: &AudioEngine<GameFileLoader>,
    state: &mut State<ClientState>,
    map: Option<&Arc<Map>>,
    track: &str,
    play_type: u8,
) {
    match apply_npc_bgm(play_type, track) {
        Some(path) => {
            state.follow_mut(client_state().npc_presentation()).set_npc_bgm(path.clone());
            audio.play_background_music_track(Some(&path));
        }
        None => {
            state.follow_mut(client_state().npc_presentation()).clear_npc_bgm();
            let map_track = map.and_then(|map| map.background_music_track_name().map(str::to_owned));
            audio.play_background_music_track(map_track.as_deref());
        }
    }
}

/// A local walk aborts the bar with `0x02F1` only. Hercules ignores a client
/// `0x02F2`.
pub(crate) fn abort_npc_progress_for_move<Callback>(state: &mut State<ClientState>, networking: &mut NetworkingSystem<Callback>)
where
    Callback: PacketCallback + Send,
{
    if state.follow_mut(client_state().npc_presentation()).cancel_for_move() {
        let _ = networking.npc_progress_complete();
    }
}

pub(crate) fn reply_npc_input<Callback>(
    state: &mut State<ClientState>,
    interface: &mut Interface<'static, ClientState>,
    networking: &mut NetworkingSystem<Callback>,
    cancel: bool,
) where
    Callback: PacketCallback + Send,
{
    let Some(submission) = state.follow_mut(client_state().dialog_window()).take_input() else {
        return;
    };
    let reply = decide_npc_input(submission.npc_id, submission.number, &submission.text, cancel);
    match reply {
        NpcInputReply::Number { npc_id, value } => {
            let _ = networking.npc_number_input(npc_id, value);
        }
        NpcInputReply::Text { npc_id, text } => {
            let _ = networking.npc_text_input(npc_id, text);
        }
    }
    interface.close_window_with_class(WindowClass::NpcInput);
}

pub(crate) fn dismiss_npc_dialog_ui(state: &mut State<ClientState>, interface: &mut Interface<'static, ClientState>) {
    state.follow_mut(client_state().dialog_window()).end();
    state.follow_mut(client_state().npc_presentation()).clear_cutin();
    interface.close_window_with_class(WindowClass::Dialog);
    interface.close_window_with_class(WindowClass::NpcInput);
}

pub(crate) fn finish_npc_dialog<Callback>(
    state: &mut State<ClientState>,
    interface: &mut Interface<'static, ClientState>,
    networking: &mut NetworkingSystem<Callback>,
    npc_id: EntityId,
) where
    Callback: PacketCallback + Send,
{
    dismiss_npc_dialog_ui(state, interface);
    let _ = networking.close_dialog(npc_id);
}

pub(crate) fn poll_npc_progress<Callback>(state: &mut State<ClientState>, networking: &mut NetworkingSystem<Callback>, now_ms: u32)
where
    Callback: PacketCallback + Send,
{
    if state.follow_mut(client_state().npc_presentation()).poll(now_ms) {
        let _ = networking.npc_progress_complete();
    }
}

/// The window X button removes the window without a callback. Answer the script
/// when that drops an input or a Close dialog, and reopen a Next-only dialog.
///
/// Returns the NPC whose script keeps running after the window closed, so the
/// caller can answer its remaining pages without showing them.
pub(crate) fn recover_dismissed_npc_windows<Callback>(
    state: &mut State<ClientState>,
    interface: &mut Interface<'static, ClientState>,
    networking: &mut NetworkingSystem<Callback>,
) -> Option<EntityId>
where
    Callback: PacketCallback + Send,
{
    let (input_open, awaiting_next, awaiting_close, awaiting_choice, npc_id) = {
        let dialog = state.follow(client_state().dialog_window());
        (
            dialog.input_is_open(),
            dialog.is_awaiting_next(),
            dialog.is_awaiting_close(),
            dialog.is_awaiting_choice(),
            dialog.npc_id(),
        )
    };

    if input_open && !interface.is_window_with_class_open(WindowClass::NpcInput) {
        reply_npc_input(state, interface, networking, true);
    }

    if interface.is_window_with_class_open(WindowClass::Dialog) {
        return None;
    }

    // The title-bar X is the same as Escape: cancel a menu, or end a plain page.
    // Reopening the window would leave the script running with nowhere to click.
    if awaiting_choice {
        let _ = networking.choose_dialog_option(npc_id, MENU_CANCEL_OPTION);
        dismiss_npc_dialog_ui(state, interface);
        None
    } else if awaiting_next || awaiting_close {
        finish_npc_dialog(state, interface, networking, npc_id);
        (!awaiting_close).then_some(npc_id)
    } else {
        None
    }
}

/// Result of Escape on the NPC script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NpcEscape {
    /// No script window is open; Escape goes to the window stack.
    NotOpen,
    /// The packet sent ends the script on Hercules.
    Ended,
    /// The dialog is hidden but the script continues on the server.
    Dismissed(EntityId),
}

/// Pin the dialog to the bottom and advance the typewriter before layout.
pub(crate) fn prepare_vn_dialog(
    state: &mut State<ClientState>,
    interface: &mut Interface<'static, ClientState>,
    screen: ScreenSize,
    now_ms: u32,
) {
    if !interface.is_window_with_class_open(WindowClass::Dialog) {
        return;
    }

    let band = dialog_band(screen.width, screen.height);
    let (offset_left, offset_top) = bottom_anchor_offset(screen.width, band);
    let npc_id = state.follow(client_state().dialog_window()).npc_id();
    let name = state
        .follow(client_state().entities())
        .iter()
        .find(|entity| entity.get_entity_id() == npc_id)
        .and_then(|entity| {
            entity
                .get_details()
                .and_then(|name| vn_dialog::npc_display_name(name))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "NPC".to_owned());
    let rate = state.follow(client_state().game_settings()).dialog_chars_per_second;

    let dialog = state.follow_mut(client_state().dialog_window());
    dialog.set_npc_name(name);
    dialog.set_band(band.width, band.height);
    dialog.tick_typewriter(now_ms, rate);

    interface.place_window_class(
        WindowClass::Dialog,
        Anchor::at(AnchorPoint::BottomCenter, ScreenPosition {
            left: offset_left,
            top: offset_top,
        }),
        ScreenSize {
            width: band.width,
            height: band.height,
        },
    );
}

/// A primary click while a script is open. `Pass` means the world may walk.
pub(crate) enum VnPrimaryClick {
    Pass,
    Consume,
    InterfaceOnly,
    Event(Box<InputEvent>),
}

pub(crate) fn vn_primary_click(state: &mut State<ClientState>, dialog_open: bool) -> VnPrimaryClick {
    let input_open = state.follow(client_state().dialog_window()).input_is_open();
    if !dialog_open && !input_open {
        return VnPrimaryClick::Pass;
    }

    let cutin_visible = state.follow(client_state().npc_presentation()).cutin().is_some();
    let (revealing, awaiting_next, awaiting_close, blocked, npc_id) = {
        let dialog = state.follow(client_state().dialog_window());
        (
            dialog.is_revealing(),
            dialog.is_awaiting_next(),
            dialog.is_awaiting_close(),
            dialog.blocks_advance(cutin_visible),
            dialog.npc_id(),
        )
    };

    match vn_dialog::route_click(revealing, awaiting_next, awaiting_close, blocked) {
        ClickDecision::CompleteReveal => {
            state.follow_mut(client_state().dialog_window()).complete_reveal();
            VnPrimaryClick::Consume
        }
        ClickDecision::Next => VnPrimaryClick::Event(Box::new(InputEvent::NextDialog { npc_id })),
        ClickDecision::Close => VnPrimaryClick::Event(Box::new(InputEvent::CloseDialog { npc_id })),
        ClickDecision::Ignore => VnPrimaryClick::InterfaceOnly,
    }
}

pub(crate) fn npc_script_captures_pointer(state: &State<ClientState>, dialog_open: bool) -> bool {
    dialog_open || state.follow(client_state().dialog_window()).input_is_open()
}

/// Escape ends the current script state with the packet Hercules expects.
/// Returns [`NpcEscape::NotOpen`] when no script is open so the generic
/// window stack can run.
pub(crate) fn try_close_npc_script<Callback>(
    state: &mut State<ClientState>,
    interface: &mut Interface<'static, ClientState>,
    networking: &mut NetworkingSystem<Callback>,
) -> NpcEscape
where
    Callback: PacketCallback + Send,
{
    let window_open = interface.is_window_with_class_open(WindowClass::Dialog);
    let (menu, input, plain, awaiting_close, npc_id) = {
        let dialog = state.follow(client_state().dialog_window());
        let plain = window_open && (dialog.is_awaiting_next() || dialog.is_awaiting_close() || dialog.has_page() || dialog.is_revealing());
        (
            dialog.is_awaiting_choice(),
            dialog.input_is_open(),
            plain,
            dialog.is_awaiting_close(),
            dialog.npc_id(),
        )
    };

    match vn_dialog::escape_action(menu, input, plain) {
        Some(EscapeKind::MenuCancel) => {
            let _ = networking.choose_dialog_option(npc_id, MENU_CANCEL_OPTION);
            dismiss_npc_dialog_ui(state, interface);
            NpcEscape::Ended
        }
        Some(EscapeKind::InputCancel) => {
            // The empty reply resumes the script; hide the rest of it too.
            reply_npc_input(state, interface, networking, true);
            dismiss_npc_dialog_ui(state, interface);
            NpcEscape::Dismissed(npc_id)
        }
        Some(EscapeKind::CloseDialog) => {
            finish_npc_dialog(state, interface, networking, npc_id);
            match awaiting_close {
                true => NpcEscape::Ended,
                false => NpcEscape::Dismissed(npc_id),
            }
        }
        None => NpcEscape::NotOpen,
    }
}
