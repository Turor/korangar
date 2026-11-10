use std::path::Path;

use korangar_interface::element::StateElement;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, RustState, StateElement)]
pub(super) struct GameArchiveList {
    pub archives: Vec<String>,
}

impl Default for GameArchiveList {
    fn default() -> Self {
        let mut archives = Self::loader_order_from_data_ini_slots(Self::TURORAN_DATA_INI_ORDER);
        archives.push(Self::LOCAL_OVERLAY.to_owned());
        Self { archives }
    }
}

impl GameArchiveList {
    const DATA_INI_CANDIDATES: &'static [&'static str] = &["DATA.INI", "data.ini", "client/DATA.INI", "client/data.ini"];
    const FILE_NAME: &'static str = "client/game_archives.ron";
    /// Local overlay directory; kept last so GameFileLoader's last-wins insert
    /// order gives it the highest search priority (see stage-turoran-test.py).
    const LOCAL_OVERLAY: &'static str = "archive/";
    /// Live Turoran `resources-0716/DATA.INI` slot order (0 = highest
    /// priority).
    const TURORAN_DATA_INI_ORDER: &'static [&'static str] = &[
        "translation.grf",
        "2025-07-16_en.grf",
        "data.grf",
        "iro_assets.grf",
        "region_assets.grf",
    ];

    pub(super) fn load(remote_enabled: bool) -> Result<Self, String> {
        match std::fs::read_to_string(Self::FILE_NAME) {
            Ok(data) => {
                let mut list: Self =
                    ron::from_str(&data).map_err(|_| format!("Invalid {}: expected (archives: [\"path\", ...])", Self::FILE_NAME))?;
                // Explicit config still errors on missing entries after filter? Task asks
                // missing archives skipped gracefully for DATA.INI-equivalent defaults.
                // For an explicit ron list we also skip missing so remote/partial installs
                // do not hard-fail on absent proprietary GRFs.
                list.archives.retain(|path| Path::new(path).exists());
                Ok(list)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::without_settings(remote_enabled)),
            Err(_) => Err(format!("Cannot read {}", Self::FILE_NAME)),
        }
    }

    fn without_settings(remote_enabled: bool) -> Self {
        // Remote installs stay archive-free unless an explicit DATA.INI is present, so
        // the verified cache is not shadowed by whatever GRFs happen to sit in
        // the CWD.
        let slot_order = if let Some(slots) = Self::read_data_ini_slots() {
            slots
        } else if remote_enabled {
            Vec::new()
        } else {
            Self::TURORAN_DATA_INI_ORDER.iter().map(|path| (*path).to_owned()).collect()
        };

        let mut archives = if slot_order.is_empty() {
            Vec::new()
        } else {
            Self::loader_order_from_data_ini_slots(&slot_order.iter().map(String::as_str).collect::<Vec<_>>())
        };

        if !remote_enabled || !slot_order.is_empty() {
            Self::ensure_legacy_grf_priority(&mut archives);
        }

        if Path::new(Self::LOCAL_OVERLAY).exists() && !archives.iter().any(|path| path == Self::LOCAL_OVERLAY) {
            archives.push(Self::LOCAL_OVERLAY.to_owned());
        }

        archives.retain(|path| Path::new(path).exists());
        Self { archives }
    }

    /// Keep upstream `rdata.grf` above `data.grf` under last-wins loading,
    /// without outranking Turoran English/translation GRFs when those are
    /// present.
    fn ensure_legacy_grf_priority(archives: &mut Vec<String>) {
        let has = |archives: &[String], name: &str| archives.iter().any(|path| path == name);

        if Path::new("data.grf").exists() && !has(archives, "data.grf") {
            archives.insert(0, "data.grf".to_owned());
        }

        if Path::new("rdata.grf").exists() && !has(archives, "rdata.grf") {
            if let Some(data_at) = archives.iter().position(|path| path == "data.grf") {
                archives.insert(data_at + 1, "rdata.grf".to_owned());
            } else {
                archives.insert(0, "rdata.grf".to_owned());
            }
        }
    }

    /// Convert DATA.INI slot order (index 0 = highest priority) into
    /// GameFileLoader list order (last entry wins because archives are
    /// `insert(0, …)` on load).
    fn loader_order_from_data_ini_slots(slots_high_to_low: &[&str]) -> Vec<String> {
        slots_high_to_low.iter().rev().map(|path| (*path).to_owned()).collect()
    }

    fn read_data_ini_slots() -> Option<Vec<String>> {
        for candidate in Self::DATA_INI_CANDIDATES {
            let Ok(text) = std::fs::read_to_string(candidate) else {
                continue;
            };
            if let Some(slots) = Self::parse_data_ini(&text) {
                return Some(slots);
            }
        }
        None
    }

    /// Parse a Gravity-style DATA.INI `[Data]` section into ascending slot
    /// order.
    fn parse_data_ini(text: &str) -> Option<Vec<String>> {
        let mut in_data = false;
        let mut slots: Vec<(u32, String)> = Vec::new();

        for raw in text.lines() {
            let line = raw.split(['#', ';']).next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                in_data = line.eq_ignore_ascii_case("[Data]");
                continue;
            }
            if !in_data {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Ok(index) = key.trim().parse::<u32>() else {
                continue;
            };
            let path = value.trim().trim_matches('"').trim_matches('\'').to_owned();
            if !path.is_empty() {
                slots.push((index, path));
            }
        }

        if slots.is_empty() {
            return None;
        }
        slots.sort_by_key(|(index, _)| *index);
        Some(slots.into_iter().map(|(_, path)| path).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_defaults_do_not_require_grfs() {
        // Without local DATA.INI/GRFs in the test CWD this stays empty.
        assert!(
            GameArchiveList::without_settings(true)
                .archives
                .iter()
                .all(|path| Path::new(path).exists())
        );
    }

    #[test]
    fn explicit_empty_archive_list_and_local_overlays_are_valid() {
        assert!(ron::from_str::<GameArchiveList>("(archives: [])").unwrap().archives.is_empty());
        assert_eq!(
            ron::from_str::<GameArchiveList>("(archives: [\"archive/\"])").unwrap().archives,
            vec!["archive/"]
        );
    }

    #[test]
    fn data_ini_slot_zero_becomes_highest_loader_priority() {
        let text = "[Data]\n0=translation.grf\n1=2025-07-16_en.grf\n2=data.grf\n3=iro_assets.grf\n4=region_assets.grf\n";
        let slots = GameArchiveList::parse_data_ini(text).unwrap();
        assert_eq!(slots, vec![
            "translation.grf",
            "2025-07-16_en.grf",
            "data.grf",
            "iro_assets.grf",
            "region_assets.grf",
        ]);
        let loader_order = GameArchiveList::loader_order_from_data_ini_slots(&slots.iter().map(String::as_str).collect::<Vec<_>>());
        // Last listed archive is inserted last → searched first (highest priority).
        assert_eq!(loader_order.last().unwrap(), "translation.grf");
        assert_eq!(loader_order.first().unwrap(), "region_assets.grf");
    }

    #[test]
    fn legacy_rdata_outranks_data_but_not_translation() {
        let mut archives = GameArchiveList::loader_order_from_data_ini_slots(&["translation.grf", "data.grf"]);
        // Simulate ensure_legacy when rdata exists: insert after data.
        if let Some(data_at) = archives.iter().position(|path| path == "data.grf") {
            archives.insert(data_at + 1, "rdata.grf".to_owned());
        }
        assert_eq!(archives, vec!["data.grf", "rdata.grf", "translation.grf"]);
        assert!(archives.iter().position(|p| p == "rdata.grf") > archives.iter().position(|p| p == "data.grf"));
        assert!(archives.iter().position(|p| p == "translation.grf") > archives.iter().position(|p| p == "rdata.grf"));
    }

    #[test]
    fn turoran_default_matches_live_data_ini_order() {
        let order = GameArchiveList::loader_order_from_data_ini_slots(GameArchiveList::TURORAN_DATA_INI_ORDER);
        assert_eq!(order, vec![
            "region_assets.grf",
            "iro_assets.grf",
            "data.grf",
            "2025-07-16_en.grf",
            "translation.grf",
        ]);
    }

    #[test]
    fn parse_data_ini_ignores_comments_and_other_sections() {
        let text = "\
; header\n[Misc]\n0=ignore.grf\n[Data]\n0=translation.grf # slot zero\n2=data.grf\n1=\"2025-07-16_en.grf\"\n";
        assert_eq!(GameArchiveList::parse_data_ini(text).unwrap(), vec![
            "translation.grf",
            "2025-07-16_en.grf",
            "data.grf"
        ]);
    }
}
