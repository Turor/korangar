//! Scriptable input and the no-swapchain logic session.
//!
//! See `docs/e2e-harness.md`. Live tests talk only to Hercules TEST and skip
//! when credentials or the server are missing.

mod actions;
pub mod archives;
mod chat;
mod companions;
mod credentials;
mod headgear;
mod inject;
mod item_move;
mod item_workshop;
mod job_names;
mod logic;
mod mail;
mod minimap;
mod navigation_followups;
mod release;
mod screenshot;
mod social;
mod status_icons;
mod weather;

pub(crate) use self::actions::{apply_choose_dialog_option, apply_switch_window_tab, begin_npc_dialog, send_public_chat, send_walk};
pub use self::chat::{ChatSession, ChatView};
pub use self::companions::{CompanionNote, CompanionSession};
pub use self::credentials::{Credentials, LiveConnect, TEST_HOST, TEST_PORT};
pub use self::headgear::{HeadgearScene, LookedPlayer};
pub use self::inject::input_path_parity;
pub use self::item_move::{ItemMoveCase, OfflineItemMoveReport, item_move_request, offline_item_moves};
pub use self::item_workshop::WorkshopView;
pub use self::job_names::{JobNamesReport, job_names_scenario};
pub use self::logic::{
    DialogView, LiveItemMoveReport, LivePartyJobReport, LogicSession, MailRoundTrip, Outbound, Phase, SelectedOption, SentChoice,
    SettingsView,
};
pub use self::mail::{OfflineMailReport, offline_mail_exchange};
pub use self::minimap::{MinimapHarness, MinimapMarkView, MinimapView};
pub use self::navigation_followups::{MapRouteReport, RewardDetailReport, map_route_scenario, reward_detail_scenario};
pub use self::release::{ClientStartup, drive_startup_update};
pub use self::screenshot::{
    Capture, capture_client_settings, capture_display, capture_lavapipe, lavapipe_icd_present, logic_mode_skips_image_assertions,
};
pub use self::social::{
    AchievementReport, NavigationReport, achievement_scenario, attendance_scenario, clan_chat_scenario, navigation_follow,
    navigation_scenario,
};
pub use self::status_icons::{StatusIconView, StatusStrip};
pub use self::weather::{MapWorld, WeatherKind};
pub use crate::state::item_move::{ItemMovePrompt, ItemMoveStep, ItemMoveTarget};
