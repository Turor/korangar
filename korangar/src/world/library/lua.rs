//! Lua chunks are byte strings, not necessarily UTF-8 text. Never rewrite
//! assets to repair an execution/conversion error: text may be a user
//! translation and a compiled chunk may simply use a different pointer width.

use std::borrow::Cow;

use encoding_rs::EUC_KR;
use mlua::{Error, ErrorContext, Lua};

/// Adapt recognized bytecode in memory only. Source strings (including their
/// legacy-encoded literals/comments) must reach Lua unchanged. UTF-8's optional
/// BOM is not understood by Lua 5.1's buffer loader; strip only that signature.
fn chunk_bytes(data: &[u8]) -> mlua::Result<Cow<'_, [u8]>> {
    if data.starts_with(b"\x1bLua") {
        lunify::unify(data, &lunify::Format::default(), &lunify::Settings::default())
            .map(Cow::Owned)
            .map_err(|error| Error::RuntimeError(format!("Cannot adapt Lua bytecode: {error:?}")))
    } else if data.starts_with(b"\xff\xfe") || data.starts_with(b"\xfe\xff") {
        Err(Error::RuntimeError(
            "UTF-16 Lua source is unsupported; supply UTF-8 or the original legacy-byte source".into(),
        ))
    } else {
        Ok(Cow::Borrowed(data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data)))
    }
}

pub(super) fn execute_chunk(state: &Lua, name: &str, data: &[u8]) -> mlua::Result<()> {
    let bytes = chunk_bytes(data).context(format!("preparing Lua asset {name}"))?;
    state
        .load(bytes.as_ref())
        .set_name(name)
        .exec()
        .context(format!("executing Lua asset {name}"))
}

/// Prefer valid UTF-8 verbatim (including non-Korean user translations). Only
/// otherwise decode the original bytes as Windows-949/EUC-KR, without lossy
/// replacement, low-byte truncation, or rewriting the source/cache on disk.
///
/// This matches roBrowser `TextEncoding.smartDecode`: UTF-8 when the bytes are
/// well-formed, CP949/EUC-KR for legacy Korean client files. `encoding_rs`'s
/// `EUC_KR` is the Windows-949 mapping.
pub(super) fn decode_game_bytes(bytes: &[u8]) -> Result<String, ()> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.to_owned());
    }
    EUC_KR
        .decode_without_bom_handling_and_without_replacement(bytes)
        .map(Cow::into_owned)
        .ok_or(())
}

pub(super) fn game_string(value: mlua::String) -> mlua::Result<String> {
    let bytes = value.as_bytes();
    decode_game_bytes(&bytes).map_err(|_| Error::RuntimeError("Lua string is neither valid UTF-8 nor valid Windows-949/EUC-KR".into()))
}

pub(super) fn value_u32(value: mlua::Value) -> Option<u32> {
    match value {
        mlua::Value::Integer(number) if number >= 0 => u32::try_from(number as u64).ok(),
        mlua::Value::Number(number) if number.is_finite() && number >= 0.0 && number <= u32::MAX as f64 => Some(number as u32),
        mlua::Value::String(text) => game_string(text).ok().and_then(|text| text.trim().parse().ok()),
        _ => None,
    }
}

