//! Status icon art and tooltips from `stateicon/*.lub`.
//!
//! Same files and order as roBrowser `loadStateIconInfo`: `efstids.lub`
//! defines `EFST_IDs`, `stateiconimginfo.lub` fills
//! `StateIconImgList[priority][efst] = "file.tga"` (art under
//! `data\texture\effect\`), and `stateiconinfo.lub` fills
//! `StateIconList[efst] = { haveTimeLimit, descript = {{ text, color }, ..} }`.
//! Every file is optional. Older statuses (Blessing, Increase AGI, ...) are
//! not in `stateiconimginfo.lub` because the official client keeps them in
//! its executable; [`with_builtin_icons`] fills those from roBrowser's table.
//! A status with no art anywhere keeps the letter chip.

use hashbrown::HashMap;
use korangar_loaders::FileLoader;
use mlua::{Lua, Value};

use super::lua::{execute_chunk, game_string};
use super::status_icon_builtin::BUILTIN_STATUS_ICONS;
use crate::loaders::GameFileLoader;

const STATE_ICON_DIRECTORY: &str = "data\\luafiles514\\lua files\\stateicon\\";

/// After `efstids.lub`: an id the client does not know resolves to `-1`
/// instead of `nil`. Otherwise a single newer EFST name in
/// `stateiconimginfo.lub` raises "table index is nil" and drops every icon.
const MISSING_EFST_GUARD: &str = r#"
if type(EFST_IDs) == "table" then
    local original = EFST_IDs
    EFST_IDs = setmetatable({}, {
        __index = function(_, key)
            local id = original[key]
            if id ~= nil then
                return id
            end
            return -1
        end,
    })
end
"#;

/// Icon file and tooltip text for one EFST id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatusIconInfo {
    /// File name under `data\texture\effect\`, such as `EXTREMITYFIST.TGA`.
    pub icon: Option<String>,
    /// First `descript` line, such as `Blessing`.
    pub title: Option<String>,
    /// Remaining `descript` lines without the `%s` time placeholder.
    pub description: Vec<String>,
    pub has_time_limit: bool,
}

pub(super) fn load_status_icons(game_file_loader: &GameFileLoader) -> HashMap<u16, StatusIconInfo> {
    let read = |name: &str| game_file_loader.get(&format!("{STATE_ICON_DIRECTORY}{name}")).ok();
    let table = match read("efstids.lub") {
        Some(efst_ids) => {
            let image_info = read("stateiconimginfo.lub");
            let icon_info = read("stateiconinfo.lub");
            parse_status_icons(&efst_ids, image_info.as_deref(), icon_info.as_deref()).unwrap_or_default()
        }
        None => HashMap::new(),
    };
    with_builtin_icons(table)
}

/// Give every status the client tables left without art its built-in icon
/// file. Table entries win.
pub fn with_builtin_icons(mut table: HashMap<u16, StatusIconInfo>) -> HashMap<u16, StatusIconInfo> {
    for (id, icon) in BUILTIN_STATUS_ICONS {
        let entry = table.entry(*id).or_default();
        if entry.icon.is_none() {
            entry.icon = Some((*icon).to_owned());
        }
    }
    table
}

/// Parse the three state icon chunks (source or 5.1 bytecode). Only
/// `efstids.lub` is required. A broken image or info chunk leaves its part
/// empty.
pub fn parse_status_icons(
    efst_ids: &[u8],
    image_info: Option<&[u8]>,
    icon_info: Option<&[u8]>,
) -> mlua::Result<HashMap<u16, StatusIconInfo>> {
    let state = Lua::new();
    execute_chunk(&state, "efstids.lub", efst_ids)?;
    state.load(MISSING_EFST_GUARD).set_name("efst guard").exec()?;

    let mut result: HashMap<u16, StatusIconInfo> = HashMap::new();

    if let Some(data) = image_info
        && execute_chunk(&state, "stateiconimginfo.lub", data).is_ok()
        && let Ok(priorities) = state.globals().get::<mlua::Table>("StateIconImgList")
    {
        // Lower priority numbers first, so a later (higher) list wins like the
        // last write in roBrowser.
        let mut lists: Vec<(i64, mlua::Table)> = priorities
            .pairs::<Value, Value>()
            .flatten()
            .filter_map(|(key, value)| match (key, value) {
                (Value::Integer(priority), Value::Table(list)) => Some((priority, list)),
                _ => None,
            })
            .collect();
        lists.sort_by_key(|(priority, _)| *priority);
        for (_, list) in lists {
            for (key, value) in list.pairs::<Value, Value>().flatten() {
                let (Some(id), Value::String(name)) = (efst_key(&key), value) else {
                    continue;
                };
                let Ok(name) = game_string(name) else {
                    continue;
                };
                if is_usable_icon_name(&name) {
                    result.entry(id).or_default().icon = Some(name);
                }
            }
        }
    }

    if let Some(data) = icon_info
        && execute_chunk(&state, "stateiconinfo.lub", data).is_ok()
        && let Ok(list) = state.globals().get::<mlua::Table>("StateIconList")
    {
        for (key, value) in list.pairs::<Value, Value>().flatten() {
            let (Some(id), Value::Table(info)) = (efst_key(&key), value) else {
                continue;
            };
            let entry = result.entry(id).or_default();
            entry.has_time_limit = matches!(
                info.get::<Value>("haveTimeLimit"),
                Ok(Value::Integer(1..)) | Ok(Value::Boolean(true))
            );
            if let Ok(lines) = info.get::<mlua::Table>("descript") {
                let mut texts = lines
                    .sequence_values::<mlua::Table>()
                    .flatten()
                    .filter_map(|line| match line.get::<Value>(1) {
                        Ok(Value::String(text)) => game_string(text).ok(),
                        _ => None,
                    })
                    .map(|text| text.trim().to_owned())
                    .filter(|text| !text.is_empty() && text != "%s");
                entry.title = texts.next();
                entry.description = texts.collect();
            }
        }
    }

    Ok(result)
}

