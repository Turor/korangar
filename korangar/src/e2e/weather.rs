//! Map weather, zone badge, and gat cell changes in logic mode.
//!
//! Byte fixtures are the Hercules layouts (`0x0091`, `0x01F3`, `0x099B`,
//! `0x01D6`, `0x0192`). Parsing uses the same packet readers as the client.
//! The reducers are [`WeatherSet`], [`MapProperties`], and [`WalkGrid`].

use ragnarok_bytes::ByteReader;
use ragnarok_packets::{
    ChangeMapCellPacket, ChangeMapPacket, DisplaySpecialEffectPacket, MapTypePacket, NotifyMapProperty2Packet, PacketExt, TilePosition,
};

use super::logic::LogicSession;
use crate::state::map_properties::MapProperties;
use crate::world::effects::effect_id_value;
pub use crate::world::weather::WeatherKind;
use crate::world::weather::WeatherSet;
use crate::world::{PathFinder, Traversable, WalkGrid};

/// Active weather, the status badge, and a one-cell corridor for pathing.
#[derive(Debug)]
pub struct MapWorld {
    weather: WeatherSet,
    properties: MapProperties,
    grid: WalkGrid,
}

impl Default for MapWorld {
    fn default() -> Self {
        Self {
            weather: WeatherSet::default(),
            properties: MapProperties::default(),
            grid: WalkGrid::corridor(8),
        }
    }
}

impl MapWorld {
    /// Feeds one server packet. The header selects the same struct the map
    /// server handler reads.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() < 2 {
            return Err("packet shorter than a header".to_owned());
        }
        let header = u16::from_le_bytes([bytes[0], bytes[1]]);
        let mut reader = ByteReader::without_metadata(bytes);
        match header {
            0x0091 => {
                let packet = ChangeMapPacket::packet_from_bytes(&mut reader).map_err(|error| format!("change map: {error:?}"))?;
                self.on_map_change(&packet.map_name);
                Ok(())
            }
            0x01F3 => {
                let packet = DisplaySpecialEffectPacket::packet_from_bytes(&mut reader).map_err(|error| format!("effect: {error:?}"))?;
                let _ = self.note_effect(effect_id_value(packet.effect_id));
                Ok(())
            }
            0x099B => {
                let packet = MapTypePacket::packet_from_bytes(&mut reader).map_err(|error| format!("map property: {error:?}"))?;
                self.apply_property(packet.map_type, packet.flags);
                Ok(())
            }
            0x01D6 => {
                let packet = NotifyMapProperty2Packet::packet_from_bytes(&mut reader).map_err(|error| format!("map type: {error:?}"))?;
                self.apply_map_type(packet.map_type);
                Ok(())
            }
            0x0192 => {
                let packet = ChangeMapCellPacket::packet_from_bytes(&mut reader).map_err(|error| format!("map cell: {error:?}"))?;
                self.apply_cell(packet.position, packet.cell_type, &packet.map_name);
                Ok(())
            }
            _ => Err(format!("unsupported map packet {header:#06x}")),
        }
    }

    pub fn on_map_change(&mut self, map_name: &str) {
        self.weather.clear();
        self.properties.note_map(map_name);
        self.grid = WalkGrid::corridor(8);
    }

    pub fn note_effect(&mut self, effect_id: u32) -> bool {
        self.weather.note_effect(effect_id)
    }

    pub fn apply_property(&mut self, property_type: u16, flags: u32) {
        self.properties.apply_property(property_type, flags);
    }

    pub fn apply_map_type(&mut self, map_type: u16) {
        self.properties.apply_map_type(map_type);
    }

    pub fn apply_cell(&mut self, position: TilePosition, cell_type: u16, map_name: &str) {
        if self.properties.accepts_map(map_name) {
            let _ = self.grid.apply_gat(position, cell_type);
        }
    }

    pub fn set_show(&mut self, show: bool) {
        self.weather.set_show(show);
    }

    pub fn active(&self) -> &[WeatherKind] {
        self.weather.active()
    }

    pub fn visible(&self) -> &[WeatherKind] {
        self.weather.visible()
    }

    pub fn badge(&self) -> Option<&str> {
        let badge = self.properties.badge_text();
        if badge.is_empty() { None } else { Some(badge) }
    }

    pub fn map_name(&self) -> &str {
        self.properties.map_name()
    }

    pub fn cell_walkable(&self, x: u16, y: u16) -> bool {
        self.grid.is_walkable(TilePosition { x, y })
    }

    /// Path from the west end of the corridor to the east end.
    pub fn corridor_path_open(&self) -> bool {
        let mut finder = PathFinder::default();
        let start = TilePosition { x: 0, y: 0 };
        let goal = TilePosition {
            x: self.grid.width().saturating_sub(1),
            y: 0,
        };
        finder.find_walkable_path(&self.grid, start, goal).is_some()
    }
}

impl LogicSession {
    pub fn feed_map_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.map_world.feed(bytes)
    }

    /// Same flag the client copies from Game settings each frame.
    pub fn set_show_weather(&mut self, show: bool) {
        self.map_world.set_show(show);
    }

    pub fn active_weather(&self) -> Vec<WeatherKind> {
        self.map_world.active().to_vec()
    }

    pub fn visible_weather(&self) -> Vec<WeatherKind> {
        self.map_world.visible().to_vec()
    }

    pub fn map_badge(&self) -> Option<String> {
        self.map_world.badge().map(str::to_owned)
    }

    pub fn loaded_map_name(&self) -> String {
        self.map_world.map_name().to_owned()
    }

    pub fn cell_is_walkable(&self, x: u16, y: u16) -> bool {
        self.map_world.cell_walkable(x, y)
    }

    pub fn corridor_path_open(&self) -> bool {
        self.map_world.corridor_path_open()
    }
}
