//! Scriptable input and the no-swapchain logic session.
//!
//! See `docs/e2e-harness.md`. Live tests talk only to Hercules TEST and skip
//! when credentials or the server are missing.

mod actions;
pub mod archives;
mod chat;
mod credentials;
mod inject;
mod logic;
mod release;
mod screenshot;
mod social;

pub(crate) use self::actions::{apply_choose_dialog_option, apply_switch_window_tab, begin_npc_dialog, send_public_chat, send_walk};
pub use self::chat::{ChatSession, ChatView};
pub use self::credentials::{Credentials, LiveConnect, TEST_HOST, TEST_PORT};
pub use self::inject::input_path_parity;
pub use self::logic::{DialogView, LogicSession, Outbound, Phase, SelectedOption, SentChoice, SettingsView};
pub use self::release::{ClientStartup, drive_startup_update};
pub use self::screenshot::{
    Capture, capture_client_settings, capture_display, capture_lavapipe, lavapipe_icd_present, logic_mode_skips_image_assertions,
};
pub use self::social::{
    AchievementReport, NavigationReport, achievement_scenario, attendance_scenario, clan_chat_scenario, navigation_follow,
    navigation_scenario,
};