fn efst_key(key: &Value) -> Option<u16> {
    match key {
        Value::Integer(id) if (0..=i64::from(u16::MAX)).contains(id) => Some(*id as u16),
        Value::Number(id) if id.fract() == 0.0 && (0.0..=f64::from(u16::MAX)).contains(id) => Some(*id as u16),
        _ => None,
    }
}

/// `".tga"` alone is a placeholder in the kRO table.
fn is_usable_icon_name(name: &str) -> bool {
    let trimmed = name.trim();
    let stem = trimmed.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(trimmed);
    !stem.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::parse_status_icons;

    const IDS: &[u8] = b"EFST_IDs = { EFST_PROVOKE = 0, EFST_BLESSING = 10, EFST_INC_AGI = 12 }";

    #[test]
    fn icons_and_descriptions_are_keyed_by_efst_id() {
        let images = br#"
            PRIORITY_RED = 0
            PRIORITY_WHITE = 4
            StateIconImgList = {
                [PRIORITY_RED] = { [EFST_IDs.EFST_PROVOKE] = "PROVOKE.TGA" },
                [PRIORITY_WHITE] = {
                    [EFST_IDs.EFST_BLESSING] = "BLESSING.TGA",
                    [EFST_IDs.EFST_NOT_IN_THIS_CLIENT] = "NEW.TGA",
                    [EFST_IDs.EFST_INC_AGI] = ".tga",
                },
            }
        "#;
        let info = br#"
            COLOR_TIME = { 255, 176, 98 }
            StateIconList = {}
            StateIconList[EFST_IDs.EFST_BLESSING] = {
                haveTimeLimit = 1,
                posTimeLimitStr = 2,
                descript = {
                    { "Blessing", { 155, 202, 155 } },
                    { "%s", COLOR_TIME },
                    { "Increase DEX, INT, STR" },
                },
            }
        "#;
        let table = parse_status_icons(IDS, Some(images), Some(info)).unwrap();
        assert_eq!(table[&0].icon.as_deref(), Some("PROVOKE.TGA"));
        let blessing = &table[&10];
        assert_eq!(blessing.icon.as_deref(), Some("BLESSING.TGA"));
        assert_eq!(blessing.title.as_deref(), Some("Blessing"));
        assert_eq!(blessing.description, vec!["Increase DEX, INT, STR".to_owned()]);
        assert!(blessing.has_time_limit);
        // Placeholder name and unknown EFST names do not create icons.
        assert!(table.get(&12).is_none_or(|entry| entry.icon.is_none()));
        assert!(!table.contains_key(&u16::MAX));
    }

    #[test]
    fn builtin_icons_fill_gaps_without_overriding_the_tables() {
        use super::{StatusIconInfo, with_builtin_icons};

        let mut table = hashbrown::HashMap::new();
        table.insert(0, StatusIconInfo {
            icon: Some("FROM_LUB.TGA".into()),
            ..Default::default()
        });
        table.insert(10, StatusIconInfo {
            title: Some("Blessing".into()),
            ..Default::default()
        });
        let table = with_builtin_icons(table);
        assert_eq!(table[&0].icon.as_deref(), Some("FROM_LUB.TGA"));
        assert_eq!(table[&10].icon.as_deref(), Some("블레싱.tga"));
        assert_eq!(table[&10].title.as_deref(), Some("Blessing"));
        assert_eq!(table[&12].icon.as_deref(), Some("민첩성증가.tga"));
    }

    #[test]
    fn broken_optional_chunks_leave_the_rest() {
        let table = parse_status_icons(IDS, Some(b"this is not lua"), None).unwrap();
        assert!(table.is_empty());
        assert!(parse_status_icons(b"also not lua (", None, None).is_err());
    }
}
