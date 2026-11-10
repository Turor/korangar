use std::borrow::Cow;
use std::fmt::{Display, Formatter};

use ragnarok_packets::ItemId;

use super::item_info::ItemInfo;
use super::{Library, Table};
use crate::loaders::GameFileLoader;

static DEFAULT_STRING: &str = "사과"; // Apple
static DEFAULT_VALUE: ItemResource = ItemResource(Cow::Borrowed(DEFAULT_STRING));

#[derive(Debug, Clone, Copy)]
pub struct ItemResourceKey {
    pub item_id: ItemId,
    pub is_identified: bool,
}

#[derive(Debug, Clone)]
pub struct ItemResource(Cow<'static, str>);

impl Display for ItemResource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Inventory icon under `data/texture/`, the 24×24 BMP ro clients put in
/// `유저인터페이스/item/<resource>.bmp`.
pub fn inventory_icon_texture_path(resource_name: &str) -> Result<String, &'static str> {
    if resource_name.is_empty() {
        return Err("missing name mapping");
    }
    if resource_name
        .chars()
        .any(|character| character.is_control() || matches!(character, '%' | ':' | '?' | '#' | '\\' | '/'))
    {
        return Err("EUC-KR/encoding");
    }
    Ok(format!("유저인터페이스\\item\\{resource_name}.bmp"))
}

/// Bucket a loader error so logs say why an equipped item has no icon.
pub fn classify_item_icon_failure(error: &str) -> &'static str {
    let lower = error.to_ascii_lowercase();
    if lower.contains("404") || lower.contains("not found") || lower.contains("can't find") || lower.contains("asset not found") {
        "remote 404"
    } else if lower.contains("euc-kr") || lower.contains("encoding") || lower.contains("utf-8") {
        "EUC-KR/encoding"
    } else if lower.contains("decode") || lower.contains("parse") || lower.contains("image format") {
        "parse miss"
    } else if lower.contains("missing name") {
        "missing name mapping"
    } else {
        "remote retrieval failed"
    }
}

impl ItemResource {
    pub(super) const fn not_found_value() -> Self {
        ItemResource(Cow::Borrowed(DEFAULT_STRING))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    pub(super) fn from_option(value: Option<String>) -> Self {
        match value {
            Some(s) => ItemResource(Cow::Owned(s)),
            None => ItemResource::not_found_value(),
        }
    }

    pub(super) fn is_placeholder(&self) -> bool {
        self.0 == DEFAULT_STRING || self.0.is_empty()
    }
}

impl Table for ItemResource {
    type Key<'a> = ItemResourceKey;
    type Storage = ();

    fn load(_game_file_loader: &GameFileLoader) -> mlua::Result<Self::Storage> {
        Ok(())
    }

    fn try_get<'a, 'b>(library: &'a Library, key: Self::Key<'b>) -> Option<&'a Self> {
        let item_info = ItemInfo::try_get(library, key.item_id)?;
        Some(if key.is_identified {
            &item_info.identified_resource
        } else {
            &item_info.unidentified_resource
        })
    }

    fn get<'a, 'b>(library: &'a Library, key: Self::Key<'b>) -> &'a Self {
        Self::try_get(library, key).unwrap_or(&DEFAULT_VALUE)
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_item_icon_failure, inventory_icon_texture_path};

    #[test]
    fn korean_resource_maps_to_the_inventory_bmp() {
        let path = inventory_icon_texture_path("빨간포션").unwrap();
        assert_eq!(path, "유저인터페이스\\item\\빨간포션.bmp");
        assert_eq!(
            classify_item_icon_failure("can't find file: data\\texture\\x.bmp"),
            "remote 404"
        );
        assert_eq!(classify_item_icon_failure("asset not found"), "remote 404");
        assert_eq!(
            classify_item_icon_failure("verified remote retrieval failed: asset not found"),
            "remote 404"
        );
        assert_eq!(
            classify_item_icon_failure("Lua string is neither valid UTF-8 nor valid Windows-949/EUC-KR"),
            "EUC-KR/encoding"
        );
        assert_eq!(classify_item_icon_failure("Failed to decode image"), "parse miss");
        assert_eq!(inventory_icon_texture_path("").unwrap_err(), "missing name mapping");
        assert_eq!(inventory_icon_texture_path("a:b").unwrap_err(), "EUC-KR/encoding");
    }
}
