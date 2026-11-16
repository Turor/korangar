//! Pet, homunculus, mercenary, and elemental panels.
//!
//! The Pets tab of the Pet Stable group stays [`CompanionWindow`]. Egg hatching
//! opens [`PetEggWindow`].

mod actions;
mod bars;
mod eggs;
mod skills;
mod state;

use korangar_interface::Interface;
use korangar_networking::{CompanionNotice, CompanionSkillSnapshot, egg_inventory_index};
use ragnarok_packets::{ItemId, SkillId};
use rust_state::State;

pub use self::eggs::PetEggWindow;
pub use self::state::{
    CompanionPanelView, CompanionSkill, CompanionWindowState, EggChoice, HomunculusPanel, MercenaryPanel, PetPanel, SkillView,
};
use self::state::{CompanionWindowStatePathExt, format_skill_label};
use super::WindowClass;
use crate::loaders::{AsyncLoader, ImageType};
use crate::state::inventory::InventoryPathExt;
use crate::state::item_skill_selection::skill_icon_sprite_path;
use crate::state::{ClientState, ClientStatePathExt, client_state};
use crate::world::{ItemResource, ItemResourceKey, Library, SkillListInformation, Table, inventory_icon_texture_path};

type GameInterface = Interface<'static, ClientState>;

pub struct CompanionWindow;

impl CompanionWindow {
    pub fn new() -> Self {
        Self
    }
}

impl korangar_interface::window::CustomWindow<ClientState> for CompanionWindow {
    fn window_class() -> Option<WindowClass> {
        Some(WindowClass::Companions)
    }

    fn to_window<'a>(self) -> impl korangar_interface::window::Window<ClientState> + 'a {
        use korangar_interface::prelude::*;

        use super::WindowGroupTabs;
        use crate::loaders::OverflowBehavior;
        use crate::state::theme::InterfaceThemeType;

        let state_path = client_state().companion_window();
        window! {
            title: "Companions",
            class: Self::window_class(),
            theme: InterfaceThemeType::InGame,
            closable: true,
            resizable: true,
            minimum_width: 320.0,
            minimum_height: 240.0,
            elements: (
                WindowGroupTabs::new(WindowClass::Companions),
                text! {
                    text: state_path.status(),
                    overflow_behavior: OverflowBehavior::Shrink,
                },
                actions::CompanionActions::new(state_path),
            ),
        }
    }
}

pub fn apply_companion_notice(
    state: &mut State<ClientState>,
    interface: &mut GameInterface,
    loader: &AsyncLoader,
    library: &Library,
    notice: CompanionNotice,
) {
    match notice {
        CompanionNotice::StartCapture => {
            state.follow_mut(client_state().companion_window()).arm_capture();
            interface.open_window(CompanionWindow::new());
        }
        CompanionNotice::CaptureResult { success } => {
            state.follow_mut(client_state().companion_window()).apply_capture_result(success);
            interface.open_window(CompanionWindow::new());
        }
        CompanionNotice::PetEggs { indices } => {
            let eggs = egg_rows(state, loader, library, &indices);
            state.follow_mut(client_state().companion_window()).show_eggs(eggs);
            interface.open_window(PetEggWindow::new());
        }
        CompanionNotice::HomunculusState { kind, entity_id, value } => {
            state
                .follow_mut(client_state().companion_window())
                .apply_homunculus_state(kind, entity_id, value);
            interface.open_window(CompanionWindow::new());
        }
        CompanionNotice::HomunculusSkills { skills } => {
            let rows = skill_rows(loader, library, &skills, true);
            state.follow_mut(client_state().companion_window()).set_homunculus_skills(rows);
            interface.open_window(CompanionWindow::new());
        }
        CompanionNotice::HomunculusSkillUpdate {
            skill_id,
            level,
            sp_cost,
            attack_range,
            upgradable,
        } => {
            state.follow_mut(client_state().companion_window()).apply_homunculus_skill_update(
                skill_id,
                level,
                sp_cost,
                attack_range,
                upgradable,
            );
            interface.open_window(CompanionWindow::new());
        }
        CompanionNotice::MercenarySkills { skills } => {
            let rows = skill_rows(loader, library, &skills, false);
            state.follow_mut(client_state().companion_window()).set_mercenary_skills(rows);
            interface.open_window(CompanionWindow::new());
        }
        CompanionNotice::MercenarySkillUpdate {
            skill_id,
            level,
            sp_cost,
            attack_range,
            upgradable,
        } => {
            state.follow_mut(client_state().companion_window()).apply_mercenary_skill_update(
                skill_id,
                level,
                sp_cost,
                attack_range,
                upgradable,
            );
            interface.open_window(CompanionWindow::new());
        }
    }
}

