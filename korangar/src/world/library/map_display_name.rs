use hashbrown::HashMap;
use korangar_loaders::FileLoader;

use crate::loaders::GameFileLoader;

const MAPNAMETABLE_PATHS: &[&str] = &[
    "data\\mapnametable.txt",
    "data/mapnametable.txt",
    "System\\mapnametable.txt",
    "System/mapnametable.txt",
];

/// Lowercase map id without a `.gat` / `.rsw` suffix, e.g. `iz_int`.
pub fn normalize_map_key(name: &str) -> String {
    let trimmed = name.trim().trim_matches('\0');
    let without_ext = trimmed
        .strip_suffix(".gat")
        .or_else(|| trimmed.strip_suffix(".GAT"))
        .or_else(|| trimmed.strip_suffix(".rsw"))
        .or_else(|| trimmed.strip_suffix(".RSW"))
        .unwrap_or(trimmed);
    without_ext.to_ascii_lowercase()
}

fn decode_table_bytes(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }
    bytes
        .iter()
        .copied()
        .map(|byte| {
            if byte < 0x80 {
                byte as char
            } else {
                // Best-effort EUC-KR lead/trail as replacement; display names are mostly ASCII.
                char::REPLACEMENT_CHARACTER
            }
        })
        .collect()
}

/// Parse Gravity `mapnametable.txt` lines: `prontera.rsw#Prontera#`.
pub fn parse_mapnametable(text: &str) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('#');
        let Some(file_name) = parts.next() else { continue };
        let Some(display) = parts.next() else { continue };
        let key = normalize_map_key(file_name);
        let display = display.trim();
        if key.is_empty() || display.is_empty() {
            continue;
        }
        names.insert(key, display.to_owned());
    }
    names
}

pub fn load_map_display_names(game_file_loader: &GameFileLoader) -> HashMap<String, String> {
    for path in MAPNAMETABLE_PATHS {
        if let Ok(bytes) = game_file_loader.get(path) {
            return parse_mapnametable(&decode_table_bytes(&bytes));
        }
    }
    HashMap::new()
}

pub fn display_name_from_table(table: &HashMap<String, String>, internal: &str) -> String {
    let key = normalize_map_key(internal);
    table.get(&key).cloned().unwrap_or(key)
}

#[cfg(test)]
mod tests {
    use super::{normalize_map_key, parse_mapnametable};

    #[test]
    fn parse_classic_mapnametable_lines() {
        let table = parse_mapnametable("// comment\nprontera.rsw#Prontera City#\niz_int.rsw#Izlude Introductory Area#\n");
        assert_eq!(table.get("prontera").map(String::as_str), Some("Prontera City"));
        assert_eq!(table.get("iz_int").map(String::as_str), Some("Izlude Introductory Area"));
    }

    #[test]
    fn normalize_strips_known_extensions() {
        assert_eq!(normalize_map_key("Prontera.rsw"), "prontera");
        assert_eq!(normalize_map_key("iz_int.gat"), "iz_int");
        assert_eq!(normalize_map_key("geffen"), "geffen");
    }
}
