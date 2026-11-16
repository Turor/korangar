//! Client startup update hook.
//!
//! [`drive_startup_update`] is the entry the binary calls from `main`. Tests
//! point `KORANGAR_UPDATE_MANIFEST_URL` at a fixture and pass a temp install.

use std::path::Path;

use korangar_update::{StartupOutcome, StartupRequest, UpdatePlatform};

/// Paths and key for one startup update check.
pub struct ClientStartup<'a> {
    pub install_root: &'a Path,
    pub current_exe: &'a Path,
    pub public_key: &'a [u8; 32],
    pub platform: UpdatePlatform,
    pub current_version: &'a str,
    pub relaunch_args: &'a [String],
    pub spawn_relaunch: bool,
}

/// Check `KORANGAR_UPDATE_MANIFEST_URL` unless `KORANGAR_SKIP_UPDATE=1`.
///
/// An unreachable channel returns [`StartupOutcome::Skipped`]. A manifest with
/// no file for `platform` does too, and does not change
/// `updates/applied.build`.
pub fn drive_startup_update(startup: &ClientStartup<'_>) -> Result<StartupOutcome, String> {
    let skip = std::env::var("KORANGAR_SKIP_UPDATE").ok();
    let manifest = std::env::var("KORANGAR_UPDATE_MANIFEST_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let default = korangar_update::DEFAULT_UPDATE_MANIFEST_URL;
    let manifest_url = manifest.as_deref().unwrap_or(default);
    korangar_update::run_startup_update(&StartupRequest {
        skip_env: skip.as_deref(),
        manifest_url: Some(manifest_url),
        current_version: startup.current_version,
        install_root: startup.install_root,
        current_exe: startup.current_exe,
        public_key: startup.public_key,
        relaunch_args: startup.relaunch_args,
        spawn_relaunch: startup.spawn_relaunch,
        platform: startup.platform,
    })
}