pub fn refresh_accessory_icon(state: &mut State<ClientState>, loader: &AsyncLoader, library: &Library) {
    let item_id = state.follow(client_state().companion_window()).panel_view().pet_accessory;
    let texture = if item_id <= 0 {
        None
    } else {
        item_icon(library, loader, ItemId(item_id as u32))
    };
    state.follow_mut(client_state().companion_window()).set_accessory_texture(texture);
}

pub fn refresh_contract(state: &mut State<ClientState>, now: i64) {
    state.follow_mut(client_state().companion_window()).refresh_contract(now);
}

/// Title-bar X and Escape remove the egg window without an event. Choosing an
/// egg already cleared the list, so this only cancels a still-open picker.
pub fn on_pet_egg_dismissed(was_open: bool, state: &mut State<ClientState>, interface: &GameInterface) {
    if !was_open || interface.is_window_with_class_open(WindowClass::PetEggs) {
        return;
    }
    state.follow_mut(client_state().companion_window()).cancel_eggs();
}

fn egg_rows(state: &State<ClientState>, loader: &AsyncLoader, library: &Library, indices: &[u16]) -> Vec<EggChoice> {
    let items: Vec<(u16, u32, String, Option<std::sync::Arc<crate::graphics::Texture>>)> = state
        .follow(client_state().inventory().items())
        .iter()
        .map(|item| {
            (
                item.index.0,
                item.item_id.0,
                item.metadata.name.clone(),
                item.metadata.texture.clone(),
            )
        })
        .collect();
    indices
        .iter()
        .map(|wire| {
            let inventory_index = egg_inventory_index(*wire);
            let found = inventory_index.and_then(|index| items.iter().find(|item| item.0 == index));
            match found {
                Some((_, item_id, name, texture)) => {
                    let texture = texture.clone().or_else(|| item_icon(library, loader, ItemId(*item_id)));
                    let label = if name.is_empty() { format!("Egg {wire}") } else { name.clone() };
                    EggChoice {
                        wire_index: *wire,
                        label,
                        item_id: *item_id,
                        texture,
                    }
                }
                None => EggChoice {
                    wire_index: *wire,
                    label: format!("Egg {wire}"),
                    item_id: 0,
                    texture: None,
                },
            }
        })
        .collect()
}

fn item_icon(library: &Library, loader: &AsyncLoader, item_id: ItemId) -> Option<std::sync::Arc<crate::graphics::Texture>> {
    let resource = ItemResource::try_get(library, ItemResourceKey {
        item_id,
        is_identified: true,
    })?;
    let path = inventory_icon_texture_path(resource.as_str()).ok()?;
    loader.request_item_sprite_load(item_id, resource.as_str(), &path, ImageType::Color)
}

fn skill_rows(loader: &AsyncLoader, library: &Library, skills: &[CompanionSkillSnapshot], offer_level_up: bool) -> Vec<CompanionSkill> {
    skills
        .iter()
        .map(|skill| {
            let info = library.get::<SkillListInformation>(SkillId(skill.skill_id));
            let name = if info.name.is_empty() || info.name == "notfound" {
                if skill.name.is_empty() {
                    format!("Skill {}", skill.skill_id)
                } else {
                    skill.name.clone()
                }
            } else {
                info.name.clone()
            };
            let texture = skill_icon_sprite_path(&info.file_name).and_then(|path| {
                loader
                    .request_skill_sprite_load(SkillId(skill.skill_id), &path)
                    .and_then(|sprite| sprite.textures.first().cloned())
            });
            let upgradable = skill.upgradable;
            CompanionSkill {
                skill_id: skill.skill_id,
                level: skill.level,
                sp_cost: skill.sp_cost,
                attack_range: skill.attack_range,
                label: format_skill_label(&name, skill.level, skill.sp_cost, upgradable, offer_level_up),
                name,
                upgradable,
                texture,
            }
        })
        .collect()
}
