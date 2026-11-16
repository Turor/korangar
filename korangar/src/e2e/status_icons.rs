//! HUD status icons in logic mode.
//!
//! [`StatusStrip`] runs `StatusChange` / `StatusChangeSequence` through the
//! same [`StatusEffects`] calls as the client match arms (local entity only,
//! `Ended` removes, everything else upserts and resolves the icon once), and
//! reports what the status panel strip would draw: icon file, tooltip, time
//! label and countdown sweep. Logic mode has no renderer, so the icon is the
//! `stateiconimginfo.lub` file name rather than a texture.

use hashbrown::HashMap;
use korangar_networking::StatusChangeState;
use ragnarok_packets::ClientTick;

use crate::state::status_effects::{
    CountdownSweep, StatusEffects, countdown_sweep, format_remaining, remaining_ms, status_display_name, status_tooltip,
};
use crate::world::{StatusIconInfo, parse_status_icons, with_builtin_icons};

/// One icon as the status panel strip shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct StatusIconView {
    pub status_index: u16,
    pub name: String,
    /// File under `data\texture\effect\`, `None` for a letter chip.
    pub icon_file: Option<String>,
    pub tooltip: String,
    /// `12s`, `3m`, or empty without a time limit.
    pub time: String,
    pub sweep: Option<CountdownSweep>,
}

#[derive(Default)]
pub struct StatusStrip {
    effects: StatusEffects,
    table: HashMap<u16, StatusIconInfo>,
    icon_files: HashMap<u16, Option<String>>,
    local_entity: Option<u32>,
    local_packets: usize,
}

impl StatusStrip {
    /// Strip backed by `efstids.lub`, `stateiconimginfo.lub` and
    /// `stateiconinfo.lub` (the latter two optional) plus the built-in icons,
    /// the same table the client library builds.
    pub fn with_tables(efst_ids: &[u8], image_info: Option<&[u8]>, icon_info: Option<&[u8]>) -> Result<Self, String> {
        let table = parse_status_icons(efst_ids, image_info, icon_info).map_err(|error| format!("stateicon tables: {error}"))?;
        let table = with_builtin_icons(table);
        Ok(Self { table, ..Self::default() })
    }

    pub fn table(&self) -> &HashMap<u16, StatusIconInfo> {
        &self.table
    }

    /// The player's entity id (the account id on Hercules).
    pub fn set_local_entity(&mut self, entity_id: u32) {
        self.local_entity = Some(entity_id);
    }

    /// Status packets for the local player seen so far.
    pub fn local_packets(&self) -> usize {
        self.local_packets
    }

    /// Same filter and calls as `NetworkEvent::StatusChange` in the client.
    pub fn on_status_change(
        &mut self,
        entity_id: u32,
        status_index: u16,
        state: StatusChangeState,
        remaining_milliseconds: u32,
        total_duration_milliseconds: u32,
        tick: u32,
    ) {
        if self.local_entity != Some(entity_id) {
            return;
        }
        self.local_packets += 1;
        match state {
            StatusChangeState::Ended => self.effects.end(status_index),
            StatusChangeState::Active | StatusChangeState::StillActive | StatusChangeState::Other(_) => {
                self.effects.apply(
                    status_index,
                    remaining_milliseconds,
                    total_duration_milliseconds,
                    ClientTick(tick),
                );
                self.resolve(status_index);
            }
        }
    }

    /// The per-frame expiry the client runs before handling network events.
    pub fn expire(&mut self, tick: u32) {
        self.effects.expire(ClientTick(tick));
    }

    /// Icons in strip order at `tick`.
    pub fn view(&self, tick: u32) -> Vec<StatusIconView> {
        self.effects
            .effects()
            .iter()
            .map(|effect| {
                let remaining = remaining_ms(effect, ClientTick(tick));
                StatusIconView {
                    status_index: effect.status_index,
                    name: status_display_name(effect.status_index, self.table.get(&effect.status_index)),
                    icon_file: self.icon_files.get(&effect.status_index).cloned().flatten(),
                    tooltip: effect.tooltip.clone(),
                    time: format_remaining(remaining),
                    sweep: countdown_sweep(remaining, effect.total_duration_milliseconds),
                }
            })
            .collect()
    }

    /// Mirror of `Client::resolve_status_presentation` minus the texture
    /// upload.
    fn resolve(&mut self, status_index: u16) {
        if !self.effects.needs_presentation(status_index) {
            return;
        }
        let info = self.table.get(&status_index);
        let tooltip = status_tooltip(status_index, info);
        let icon_file = info.and_then(|info| info.icon.clone());
        self.icon_files.insert(status_index, icon_file);
        self.effects.set_presentation(status_index, None, tooltip);
    }
}
