//! Navigation target (`ZC_NAVIGATION_ACTIVE` / `0x08E2`) and minimap
//! viewpoint marks (`ZC_MARK_POSITION` / `0x0144`).

use korangar_interface::element::StateElement;
use rust_state::RustState;

/// How long a viewpoint mark stays on the current map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, RustState, StateElement)]
pub enum MinimapMarkDuration {
    /// `MarkerType::DisplayFor15Seconds`.
    Timed { expires_at: u32 },
    /// `MarkerType::DisplayUntilLeave`.
    UntilMapChange,
}

/// One server viewpoint pin. `slot` is the client mark id from the packet.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct MinimapMark {
    pub slot: u8,
    pub npc_id: u32,
    pub x: u32,
    pub y: u32,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub duration: MinimapMarkDuration,
    pub summary: String,
}

/// Destination from `navigateto` / `0x08E2`.
#[derive(Clone, Debug, PartialEq, Eq, RustState, StateElement)]
pub struct NavigationTarget {
    pub map_name: String,
    pub x: u16,
    pub y: u16,
    pub monster_id: u16,
    /// 0 coordinates, 1 coordinates that fail when already on the map, 3
    /// monster.
    pub target_type: u8,
    /// rAthena `navigation_service` bits: airship, scroll, kafra.
    pub service_flags: u8,
    pub summary: String,
}

/// One `0x0144` viewpoint update.
pub struct MinimapMarkUpdate {
    pub npc_id: u32,
    pub kind: u8,
    pub x: u32,
    pub y: u32,
    pub slot: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    /// Client tick in milliseconds when the packet arrived.
    pub now: u32,
}

/// Local player cell plus the active navigation target and viewpoint marks.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct Navigation {
    map_name: String,
    player_x: u16,
    player_y: u16,
    /// "Map … — you x,y", kept here so the window can borrow it.
    status_line: String,
    target: Option<NavigationTarget>,
    marks: Vec<MinimapMark>,
}

pub fn normalize_map_name(name: &str) -> String {
    let trimmed = name.trim().trim_matches('\0');
    let without_ext = trimmed
        .strip_suffix(".gat")
        .or_else(|| trimmed.strip_suffix(".GAT"))
        .unwrap_or(trimmed);
    without_ext.to_ascii_lowercase()
}

fn service_label(flags: u8) -> String {
    if flags == 0 {
        return "walk".to_string();
    }
    let mut parts = Vec::new();
    if flags & 1 != 0 {
        parts.push("airship");
    }
    if flags & 2 != 0 {
        parts.push("scroll");
    }
    if flags & 4 != 0 {
        parts.push("kafra");
    }
    if parts.is_empty() {
        format!("services {flags:#x}")
    } else {
        parts.join(", ")
    }
}

fn target_summary(map_name: &str, x: u16, y: u16, monster_id: u16, target_type: u8, service_flags: u8) -> String {
    let place = if monster_id == 0 {
        format!("{map_name} {x},{y}")
    } else {
        format!("{map_name} {x},{y} monster {monster_id}")
    };
    let kind = match target_type {
        0 => "coordinates",
        1 => "coordinates (only from another map)",
        3 => "monster",
        other => return format!("{place} (type {other}, {})", service_label(service_flags)),
    };
    format!("{place} — {kind}, {}", service_label(service_flags))
}

fn mark_summary(slot: u8, x: u32, y: u32, duration: MinimapMarkDuration) -> String {
    match duration {
        MinimapMarkDuration::Timed { expires_at } => format!("#{slot} {x},{y} until tick {expires_at}"),
        MinimapMarkDuration::UntilMapChange => format!("#{slot} {x},{y} until map change"),
    }
}

impl Navigation {
    pub fn map_name(&self) -> &str {
        &self.map_name
    }

    pub fn status_line(&self) -> &str {
        &self.status_line
    }

    pub fn player_cell(&self) -> (u16, u16) {
        (self.player_x, self.player_y)
    }

    fn refresh_status_line(&mut self) {
        if self.map_name.is_empty() {
            self.status_line = format!("Map unknown — you {},{}", self.player_x, self.player_y);
        } else {
            self.status_line = format!("Map {} — you {},{}", self.map_name, self.player_x, self.player_y);
        }
    }

    pub fn target(&self) -> Option<&NavigationTarget> {
        self.target.as_ref()
    }

    pub fn marks(&self) -> &[MinimapMark] {
        &self.marks
    }

