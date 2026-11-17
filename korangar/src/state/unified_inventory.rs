//! Unified inventory (Matthew t275u).
//!
//! The cart and Kafra or guild storage no longer have windows of their own.
//! They are a second pane of the Inventory window:
//!
//! - Storage opening (`SetStorage`) snaps the inventory into a split view, bag
//!   on the left, storage on the right. Closing storage (the Close button, the
//!   window's X, Escape, a map change or `ZC_CLOSE_STORE`) returns to the
//!   single view.
//! - With a cart equipped, the Inventory/Cart tabs (or Alt+W) swap between the
//!   single view and the same split with the cart on the right. `ZC_CARTOFF`
//!   takes the Cart tab away.
//!
//! In the split each pane gets [`split_columns`] of the columns the whole
//! window would draw, so both scroll panes keep room for their scroll area
//! and the accent bar down the middle. One set of tabs, search and sort
//! drives both panes (`state::inventory_view`).

use std::collections::BTreeMap;
use std::path::Path as FilePath;

use rust_state::RustState;
use serde::{Deserialize, Serialize};

use crate::state::inventory_view::ViewControls;

/// What fills the right half of the split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondPane {
    Cart,
    Storage,
}

/// Columns each pane draws in the split: half the window's columns, less
/// one, never fewer than one. `total` is what the single view would draw at
/// this width.
pub fn split_columns(total: usize) -> usize {
    (total / 2).saturating_sub(1).max(1)
}

/// Pane and storage flags of the Inventory window.
#[derive(Clone, Debug, Default, PartialEq, Eq, RustState)]
pub struct UnifiedInventory {
    /// Storage is open on the server (`SetStorage` until a close).
    storage_open: bool,
    /// The player picked the Cart tab.
    cart_view: bool,
    /// A cart is equipped: `ZC_NOTIFY_CARTITEM_COUNTINFO` arrived and no
    /// `ZC_CARTOFF` since.
    cart_equipped: bool,
    /// Per-character tab and sort (`client/inventory_view.ron`).
    prefs: ViewPrefsTracker,
}

impl UnifiedInventory {
    pub fn storage_open(&self) -> bool {
        self.storage_open
    }

    pub fn prefs_mut(&mut self) -> &mut ViewPrefsTracker {
        &mut self.prefs
    }

    /// The Cart tab is selected and a cart is equipped.
    pub fn cart_view(&self) -> bool {
        self.cart_view && self.cart_equipped
    }

    /// Storage wins over the cart: it is the open transaction.
    pub fn second_pane(&self) -> Option<SecondPane> {
        if self.storage_open {
            Some(SecondPane::Storage)
        } else if self.cart_view() {
            Some(SecondPane::Cart)
        } else {
            None
        }
    }

    pub fn is_split(&self) -> bool {
        self.second_pane().is_some()
    }

    /// The Inventory/Cart tabs show while a cart is equipped and storage is
    /// not taking the second pane.
    pub fn shows_cart_tabs(&self) -> bool {
        self.cart_equipped && !self.storage_open
    }

    pub fn open_storage(&mut self) {
        self.storage_open = true;
    }

    /// Returns `true` when storage was open.
    pub fn close_storage(&mut self) -> bool {
        std::mem::take(&mut self.storage_open)
    }

    pub fn set_cart_equipped(&mut self, equipped: bool) {
        self.cart_equipped = equipped;
        if !equipped {
            self.cart_view = false;
        }
    }

    /// Inventory (`false`) or Cart (`true`) tab. Without a cart the Cart tab
    /// does nothing.
    pub fn show_cart(&mut self, cart: bool) {
        self.cart_view = cart && self.cart_equipped;
    }
}

/// What Alt+W does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CartHotkey {
    /// Open the Inventory window on the Cart tab.
    OpenOnCart,
    /// The window is open: switch it to the Cart tab.
    ShowCart,
    /// The Cart tab is showing: close the window.
    Close,
    /// No cart equipped.
    NoCart,
}

pub const NO_CART_MESSAGE: &str = "You don't have a cart.";

pub fn cart_hotkey(state: &UnifiedInventory, inventory_open: bool) -> CartHotkey {
    match (state.cart_equipped, inventory_open, state.cart_view()) {
        (false, ..) => CartHotkey::NoCart,
        (true, false, _) => CartHotkey::OpenOnCart,
        (true, true, false) => CartHotkey::ShowCart,
        (true, true, true) => CartHotkey::Close,
    }
}

/// Tab, sort and direction per character, `client/inventory_view.ron`.
pub const VIEW_PREFS_FILE: &str = "client/inventory_view.ron";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewPrefsFile {
    pub by_character: BTreeMap<String, ViewControls>,
}

impl ViewPrefsFile {
    /// Missing file is empty; an unreadable or invalid file is an error so a
    /// save does not drop other characters' entries.
    pub fn load_checked(path: &FilePath) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => ron::from_str(&text).map_err(|error| error.to_string()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn save(&self, path: &FilePath) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|error| error.to_string())?;
        let mut temporary = path.as_os_str().to_owned();
        temporary.push(".tmp");
        let temporary = std::path::PathBuf::from(temporary);
        std::fs::write(&temporary, text).map_err(|error| error.to_string())?;
        std::fs::rename(&temporary, path).map_err(|error| error.to_string())
    }
}

/// Which character's prefs are applied and what was last written, so the
/// file is only touched when the controls change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewPrefsTracker {
    character: Option<String>,
    written: Option<ViewControls>,
}

