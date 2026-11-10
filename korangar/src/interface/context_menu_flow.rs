//! Open the player context menu and apply sit, view-equip, and show-equip.

use korangar_networking::ViewedEquipPiece;
use ragnarok_packets::{AccountId, ClientTick, EntityId};

use crate::Client;
use crate::interface::windows::{ChatWindow, ContextMenuWindow, InventoryWindow, ViewEquipWindow, WindowClass};
use crate::state::context_menu::ContextMenuFacts;
use crate::state::inventory::InventoryPathExt;
use crate::state::view_equip::ViewedEquipItem;
use crate::state::{ClientStatePathExt, client_state, this_entity};
use crate::world::EntityType;

impl Client {
    pub(crate) fn open_player_context_menu(&mut self, entity_id: EntityId) {
        let self_id = self.client_state.try_follow(this_entity()).map(|entity| entity.get_entity_id());
        let self_target = self_id == Some(entity_id);
        let (details, unavailable) = {
            let entities = self.client_state.follow(client_state().entities());
            let Some(entity) = entities.iter().find(|entity| entity.get_entity_id() == entity_id) else {
                return;
            };
            if entity.get_entity_type() != EntityType::Player {
                return;
            }
            (
                entity.get_details().cloned().unwrap_or_default(),
                entity.are_details_unavailable(),
            )
        };
        if !self_target
            && unavailable
            && self.networking_system.entity_details(entity_id).is_ok()
            && let Some(entity) = self
                .client_state
                .follow_mut(client_state().entities())
                .iter_mut()
                .find(|entity| entity.get_entity_id() == entity_id)
        {
            entity.set_details_requested();
        }
        let name = if self_target {
            self.client_state.follow(client_state().player_name()).clone()
        } else {
            details
        };
        let friend_names: Vec<String> = self
            .client_state
            .follow(client_state().friend_list())
            .iter()
            .map(|friend| friend.name.clone())
            .collect();
        let (in_party, party_leader) = {
            let party = self.client_state.follow(client_state().party());
            (party.joined, party.self_is_leader)
        };
        let sitting = self.client_state.follow(client_state().context_menu()).self_sitting;
        let facts = ContextMenuFacts {
            self_target,
            sitting,
            target_name: &name,
            friend_names: &friend_names,
            in_party,
            party_leader,
        };
        self.client_state
            .follow_mut(client_state().context_menu())
            .open(entity_id.0, &facts);
        if !self.interface.is_window_with_class_open(WindowClass::ContextMenu) {
            self.interface.open_window(ContextMenuWindow::new());
        }
    }

    pub(crate) fn pick_context_menu(&mut self, index: usize) {
        let (action, character_name, self_target, entity_id) = {
            let menu = self.client_state.follow(client_state().context_menu());
            let action = menu.entries.get(index).map(|entry| entry.action);
            (
                action,
                menu.character_name.clone(),
                menu.self_target,
                EntityId(menu.target_entity_id),
            )
        };
        self.interface.close_window_with_class(WindowClass::ContextMenu);
        let Some(action) = action else {
            return;
        };
        use crate::state::context_menu::ContextMenuAction;
        match action {
            ContextMenuAction::ViewEquipment => {
                let _ = self.networking_system.request_view_player_equip(AccountId(entity_id.0));
            }
            ContextMenuAction::Trade => {
                let _ = self.networking_system.request_trade(AccountId(entity_id.0));
            }
            ContextMenuAction::Whisper => {
                if character_name.is_empty() {
                    return;
                }
                self.client_state
                    .follow_mut(client_state().chat_window())
                    .set_whisper_target(character_name);
                if !self.interface.is_window_with_class_open(WindowClass::Chat) {
                    self.interface
                        .open_window(ChatWindow::new(client_state().chat_window(), client_state().chat_messages()));
                }
            }
            ContextMenuAction::SendMail => {
                if !character_name.is_empty() {
                    self.input_event_buffer
                        .push(crate::input::InputEvent::ComposeMailTo { receiver: character_name });
                }
            }
            ContextMenuAction::AddFriend => {
                if !character_name.is_empty() {
                    self.input_event_buffer.push(crate::input::InputEvent::AddFriend { character_name });
                }
            }
            ContextMenuAction::PartyInvite => {
                if !character_name.is_empty() {
                    self.input_event_buffer
                        .push(crate::input::InputEvent::InviteToParty { character_name });
                }
            }
            ContextMenuAction::Sit | ContextMenuAction::Stand => {
                if !self_target {
                    return;
                }
                let Some(self_id) = self.client_state.try_follow(this_entity()).map(|entity| entity.get_entity_id()) else {
                    return;
                };
                let sit = action == ContextMenuAction::Sit;
                if self.networking_system.player_posture(self_id, sit).is_ok() {
                    let client_tick = *self.client_state.follow(client_state().client_tick());
                    self.apply_player_posture(self_id, sit, client_tick);
                }
            }
        }
    }

    pub(crate) fn apply_player_posture(&mut self, entity_id: EntityId, sit: bool, client_tick: ClientTick) {
        if let Some(entity) = self
            .client_state
            .follow_mut(client_state().entities())
            .iter_mut()
            .find(|entity| entity.get_entity_id() == entity_id)
        {
            if sit {
                entity.set_sit(client_tick);
            } else {
                entity.set_idle(client_tick);
            }
        }
        let self_id = self.client_state.try_follow(this_entity()).map(|entity| entity.get_entity_id());
        if self_id == Some(entity_id) {
            self.client_state.follow_mut(client_state().context_menu()).self_sitting = sit;
        }
    }

    pub(crate) fn apply_show_equipment(&mut self, enabled: bool) {
        self.client_state.follow_mut(client_state().game_settings()).show_equipment = enabled;
    }

    pub(crate) fn show_player_equipment(&mut self, name: String, job: i16, sex: u8, items: Vec<ViewedEquipPiece>) {
        let rows = items
            .into_iter()
            .map(|piece| {
                let (label, texture) = if piece.identified {
                    let item_id = i32::try_from(piece.item_id).unwrap_or(0);
                    let (item_name, texture) = self.async_loader.identified_item_face(item_id);
                    let label = if piece.refine > 0 {
                        format!("+{} {item_name}", piece.refine)
                    } else {
                        item_name
                    };
                    (label, texture)
                } else {
                    ("Unidentified".to_owned(), None)
                };
                ViewedEquipItem {
                    item_id: piece.item_id,
                    location: piece.location,
                    refine: piece.refine,
                    label,
                    texture,
                }
            })
            .collect();
        self.client_state
            .follow_mut(client_state().viewed_equipment())
            .replace(name, job, sex, rows);
        if !self.interface.is_window_with_class_open(WindowClass::ViewEquip) {
            self.interface.open_window(ViewEquipWindow::new());
        }
    }

    pub(crate) fn open_inventory_for_trade(&mut self) {
        if !self.interface.is_window_with_class_open(WindowClass::Inventory) {
            self.interface.open_window(InventoryWindow::new(client_state().inventory().items()));
        }
    }
}
