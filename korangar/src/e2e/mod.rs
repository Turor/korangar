//! Scriptable input and the no-swapchain logic session.
//!
//! See `docs/e2e-harness.md`. Live tests talk only to Hercules TEST and skip
//! when credentials or the server are missing.

mod credentials;
mod inject;
mod logic;
mod screenshot;

pub use self::credentials::{Credentials, LiveConnect, TEST_HOST, TEST_PORT};
pub use self::inject::input_path_parity;
pub use self::logic::{DialogView, LogicSession, Outbound, Phase, SelectedOption, SettingsView};
pub use self::screenshot::{Capture, capture_display, capture_lavapipe, lavapipe_icd_present, logic_mode_skips_image_assertions};
