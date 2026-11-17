//! Scriptable input and the no-swapchain logic session.
//!
//! See `docs/e2e-harness.md`. Live tests talk only to Hercules TEST and skip
//! when credentials or the server are missing.

mod actions;
pub mod archives;
mod chat;
mod companions;
mod credentials;
mod dressing_room;
mod grid_navigation;
mod headgear;
mod inject;
mod inventory_grid;
mod inventory_view;
mod item_flags;
mod item_link;
mod item_menu;
mod item_move;
mod item_tooltip;
mod item_workshop;
mod job_names;
mod logic;
mod mail;
mod minimap;
mod navigation_followups;
mod release;
mod resizable_windows;
mod screenshot;
mod skill_description;
mod social;
mod status_icons;
mod storage_tools;
mod weather;

pub use korangar_networking::{ItemCapacity, ItemCapacityList};

pub(crate) use self::actions::{apply_choose_dialog_option, apply_switch_window_tab, begin_npc_dialog, send_public_chat, send_walk};
pub use self::chat::{ChatSession, ChatView};
pub use self::companions::{CompanionNote, CompanionSession};
pub use self::credentials::{Credentials, LiveConnect, TEST_HOST, TEST_PORT};
pub use self::dressing_room::{LiveDressingReport, OfflineDressingReport, no_preview_message, offline_dressing_room};
pub use self::grid_navigation::{GridKey, KEY_RUN, OfflineGridNavReport, offline_grid_navigation};
pub use self::headgear::{HeadgearScene, LookedPlayer};
pub use self::inject::input_path_parity;
pub use self::inventory_grid::{GridView, OfflineInventoryGridReport, TURORAN_CAP_BYTES, grid_view, offline_inventory_grid};
pub use self::inventory_view::{
    DrawnIndexes, FAVORITE_ACK_BYTES, InventorySort, InventoryTab, OfflineInventoryViewReport, offline_inventory_view,
};
pub use self::item_flags::{OfflineFlagMenuReport, OfflineItemFlagsReport, offline_item_flag_menu, offline_item_flags};
pub use self::item_link::{OfflineItemLinkReport, offline_item_link};
pub use self::item_menu::{OfflineItemMenuReport, offline_item_menu};
pub use self::item_move::{ItemMoveCase, OfflineItemMoveReport, item_move_request, offline_item_moves};
pub use self::item_tooltip::{LiveItemTooltipReport, OfflineTooltipReport, offline_tooltip_view};
pub use self::item_workshop::WorkshopView;
pub use self::job_names::{JobNamesReport, job_names_scenario};
pub use self::logic::{
    DialogView, LiveGridNavReport, LiveInventoryGridReport, LiveInventoryViewReport, LiveItemMenuReport, LiveItemMoveReport, LiveLockedRow,
    LivePartyJobReport, LiveServerMarksReport, LiveStoragePlanReport, LogicSession, MailRoundTrip, OfflineServerMarksReport, Outbound,
    Phase, SelectedOption, SentChoice, SettingsView, offline_server_item_marks,
};
pub use self::mail::{OfflineMailReport, offline_mail_exchange};
pub use self::minimap::{MinimapHarness, MinimapMarkView, MinimapView};
pub use self::navigation_followups::{MapRouteReport, RewardDetailReport, map_route_scenario, reward_detail_scenario};
pub use self::release::{ClientStartup, drive_startup_update};
pub use self::resizable_windows::{
    GROW_DRAG, OfflineResizeReport, RESIZE_SCREEN, SHRINK_DRAG, WindowSnapshot, inventory_limits, offline_window_resize, scripted_drag,
    window_grid,
};
pub use self::screenshot::{
    Capture, capture_client_settings, capture_display, capture_lavapipe, lavapipe_icd_present, logic_mode_skips_image_assertions,
};
pub use self::skill_description::{
    BASH_SKILL_ID, DescriptionHost, SkillDescriptionReport, live_skill_description, offline_skill_description,
};
pub use self::social::{
    AchievementReport, NavigationReport, achievement_scenario, attendance_scenario, clan_chat_scenario, navigation_follow,
    navigation_scenario,
};
pub use self::status_icons::{StatusIconView, StatusStrip};
pub use self::storage_tools::{DEPOSIT_INTERVAL, MAX_SAVED_SEARCHES, OfflineStorageReport, SavedSearchFile, offline_storage_tools};
pub use self::weather::{MapWorld, WeatherKind};
pub use crate::interface::components::item_grid::GRID_FULL_CAP_LIMIT;
pub use crate::state::item_move::{ItemMovePrompt, ItemMoveStep, ItemMoveTarget};