/// Numeric Lua array (`{ [1] = 8, [2] = 10 }` or a bare number), in key order.
pub(super) fn value_u32_list(value: mlua::Value) -> Vec<u32> {
    let mlua::Value::Table(table) = value else {
        return value_u32(value).into_iter().collect();
    };
    let mut indexed = Vec::new();
    for pair in table.pairs::<mlua::Value, mlua::Value>().flatten() {
        let Some(number) = value_u32(pair.1) else { continue };
        let key = value_u32(pair.0).unwrap_or(u32::MAX);
        indexed.push((key, number));
    }
    indexed.sort_by_key(|(key, _)| *key);
    indexed.into_iter().map(|(_, number)| number).collect()
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use encoding_rs::EUC_KR;
    use lunify::{BitWidth, Format, Settings, unify};
    use mlua::Lua;

    use super::{chunk_bytes, decode_game_bytes, execute_chunk, game_string};
    use crate::world::library::LuaExt;

    #[test]
    fn source_is_borrowed_and_legacy_literals_are_not_rewritten() {
        let lua = Lua::new();
        let source = b"-- synthetic legacy fixture\nvalue = '\xb0\xa1'";
        assert!(matches!(chunk_bytes(source).unwrap(), Cow::Borrowed(_)));
        assert_eq!(chunk_bytes(source).unwrap().as_ref(), source);
        execute_chunk(&lua, "translated.lub", source).unwrap();
        let value: mlua::String = lua.globals().get("value").unwrap();
        assert_eq!(&*value.as_bytes(), b"\xb0\xa1");
        assert_eq!(game_string(value).unwrap(), "가");
    }

    #[test]
    fn utf8_translations_survive_verbatim() {
        let lua = Lua::new();
        // The old char-as-u8 helper turned U+01B0/U+01A1 into B0 A1 (가).
        for text in ["ươ", "한글", "日本語", "Café", "🦀", "ASCII"] {
            assert_eq!(game_string(lua.create_string(text).unwrap()).unwrap(), text);
        }
        execute_chunk(&lua, "utf8.lub", "value = 'ươ 🦀 한글'".as_bytes()).unwrap();
        assert_eq!(game_string(lua.globals().get("value").unwrap()).unwrap(), "ươ 🦀 한글");
    }

    #[test]
    fn legacy_game_strings_decode_without_replacement() {
        let lua = Lua::new();
        let (bytes, _, had_errors) = EUC_KR.encode("가나다");
        assert!(!had_errors);
        assert_eq!(game_string(lua.create_string(bytes.as_ref()).unwrap()).unwrap(), "가나다");
        assert!(game_string(lua.create_string([0xFF]).unwrap()).is_err());
    }

    #[test]
    fn cp949_item_resource_bytes_decode_to_hangul() {
        // `identifiedResourceName` for Red Potion in itemInfo.turoran.renewal11.lua.
        let bytes = [0xBB, 0xA1, 0xB0, 0xA3, 0xC6, 0xF7, 0xBC, 0xC7];
        assert_eq!(decode_game_bytes(&bytes).unwrap(), "빨간포션");
        let (encoded, _, had_errors) = EUC_KR.encode("빨간포션");
        assert!(!had_errors);
        assert_eq!(encoded.as_ref(), bytes);
    }

    #[test]
    fn utf8_bom_is_removed_in_memory_and_utf16_is_explicitly_rejected() {
        let lua = Lua::new();
        let source = b"\xef\xbb\xbfvalue = 42";
        execute_chunk(&lua, "bom.lua", source).unwrap();
        assert_eq!(lua.globals().get::<i32>("value").unwrap(), 42);
        assert!(source.starts_with(b"\xef\xbb\xbf"));
        let error = execute_chunk(&lua, "utf16.lua", b"\xff\xfev\0").unwrap_err().to_string();
        assert!(error.contains("utf16.lua") && error.contains("UTF-16"));
    }

    #[test]
    fn foreign_32bit_bytecode_executes_and_preserves_legacy_strings() {
        let lua = Lua::new();
        let function = lua.load(b"value = '\xb0\xa1'; answer = 42".as_slice()).into_function().unwrap();
        let native = function.dump(false);
        let foreign = unify(
            &native,
            &Format {
                size_t_width: BitWidth::Bit32,
                ..Format::default()
            },
            &Settings::default(),
        )
        .unwrap();
        assert_eq!(&foreign[..5], b"\x1bLuaQ");
        assert_eq!(foreign[8], 4);
        let original = foreign.clone();
        execute_chunk(&lua, "foreign32.lub", &foreign).unwrap();
        assert_eq!(foreign, original);
        assert_eq!(lua.globals().get::<i32>("answer").unwrap(), 42);
        let value: mlua::String = lua.globals().get("value").unwrap();
        assert_eq!(&*value.as_bytes(), b"\xb0\xa1");
        assert_eq!(game_string(value).unwrap(), "가");
    }

    #[test]
    fn foreign_bytecode_preserves_utf8_translations_in_both_endiannesses() {
        let lua = Lua::new();
        let native = lua.load("value = 'ươ 🦀 한글'").into_function().unwrap().dump(false);
        for endianness in [lunify::Endianness::Little, lunify::Endianness::Big] {
            let foreign = unify(
                &native,
                &Format {
                    size_t_width: BitWidth::Bit32,
                    endianness,
                    ..Format::default()
                },
                &Settings::default(),
            )
            .unwrap();
            execute_chunk(&lua, "translated32.lub", &foreign).unwrap();
            assert_eq!(game_string(lua.globals().get("value").unwrap()).unwrap(), "ươ 🦀 한글");
        }
    }

    #[test]
    fn native_bytecode_remains_unchanged() {
        let lua = Lua::new();
        let original = lua.load("answer = 123").into_function().unwrap().dump(false);
        assert_eq!(chunk_bytes(&original).unwrap().as_ref(), original);
        execute_chunk(&lua, "native.lub", &original).unwrap();
        assert_eq!(lua.globals().get::<i32>("answer").unwrap(), 123);
    }

    #[test]
    fn malformed_chunks_report_asset_name_without_source_rewrite_or_retry() {
        let lua = Lua::new();
        for (name, bytes) in [
            ("broken.lua", b"answer = ???".as_slice()),
            ("truncated.lub", b"\x1bLuaQ"),
            ("lua54.lub", b"\x1bLuaT"),
        ] {
            let original = bytes.to_vec();
            let error = execute_chunk(&lua, name, bytes).unwrap_err().to_string();
            assert!(error.contains(name), "{error}");
            assert_eq!(bytes, original);
        }
    }

    struct MemoryLoader;

    impl korangar_loaders::FileLoader for MemoryLoader {
        fn get(&self, path: &str) -> Result<Vec<u8>, korangar_loaders::FileNotFoundError> {
            match path {
                "first.lua" => Ok(b"answer = 41".to_vec()),
                "second.lua" => Ok(b"answer = answer + 1".to_vec()),
                "prior.lua" => Ok(b"prior = 3".to_vec()),
                // Readable, but syntax-invalid. Must not block the stock candidate.
                "broken-syntax.lua" => Ok(b"answer = ???".to_vec()),
                // Assigns globals, then errors. Those assignments must not survive.
                "broken-runtime.lua" => Ok(b"prior = 0\nanswer = 1\nerror('overlay boom')".to_vec()),
                "stock.lua" => Ok(b"answer = prior + 4".to_vec()),
                _ => Err(korangar_loaders::FileNotFoundError::new(path.to_owned())),
            }
        }
    }

    #[test]
    fn normal_multi_file_loader_preserves_order_and_reports_missing_asset() {
        let state = Lua::load_from_game_files(&MemoryLoader, &["first.lua", "second.lua"]).unwrap();
        assert_eq!(state.globals().get::<i32>("answer").unwrap(), 42);
        let error = Lua::load_from_game_files(&MemoryLoader, &["missing.lua"])
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("missing.lua"));
    }

    #[test]
    fn fallback_groups_prefer_turoran_names_then_stock() {
        let state =
            Lua::load_from_game_files_with_fallbacks(&MemoryLoader, &[&["missing-turoran.lua", "first.lua"], &["second.lua"]]).unwrap();
        assert_eq!(state.globals().get::<i32>("answer").unwrap(), 42);

        let error = Lua::load_from_game_files_with_fallbacks(&MemoryLoader, &[&["missing-a.lua", "missing-b.lua"]])
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("missing-b.lua") || error.contains("missing-a.lua"));
    }

    #[test]
    fn broken_overlay_falls_through_without_keeping_partial_globals() {
        let state = Lua::load_from_game_files_with_fallbacks(&MemoryLoader, &[&["prior.lua"], &[
            "broken-syntax.lua",
            "broken-runtime.lua",
            "stock.lua",
        ]])
        .unwrap();
        assert_eq!(state.globals().get::<i32>("prior").unwrap(), 3);
        assert_eq!(state.globals().get::<i32>("answer").unwrap(), 7);

        let error = Lua::load_from_game_files_with_fallbacks(&MemoryLoader, &[&["broken-syntax.lua", "broken-runtime.lua"]])
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("broken-runtime.lua") || error.contains("overlay boom"),
            "{error}"
        );
    }

    #[test]
    fn readable_working_overlay_is_still_preferred_over_stock() {
        let state = Lua::load_from_game_files_with_fallbacks(&MemoryLoader, &[&["first.lua", "stock.lua"]]).unwrap();
        assert_eq!(state.globals().get::<i32>("answer").unwrap(), 41);
        assert!(state.globals().get::<i32>("prior").is_err());
    }
}
