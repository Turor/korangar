//! The calls behind the client match arms for tabs, dialog choices, walks, and
//! chat.
//!
//! Logic mode queues the same `InputEvent` and then uses these functions, so a
//! scenario does not grow a second sender.

use korangar_networking::{NetworkingSystem, NotConnectedError};
use ragnarok_packets::handler::PacketCallback;
use ragnarok_packets::{Direction, EntityId, TilePosition, WorldPosition};

use crate::input::InputEvent;
use crate::interface::windows::{WindowClass, WindowGroup, group_member_close_event, group_member_open_event};

/// What [`InputEvent::SwitchWindowTab`] does once the two classes share a
/// group.
pub(crate) struct TabSwitch {
    pub(crate) remember: WindowClass,
    pub(crate) close_from: bool,
    pub(crate) raise_to: bool,
    pub(crate) open_event: Option<InputEvent>,
    pub(crate) close_event: Option<InputEvent>,
    /// False when the target has no open event. The client still remembers
    /// `to`.
    pub(crate) switched: bool,
}

/// Same decision as the client `SwitchWindowTab` arm.
pub(crate) fn plan_tab_switch(from: WindowClass, to: WindowClass, to_is_open: bool) -> Option<TabSwitch> {
    let same_group = WindowGroup::of(from).is_some_and(|group| group.contains(to));
    if !same_group || from == to {
        return None;
    }
    if to_is_open {
        return Some(TabSwitch {
            remember: to,
            close_from: true,
            raise_to: true,
            open_event: None,
            close_event: group_member_close_event(from),
            switched: true,
        });
    }
    if let Some(open_event) = group_member_open_event(to) {
        return Some(TabSwitch {
            remember: to,
            close_from: true,
            raise_to: false,
            open_event: Some(open_event),
            close_event: group_member_close_event(from),
            switched: true,
        });
    }
    Some(TabSwitch {
        remember: to,
        close_from: false,
        raise_to: false,
        open_event: None,
        close_event: None,
        switched: false,
    })
}

pub(crate) fn apply_switch_window_tab(client: &mut crate::Client, from: WindowClass, to: WindowClass) {
    let to_is_open = client.interface.is_window_with_class_open(to);
    let Some(plan) = plan_tab_switch(from, to, to_is_open) else {
        return;
    };
    client.window_groups.remember(plan.remember);
    if plan.raise_to {
        client.interface.close_window_with_class(from);
        client.interface.raise_window_classes(&[to]);
    } else if let Some(event) = plan.open_event {
        client.interface.share_window_anchor(from, to);
        client.interface.close_window_with_class(from);
        client.input_event_buffer.push(event);
    }
    if plan.switched
        && let Some(event) = plan.close_event
    {
        client.input_event_buffer.push(event);
    }
}

pub(crate) fn send_dialog_choice<C>(networking: &mut NetworkingSystem<C>, npc_id: EntityId, option: i8) -> Result<(), NotConnectedError>
where
    C: PacketCallback + Send,
{
    networking.choose_dialog_option(npc_id, option)
}

pub(crate) fn apply_choose_dialog_option(client: &mut crate::Client, npc_id: EntityId, option: i8) {
    let _ = send_dialog_choice(&mut client.networking_system, npc_id, option);
    client.interface.unfocus();
    if option == -1 {
        crate::interface::npc_flow::dismiss_npc_dialog_ui(&mut client.client_state, &mut client.interface);
    }
}

pub(crate) fn begin_npc_dialog<C>(networking: &mut NetworkingSystem<C>, entity_id: EntityId) -> Result<(), NotConnectedError>
where
    C: PacketCallback + Send,
{
    networking.start_dialog(entity_id)
}

pub(crate) fn send_walk<C>(networking: &mut NetworkingSystem<C>, destination: TilePosition) -> Result<(), NotConnectedError>
where
    C: PacketCallback + Send,
{
    networking.player_move(WorldPosition {
        x: destination.x,
        y: destination.y,
        direction: Direction::North,
    })
}

pub(crate) fn send_public_chat<C>(networking: &mut NetworkingSystem<C>, player_name: &str, text: &str) -> Result<(), NotConnectedError>
where
    C: PacketCallback + Send,
{
    networking.send_chat_message(player_name, text)
}
