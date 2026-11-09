#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, print_debug};
use korangar_interface::element::StateElement;
use ron::ser::PrettyConfig;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, RustState, StateElement)]
pub struct GameSettings {
    pub auto_attack: bool,
    /// When false, signboard and town-info rows are not loaded.
    #[serde(default)]
    pub enable_signboards: bool,
    /// Cash shop requests stay off until the player turns this on.
    #[serde(default)]
    pub enable_cash_shop: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            auto_attack: true,
            enable_signboards: false,
            enable_cash_shop: false,
        }
    }
}

impl GameSettings {
    const FILE_NAME: &'static str = "client/game_settings.ron";

    pub fn new() -> Self {
        Self::load().unwrap_or_else(|| {
            #[cfg(feature = "debug")]
            print_debug!("failed to load game settings from {}", Self::FILE_NAME.magenta());
            Default::default()
        })
    }

    /// Reads `enable_signboards` without building a [`GameSettings`] value.
    /// That type saves `client/game_settings.ron` when it is dropped.
    pub fn read_enable_signboards() -> Option<bool> {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            enable_signboards: bool,
        }

        std::fs::read_to_string(Self::FILE_NAME)
            .ok()
            .and_then(|data| ron::from_str::<Probe>(&data).ok())
            .map(|probe| probe.enable_signboards)
    }

    pub fn load() -> Option<Self> {
        #[cfg(feature = "debug")]
        print_debug!("loading game settings from {}", Self::FILE_NAME.magenta());
        std::fs::read_to_string(Self::FILE_NAME)
            .ok()
            .and_then(|data| ron::from_str(&data).ok())
    }

    pub fn save(&self) {
        #[cfg(feature = "debug")]
        print_debug!("saving game settings to {}", Self::FILE_NAME.magenta());

        let data = ron::ser::to_string_pretty(self, PrettyConfig::new()).unwrap();

        if let Err(_error) = std::fs::write(Self::FILE_NAME, data) {
            #[cfg(feature = "debug")]
            print_debug!(
                "failed to save game settings to {}: {:?}",
                Self::FILE_NAME.magenta(),
                _error.red()
            );
        }
    }
}

impl Drop for GameSettings {
    fn drop(&mut self) {
        self.save();
    }
}