    pub fn set_player_cell(&mut self, x: u16, y: u16) {
        self.player_x = x;
        self.player_y = y;
        self.refresh_status_line();
    }

    pub fn set_navigation(&mut self, map_name: String, x: u16, y: u16, monster_id: u16, target_type: u8, service_flags: u8) {
        let summary = target_summary(&map_name, x, y, monster_id, target_type, service_flags);
        self.target = Some(NavigationTarget {
            map_name,
            x,
            y,
            monster_id,
            target_type,
            service_flags,
            summary,
        });
    }

    /// `kind` is the packet `MarkerType`: 0 for 15 seconds, 1 until the map
    /// changes, 2 removes `slot`. `color` is script RGB.
    pub fn apply_mark(&mut self, update: MinimapMarkUpdate) {
        let MinimapMarkUpdate {
            npc_id,
            kind,
            x,
            y,
            slot,
            red,
            green,
            blue,
            now,
        } = update;
        if kind == 2 {
            self.marks.retain(|mark| mark.slot != slot);
            return;
        }
        let duration = if kind == 0 {
            MinimapMarkDuration::Timed {
                expires_at: now.saturating_add(15_000),
            }
        } else {
            MinimapMarkDuration::UntilMapChange
        };
        let summary = mark_summary(slot, x, y, duration);
        let mark = MinimapMark {
            slot,
            npc_id,
            x,
            y,
            red,
            green,
            blue,
            duration,
            summary,
        };
        if let Some(existing) = self.marks.iter_mut().find(|mark| mark.slot == slot) {
            *existing = mark;
        } else {
            self.marks.push(mark);
        }
        self.marks.sort_by_key(|mark| mark.slot);
    }

    pub fn expire(&mut self, now: u32) {
        self.marks.retain(|mark| match mark.duration {
            MinimapMarkDuration::Timed { expires_at } => now < expires_at,
            MinimapMarkDuration::UntilMapChange => true,
        });
    }

    /// Viewpoint marks belong to the map that was just left. The navigation
    /// target stays so the window still names the destination.
    pub fn on_map_change(&mut self, map_name: &str, x: u16, y: u16) {
        self.map_name = normalize_map_name(map_name);
        self.player_x = x;
        self.player_y = y;
        self.marks.clear();
        self.refresh_status_line();
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::{MinimapMarkUpdate, Navigation, normalize_map_name};

    #[allow(clippy::too_many_arguments)]
    fn mark(npc_id: u32, kind: u8, x: u32, y: u32, slot: u8, red: u8, green: u8, blue: u8, now: u32) -> MinimapMarkUpdate {
        MinimapMarkUpdate {
            npc_id,
            kind,
            x,
            y,
            slot,
            red,
            green,
            blue,
            now,
        }
    }

    #[test]
    fn marks_replace_expire_and_clear_on_map_change() {
        let mut navigation = Navigation::default();
        navigation.set_navigation("prontera.gat".to_string(), 150, 180, 1002, 3, 0b111);
        navigation.apply_mark(mark(10, 0, 20, 30, 1, 255, 0, 0, 1_000));
        navigation.apply_mark(mark(11, 1, 40, 50, 1, 0, 255, 0, 1_000));
        assert_eq!(navigation.marks().len(), 1);
        assert_eq!(navigation.marks()[0].red, 0);
        assert!(navigation.marks()[0].summary.contains("until map change"));

        navigation.apply_mark(mark(12, 0, 5, 6, 2, 0, 0, 255, 2_000));
        assert_eq!(navigation.marks().len(), 2);
        navigation.expire(2_000 + 15_000);
        assert_eq!(navigation.marks().len(), 1);
        assert_eq!(navigation.marks()[0].slot, 1);

        navigation.apply_mark(mark(0, 2, 0, 0, 1, 0, 0, 0, 0));
        assert!(navigation.marks().is_empty());

        navigation.apply_mark(mark(1, 1, 1, 1, 0, 1, 2, 3, 0));
        navigation.on_map_change("geffen.gat", 10, 11);
        assert!(navigation.marks().is_empty());
        assert_eq!(navigation.map_name(), "geffen");
        assert_eq!(navigation.player_cell(), (10, 11));
        let target = navigation.target().expect("target survives the warp");
        assert_eq!(normalize_map_name(&target.map_name), "prontera");
        assert!(target.summary.contains("monster 1002"));
        assert!(target.summary.contains("airship"));
        assert!(target.summary.contains("kafra"));
    }
}
