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
use std::time::{Duration, Instant};

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
    /// Window height grown for the split, to restore after.
    fit: SplitFit,
    /// Which storage is open (meaningful while `storage_open`).
    storage_kind: StorageKind,
    /// A Storage tab was clicked and its command sent; no reply yet.
    requested: Option<(StorageKind, Instant)>,
}

/// Kafra storage or guild storage (t279u storage tabs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StorageKind {
    #[default]
    Personal,
    Guild,
}

impl StorageKind {
    /// The atcommand the tab sends. Hercules has no client packet that opens
    /// storage; `ACMD(storage)` / `ACMD(guildstorage)` call `storage->open` /
    /// `gstorage->open`, the same as a Kafra.
    pub fn command(self) -> &'static str {
        match self {
            Self::Personal => "@storage",
            Self::Guild => "@gstorage",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Personal => "Storage",
            Self::Guild => "Guild Storage",
        }
    }
}

/// The top tab row of the inventory window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneTab {
    Inventory,
    Cart,
    Storage(StorageKind),
}

/// What a click on a [`PaneTab`] asks of the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneTabStep {
    /// Only the view changed.
    Nothing,
    /// Send `CZ_CLOSE_STORE`.
    CloseStorage,
    /// Send the storage command as chat.
    Request(StorageKind),
    /// Close the open storage, then ask for the other one.
    CloseAndRequest(StorageKind),
}

/// A storage request with no reply after this long is given up: the server
/// refused it (the refusal shows in chat) and the tab goes back to idle.
pub const STORAGE_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// What [`SplitFit::step`] asks of the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FitStep {
    Nothing,
    Resize(f32),
}

/// Grows the inventory window when it splits so each half shows
/// [`SPLIT_MINIMUM_ROWS`], and gives the old height back when it joins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SplitFit {
    pane: Option<SecondPane>,
    restore_height: Option<u32>,
}

/// Rows each half of the split shows at least, screen permitting.
pub const SPLIT_MINIMUM_ROWS: usize = 4;

impl SplitFit {
    /// `needed` shows [`SPLIT_MINIMUM_ROWS`] in this pane mode, `room` is the
    /// tallest the screen allows. Acts once per change of pane.
    pub fn step(&mut self, pane: Option<SecondPane>, height: f32, needed: f32, room: f32) -> FitStep {
        if pane == self.pane {
            return FitStep::Nothing;
        }
        self.pane = pane;
        match pane {
            Some(_) => {
                let target = needed.min(room).max(height);
                if target > height + 0.5 {
                    self.restore_height.get_or_insert(height.round() as u32);
                    FitStep::Resize(target)
                } else {
                    FitStep::Nothing
                }
            }
            None => match self.restore_height.take() {
                Some(height) => FitStep::Resize(height as f32),
                None => FitStep::Nothing,
            },
        }
    }

    /// The window closed; the next open starts at its saved size.
    pub fn forget(&mut self) {
        *self = Self::default();
    }
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

    /// Opening storage shows storage. With a cart equipped the Storage /
    /// Cart tabs (or Alt+W) swap the second pane to the cart while storage
    /// stays open on the server.
    pub fn second_pane(&self) -> Option<SecondPane> {
        if self.cart_view() {
            Some(SecondPane::Cart)
        } else if self.storage_open {
            Some(SecondPane::Storage)
        } else {
            None
        }
    }

    pub fn is_split(&self) -> bool {
        self.second_pane().is_some()
    }

    /// Storage opens on the storage pane. It is the storage a tab asked for,
    /// otherwise Kafra storage (an NPC opened it; the item list does not
    /// say which).
    pub fn open_storage(&mut self) {
        self.storage_kind = self.requested.take().map(|(kind, _)| kind).unwrap_or_default();
        self.storage_open = true;
        self.cart_view = false;
    }

    /// Tabs left to right. Cart only with a cart equipped; both storages
    /// always (Guild Storage is drawn disabled outside a guild).
    pub fn pane_tabs(&self) -> Vec<PaneTab> {
        let mut tabs = vec![PaneTab::Inventory];
        if self.cart_equipped {
            tabs.push(PaneTab::Cart);
        }
        tabs.push(PaneTab::Storage(StorageKind::Personal));
        tabs.push(PaneTab::Storage(StorageKind::Guild));
        tabs
    }

    /// The storage on screen, if any.
    pub fn open_storage_kind(&self) -> Option<StorageKind> {
        self.storage_open.then_some(self.storage_kind)
    }

    pub fn pending_storage(&self) -> Option<StorageKind> {
        self.requested.map(|(kind, _)| kind)
    }

    pub fn is_tab_selected(&self, tab: PaneTab) -> bool {
        match tab {
            PaneTab::Inventory => self.second_pane().is_none(),
            PaneTab::Cart => self.cart_view(),
            PaneTab::Storage(kind) => self.second_pane() == Some(SecondPane::Storage) && self.storage_kind == kind,
        }
    }

