//! Live dressing room on Hercules TEST. Headgear the character owns is
//! resolved through the same accessory table and game-file loader the world
//! renderer uses. A character with no headgear records that and returns.

use std::path::Path;
use std::time::{Duration, Instant};

use korangar_loaders::FileLoader;
use korangar_networking::{InventoryItem, NetworkEvent, NoMetadata};
use ragnarok_packets::Sex;

use super::super::LogicSession;
use crate::e2e::dressing_room::{self as harness, LiveDressingReport};
use crate::loaders::GameFileLoader;
use crate::state::dressing_room::{self, DressingRoom, NO_PREVIEW, WornLook};
use crate::world::parse_accessory_names;

const INSTALL: &str = "/root/Work/korangar-screens-runtime-20261109";

impl LogicSession {
    /// Log in, enter the map, and try on every headgear this character owns.
    pub fn dressing_room_on_test(&mut self) -> Result<LiveDressingReport, String> {
        self.login_to_character_select()?;
        self.enter_world()?;

        let mut inventory: Vec<InventoryItem<NoMetadata>> = Vec::new();
        let mut storage: Vec<InventoryItem<NoMetadata>> = Vec::new();
        let mut cart: Vec<InventoryItem<NoMetadata>> = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let drained = {
                let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) else {
                    return Err("logic mode has no network".to_owned());
                };
                net.get_events(events);
                events.drain().collect::<Vec<_>>()
            };
            for event in drained {
                match event {
                    NetworkEvent::SetInventory { items } => inventory = items,
                    NetworkEvent::SetStorage { items } => storage = items,
                    NetworkEvent::SetCart { items } => cart = items,
                    NetworkEvent::IventoryItemAdded { item } => {
                        if !inventory.iter().any(|row| row.index == item.index) {
                            inventory.push(item);
                        }
                    }
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            std::thread::sleep(Duration::from_millis(10));
        }

        let mut owned = inventory;
        owned.extend(storage);
        owned.extend(cart);
        let character = self.headgear().characters().first().cloned();
        let worn_heads = character.as_ref().map(|character| character.look.headgear()).unwrap_or([0, 0, 0]);
        let pieces = harness::headgear_to_preview(&owned, worn_heads);
        if pieces.is_empty() {
            eprintln!("no headgear on TEST character");
            return Ok(LiveDressingReport {
                note: "no headgear on TEST character".to_owned(),
                checked: 0,
                resolved: Vec::new(),
                missing_view_ids: Vec::new(),
            });
        }

        let loader = GameFileLoader::default();
        loader
            .load_from_install(Path::new(INSTALL))
            .map_err(|error| format!("dressing room archives: {error}"))?;
        let suffixes = accessory_suffixes(&loader);
        let mut worn = WornLook::default();
        if let Some(character) = character.as_ref() {
            worn.job_id = character.job_id.0;
            worn.female = character.sex == Sex::Female;
            worn.hair_style = character.look.hair.max(1);
            worn.head_bottom = character.look.head_bottom;
            worn.head_top = character.look.head_top;
            worn.head_mid = character.look.head_mid;
        }
        worn.garment = dressing_room::worn_look_from_player(None, &owned).garment;
        worn.weapon = dressing_room::worn_look_from_player(None, &owned).weapon;
        worn.shield = dressing_room::worn_look_from_player(None, &owned).shield;

        let mut room = DressingRoom::default();
        let mut resolved = Vec::new();
        let mut missing_view_ids = Vec::new();
        for view in pieces {
            let slot = view.slot.expect("headgear slot");
            room.apply_view(
                &worn,
                view,
                |view_id| suffixes.get(&view_id).cloned(),
                |path| sprite_exists(&loader, path),
            );
            let shown = room
                .slot_views()
                .into_iter()
                .find(|row| row.slot == slot.name())
                .ok_or_else(|| format!("missing slot {}", slot.name()))?;
            if shown.sprite_path.is_empty() {
                if room.notice() != NO_PREVIEW {
                    return Err(format!(
                        "view {} did not resolve and did not say there is no preview",
                        view.view_id
                    ));
                }
                missing_view_ids.push(view.view_id);
            } else {
                resolved.push(format!("{}:{}:{}", slot.name(), view.view_id, shown.sprite_path));
            }
        }

        Ok(LiveDressingReport {
            note: String::new(),
            checked: resolved.len() + missing_view_ids.len(),
            resolved,
            missing_view_ids,
        })
    }
}

fn sprite_exists(loader: &GameFileLoader, path: &str) -> bool {
    loader.file_exists(&format!("data\\sprite\\{path}.spr")) && loader.file_exists(&format!("data\\sprite\\{path}.act"))
}

fn accessory_suffixes(loader: &GameFileLoader) -> hashbrown::HashMap<u16, String> {
    let Ok(accessory_id) = loader.get("data\\luafiles514\\lua files\\datainfo\\accessoryid.lub") else {
        return hashbrown::HashMap::new();
    };
    let Ok(accname) = loader.get("data\\luafiles514\\lua files\\datainfo\\accname.lub") else {
        return hashbrown::HashMap::new();
    };
    parse_accessory_names(&accessory_id, &accname).unwrap_or_default()
}
