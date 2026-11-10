//! Map aliases from `data\resnametable.txt`.
//!
//! The official client resolves every file through that table. Many maps the
//! server knows (`06guild_01`, `alberta_x`, instance copies such as `1@ch1b`)
//! ship no files of their own: the table points their `.rsw`, `.gnd` and
//! `.gat` at another map. Without the lookup those maps load as a black
//! screen. Only map files are aliased here so textures and sprites keep their
//! current lookup.

use std::collections::HashMap;

/// Archive path of the table, normalized like every other archive path.
pub const RESNAMETABLE_PATH: &str = "data\\resnametable.txt";

const MAP_EXTENSIONS: [&str; 3] = [".rsw", ".gnd", ".gat"];

fn is_map_file(name: &str) -> bool {
    MAP_EXTENSIONS.iter().any(|extension| name.ends_with(extension))
}

/// `name.rsw` → `target.rsw` for every map-file line of the table. Keys and
/// values are lowercase file names relative to `data\`.
pub fn parse_map_aliases(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                return None;
            }
            let mut fields = line.split('#');
            let from = fields.next()?.trim().replace('/', "\\").to_lowercase();
            let to = fields.next()?.trim().replace('/', "\\").to_lowercase();
            (is_map_file(&from) && is_map_file(&to) && !to.is_empty() && from != to).then_some((from, to))
        })
        .collect()
}

/// The archive path to try first for `archive_path`, if the table aliases it.
/// `archive_path` must already be normalized (`data\name.rsw`).
pub fn aliased_path(aliases: &HashMap<String, String>, archive_path: &str) -> Option<String> {
    let name = archive_path.strip_prefix("data\\")?;
    if !is_map_file(name) {
        return None;
    }
    aliases.get(name).map(|target| format!("data\\{target}"))
}

/// Decode the table. It is EUC-KR on official clients and plain ASCII for
/// the map lines we care about.
pub fn decode_table(bytes: &[u8]) -> String {
    let (text, ..) = encoding_rs::EUC_KR.decode(bytes);
    text.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = concat!(
        "// comment\r\n",
        "06guild_01.gnd#guild_vs2.gnd#\r\n",
        "06guild_01.gat#guild_vs2.gat#\r\n",
        "06guild_01.rsw#guild_vs2.rsw#\r\n",
        "1@ch1a.rsw#1@ch1a.rsw#\r\n",
        "httyd_01.rsw#hu_fild05.rsw#\r\n",
        "some\\texture.bmp#other\\texture.bmp#\r\n",
        "broken line without hash\r\n",
    );

    #[test]
    fn only_map_lines_become_aliases() {
        let aliases = parse_map_aliases(TABLE);
        assert_eq!(aliases.get("06guild_01.rsw").map(String::as_str), Some("guild_vs2.rsw"));
        assert_eq!(aliases.get("06guild_01.gat").map(String::as_str), Some("guild_vs2.gat"));
        assert_eq!(aliases.get("httyd_01.rsw").map(String::as_str), Some("hu_fild05.rsw"));
        // Self-aliases and textures are not map aliases.
        assert!(!aliases.contains_key("1@ch1a.rsw"));
        assert!(!aliases.contains_key("some\\texture.bmp"));
        assert_eq!(aliases.len(), 4);
    }

    #[test]
    fn map_paths_resolve_through_the_table() {
        let aliases = parse_map_aliases(TABLE);
        assert_eq!(
            aliased_path(&aliases, "data\\06guild_01.gnd").as_deref(),
            Some("data\\guild_vs2.gnd")
        );
        assert_eq!(aliased_path(&aliases, "data\\prt_in.rsw"), None);
        assert_eq!(aliased_path(&aliases, "data\\texture\\06guild_01.rsw"), None);
        assert_eq!(aliased_path(&aliases, "data\\06guild_01.bmp"), None);
    }

    #[test]
    fn table_bytes_decode_as_euc_kr() {
        let (bytes, ..) = encoding_rs::EUC_KR.encode("유저.rsw#prontera.rsw#\n");
        let aliases = parse_map_aliases(&decode_table(&bytes));
        assert_eq!(aliases.get("유저.rsw").map(String::as_str), Some("prontera.rsw"));
    }
}