    /// A click on a top tab. Changes the view here; the step says what to
    /// send. Clicking the shown storage again, or Inventory, closes it.
    pub fn click_pane_tab(&mut self, tab: PaneTab, now: Instant) -> PaneTabStep {
        match tab {
            PaneTab::Inventory => {
                self.cart_view = false;
                self.requested = None;
                match self.storage_open {
                    true => PaneTabStep::CloseStorage,
                    false => PaneTabStep::Nothing,
                }
            }
            PaneTab::Cart => {
                self.show_cart(true);
                PaneTabStep::Nothing
            }
            PaneTab::Storage(kind) => match self.open_storage_kind() {
                // Back from the cart pane to the open storage.
                Some(open) if open == kind && self.cart_view() => {
                    self.cart_view = false;
                    PaneTabStep::Nothing
                }
                Some(open) if open == kind => PaneTabStep::CloseStorage,
                Some(_) => {
                    self.requested = Some((kind, now));
                    PaneTabStep::CloseAndRequest(kind)
                }
                None if self.pending_storage() == Some(kind) => PaneTabStep::Nothing,
                None => {
                    self.requested = Some((kind, now));
                    PaneTabStep::Request(kind)
                }
            },
        }
    }

    /// Once per frame: a request the server did not answer was refused.
    pub fn expire_request(&mut self, now: Instant) {
        if let Some((_, sent)) = self.requested
            && now.saturating_duration_since(sent) >= STORAGE_REQUEST_TIMEOUT
        {
            self.requested = None;
        }
    }

    /// Map-server disconnect or character switch: nothing of the previous
    /// character's cart or storage carries over. The prefs tracker is kept so
    /// the next character's tab and sort are applied when it arrives.
    pub fn fit_mut(&mut self) -> &mut SplitFit {
        &mut self.fit
    }