/// One frame of the prefs tracker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrefsStep {
    Nothing,
    /// A character came in: apply its saved controls (default when none).
    Apply(ViewControls),
    /// The controls changed: write them for this character.
    Save(String, ViewControls),
}

impl ViewPrefsTracker {
    pub fn step(&mut self, character: &str, current: ViewControls, file: impl FnOnce() -> ViewPrefsFile) -> PrefsStep {
        if character.is_empty() {
            return PrefsStep::Nothing;
        }
        if self.character.as_deref() != Some(character) {
            let saved = file().by_character.get(character).copied().unwrap_or_default();
            self.character = Some(character.to_owned());
            self.written = Some(saved);
            return PrefsStep::Apply(saved);
        }
        if self.written != Some(current) {
            self.written = Some(current);
            return PrefsStep::Save(character.to_owned(), current);
        }
        PrefsStep::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::inventory_view::{InventorySort, InventoryTab};

    #[test]
    fn split_columns_halve_and_leave_one() {
        assert_eq!(split_columns(10), 4);
        assert_eq!(split_columns(11), 4);
        assert_eq!(split_columns(12), 5);
        assert_eq!(split_columns(20), 9);
        assert_eq!(split_columns(8), 3);
        assert_eq!(split_columns(4), 1);
        assert_eq!(split_columns(3), 1);
        assert_eq!(split_columns(1), 1);
        assert_eq!(split_columns(0), 1);
        for total in 2..=40 {
            assert!(2 * split_columns(total) < total.max(3), "{total}");
        }
    }

    #[test]
    fn storage_takes_the_second_pane_over_the_cart() {
        let mut state = UnifiedInventory::default();
        assert_eq!(state.second_pane(), None);
        state.show_cart(true);
        assert_eq!(state.second_pane(), None, "no cart, no cart pane");
        state.set_cart_equipped(true);
        assert!(state.shows_cart_tabs());
        state.show_cart(true);
        assert_eq!(state.second_pane(), Some(SecondPane::Cart));
        state.open_storage();
        assert_eq!(state.second_pane(), Some(SecondPane::Storage));
        assert!(!state.shows_cart_tabs());
        assert!(state.close_storage());
        assert!(!state.close_storage());
        assert_eq!(state.second_pane(), Some(SecondPane::Cart), "back to the cart split");
        state.show_cart(false);
        assert!(!state.is_split());
    }

    #[test]
    fn losing_the_cart_drops_the_cart_view() {
        let mut state = UnifiedInventory::default();
        state.set_cart_equipped(true);
        state.show_cart(true);
        state.set_cart_equipped(false);
        assert!(!state.cart_view());
        state.set_cart_equipped(true);
        assert!(!state.cart_view(), "a new cart starts on the Inventory tab");
    }

    #[test]
    fn alt_w_opens_switches_and_closes() {
        let mut state = UnifiedInventory::default();
        assert_eq!(cart_hotkey(&state, false), CartHotkey::NoCart);
        state.set_cart_equipped(true);
        assert_eq!(cart_hotkey(&state, false), CartHotkey::OpenOnCart);
        assert_eq!(cart_hotkey(&state, true), CartHotkey::ShowCart);
        state.show_cart(true);
        assert_eq!(cart_hotkey(&state, true), CartHotkey::Close);
    }

    #[test]
    fn prefs_apply_once_per_character_and_save_on_change() {
        let cards = ViewControls {
            tab: InventoryTab::Cards,
            sort: InventorySort::Zeny,
            reversed: true,
        };
        let mut file = ViewPrefsFile::default();
        file.by_character.insert("Alchemic Void".to_owned(), cards);
        let mut tracker = ViewPrefsTracker::default();
        assert_eq!(
            tracker.step("", ViewControls::default(), ViewPrefsFile::default),
            PrefsStep::Nothing
        );
        assert_eq!(
            tracker.step("Alchemic Void", ViewControls::default(), || file.clone()),
            PrefsStep::Apply(cards)
        );
        assert_eq!(tracker.step("Alchemic Void", cards, || unreachable!()), PrefsStep::Nothing);
        let name = ViewControls {
            sort: InventorySort::Name,
            ..cards
        };
        assert_eq!(
            tracker.step("Alchemic Void", name, || unreachable!()),
            PrefsStep::Save("Alchemic Void".to_owned(), name)
        );
        assert_eq!(tracker.step("Alchemic Void", name, || unreachable!()), PrefsStep::Nothing);
        assert_eq!(
            tracker.step("Madbird", name, ViewPrefsFile::default),
            PrefsStep::Apply(ViewControls::default())
        );
    }

    #[test]
    fn prefs_file_round_trips_and_refuses_garbage() {
        let directory = std::env::temp_dir().join(format!("korangar-view-prefs-{}", std::process::id()));
        let path = directory.join("inventory_view.ron");
        let _ = std::fs::remove_dir_all(&directory);
        assert_eq!(ViewPrefsFile::load_checked(&path).unwrap(), ViewPrefsFile::default());
        let mut file = ViewPrefsFile::default();
        file.by_character.insert("Madbird".to_owned(), ViewControls {
            tab: InventoryTab::Ammo,
            sort: InventorySort::Newest,
            reversed: false,
        });
        file.save(&path).unwrap();
        assert_eq!(ViewPrefsFile::load_checked(&path).unwrap(), file);
        std::fs::write(&path, "not ron").unwrap();
        assert!(ViewPrefsFile::load_checked(&path).is_err());
        let _ = std::fs::remove_dir_all(&directory);
    }
}
