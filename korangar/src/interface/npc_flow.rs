//! NPC dialog actions that touch several client fields at once.
//!
//! These are free functions so the network and input loops can call them while
//! they already borrow an event buffer on `Client`.

use std::sync::Arc;

use korangar_audio::AudioEngine;
use korangar_interface::Interface;
use korangar_networking::{NetworkingSystem, NpcInputReply, apply_npc_bgm, decide_npc_input, sound_effect_path};
use ragnarok_packets::EntityId;
use ragnarok_packets::handler::PacketCallback;
use rust_state::State;

use crate::interface::windows::{DialogWindow, WindowClass};
use crate::loaders::GameFileLoader;
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

/// A local walk aborts the bar with `0x02F1` only. Hercules ignores a client `0x02F2`.
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
pub(crate) fn recover_dismissed_npc_windows<Callback>(
    state: &mut State<ClientState>,
    interface: &mut Interface<'static, ClientState>,
    networking: &mut NetworkingSystem<Callback>,
) where
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
        return;
    }

    if awaiting_choice || (awaiting_next && !awaiting_close) {
        interface.open_window(DialogWindow::new(client_state().dialog_window()));
    } else if awaiting_close {
        finish_npc_dialog(state, interface, networking, npc_id);
    }
}