    pub fn reset_session(&mut self) {
        self.storage_open = false;
        self.cart_view = false;
        self.cart_equipped = false;
        self.storage_kind = StorageKind::Personal;
        self.requested = None;
        self.fit.forget();
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
    /// Storage is open and the cart is showing: back to the storage pane
    /// (Alt+W never closes storage).
    ShowStorage,
    /// No cart equipped.
    NoCart,
}

pub const NO_CART_MESSAGE: &str = "You don't have a cart.";

pub fn cart_hotkey(state: &UnifiedInventory, inventory_open: bool) -> CartHotkey {
    match (state.cart_equipped, inventory_open, state.cart_view(), state.storage_open) {
        (false, ..) => CartHotkey::NoCart,
        (true, false, ..) => CartHotkey::OpenOnCart,
        (true, true, false, _) => CartHotkey::ShowCart,
        (true, true, true, true) => CartHotkey::ShowStorage,
        (true, true, true, false) => CartHotkey::Close,
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
    fn the_split_grows_the_window_once_and_gives_the_height_back() {
        let mut fit = SplitFit::default();
        assert_eq!(fit.step(None, 300.0, 0.0, 700.0), FitStep::Nothing);
        assert_eq!(fit.step(Some(SecondPane::Storage), 300.0, 420.0, 700.0), FitStep::Resize(420.0));
        // Same pane next frame: nothing more.
        assert_eq!(fit.step(Some(SecondPane::Storage), 420.0, 420.0, 700.0), FitStep::Nothing);
        // Storage -> cart keeps the first height to restore.
        assert_eq!(fit.step(Some(SecondPane::Cart), 420.0, 440.0, 700.0), FitStep::Resize(440.0));
        assert_eq!(fit.step(None, 440.0, 0.0, 700.0), FitStep::Resize(300.0));
        // A small screen caps the growth; a tall window is left alone.
        let mut fit = SplitFit::default();
        assert_eq!(fit.step(Some(SecondPane::Cart), 300.0, 900.0, 560.0), FitStep::Resize(560.0));
        let mut fit = SplitFit::default();
        assert_eq!(fit.step(Some(SecondPane::Cart), 500.0, 420.0, 700.0), FitStep::Nothing);
        assert_eq!(fit.step(None, 500.0, 0.0, 700.0), FitStep::Nothing);
    }

    #[test]
    fn storage_tabs_request_open_and_close_storage() {
        let now = Instant::now();
        let mut panes = UnifiedInventory::default();
        let personal = PaneTab::Storage(StorageKind::Personal);
        let guild = PaneTab::Storage(StorageKind::Guild);
        assert_eq!(panes.pane_tabs(), [PaneTab::Inventory, personal, guild], "no cart: no Cart tab");
        assert!(panes.is_tab_selected(PaneTab::Inventory));

        // Click Storage: the command goes out, nothing opens until the server answers.
        assert_eq!(panes.click_pane_tab(personal, now), PaneTabStep::Request(StorageKind::Personal));
        assert_eq!(panes.pending_storage(), Some(StorageKind::Personal));
        assert!(!panes.is_split());
        // A second click while waiting sends nothing more.
        assert_eq!(panes.click_pane_tab(personal, now), PaneTabStep::Nothing);
        panes.open_storage();
        assert_eq!(panes.open_storage_kind(), Some(StorageKind::Personal));
        assert!(panes.is_tab_selected(personal) && !panes.is_tab_selected(guild));
        assert_eq!(panes.pending_storage(), None);

        // The shown storage tab again closes it; so does Inventory.
        assert_eq!(panes.click_pane_tab(personal, now), PaneTabStep::CloseStorage);
        assert_eq!(panes.click_pane_tab(PaneTab::Inventory, now), PaneTabStep::CloseStorage);
        panes.close_storage();
        assert_eq!(panes.click_pane_tab(PaneTab::Inventory, now), PaneTabStep::Nothing);

        // Guild storage opens as guild; switching from it closes it first.
        assert_eq!(panes.click_pane_tab(guild, now), PaneTabStep::Request(StorageKind::Guild));
        panes.open_storage();
        assert!(panes.is_tab_selected(guild));
        assert_eq!(
            panes.click_pane_tab(personal, now),
            PaneTabStep::CloseAndRequest(StorageKind::Personal)
        );

        // A Kafra opens personal storage with no tab involved.
        let mut panes = UnifiedInventory::default();
        panes.open_storage();
        assert_eq!(panes.open_storage_kind(), Some(StorageKind::Personal));
    }

    #[test]
    fn a_refused_storage_request_returns_the_tab_to_idle() {
        let now = Instant::now();
        let mut panes = UnifiedInventory::default();
        let guild = PaneTab::Storage(StorageKind::Guild);
        assert_eq!(panes.click_pane_tab(guild, now), PaneTabStep::Request(StorageKind::Guild));
        panes.expire_request(now + Duration::from_millis(500));
        assert_eq!(panes.pending_storage(), Some(StorageKind::Guild));
        // "You are not in a guild." arrives in chat; no storage list follows.
        panes.expire_request(now + STORAGE_REQUEST_TIMEOUT);
        assert_eq!(panes.pending_storage(), None);
        assert!(panes.is_tab_selected(PaneTab::Inventory) && !panes.is_tab_selected(guild));
        // It can be asked again.
        assert_eq!(panes.click_pane_tab(guild, now), PaneTabStep::Request(StorageKind::Guild));
    }

    #[test]
    fn storage_and_cart_tabs_share_the_second_pane() {
        let now = Instant::now();
        let mut panes = UnifiedInventory::default();
        panes.set_cart_equipped(true);
        let personal = PaneTab::Storage(StorageKind::Personal);
        assert_eq!(panes.pane_tabs(), [
            PaneTab::Inventory,
            PaneTab::Cart,
            personal,
            PaneTab::Storage(StorageKind::Guild)
        ]);
        panes.click_pane_tab(personal, now);
        panes.open_storage();
        assert_eq!(panes.click_pane_tab(PaneTab::Cart, now), PaneTabStep::Nothing);
        assert!(panes.is_tab_selected(PaneTab::Cart) && !panes.is_tab_selected(personal));
        // Storage tab brings storage back without closing it.
        assert_eq!(panes.click_pane_tab(personal, now), PaneTabStep::Nothing);
        assert!(panes.is_tab_selected(personal));
        assert!(panes.storage_open());
    }

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
        assert!(state.pane_tabs().contains(&PaneTab::Cart));
        state.show_cart(true);
        assert_eq!(state.second_pane(), Some(SecondPane::Cart));
        state.open_storage();
        assert_eq!(state.second_pane(), Some(SecondPane::Storage), "storage opens on storage");
        assert!(state.pane_tabs().contains(&PaneTab::Cart), "Storage / Cart tabs with a cart");
        state.show_cart(true);
        assert_eq!(state.second_pane(), Some(SecondPane::Cart), "the cart while storage stays open");
        state.show_cart(false);
        assert_eq!(state.second_pane(), Some(SecondPane::Storage));
        assert!(state.close_storage());
        assert!(!state.close_storage());
        assert!(!state.is_split(), "closing storage returns to the single view");
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

        // With storage open Alt+W swaps panes and never closes storage.
        state.open_storage();
        assert_eq!(cart_hotkey(&state, true), CartHotkey::ShowCart);
        state.show_cart(true);
        assert_eq!(cart_hotkey(&state, true), CartHotkey::ShowStorage);
    }

    #[test]
    fn a_new_session_forgets_the_old_cart_and_storage() {
        let mut state = UnifiedInventory::default();
        state.set_cart_equipped(true);
        state.show_cart(true);
        state.open_storage();
        state.reset_session();
        assert_eq!(state, UnifiedInventory {
            prefs: state.prefs.clone(),
            ..UnifiedInventory::default()
        });
        assert!(
            !state.pane_tabs().contains(&PaneTab::Cart),
            "a cartless character gets no Cart tab"
        );
        assert_eq!(cart_hotkey(&state, true), CartHotkey::NoCart);
        assert_eq!(state.second_pane(), None);
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
