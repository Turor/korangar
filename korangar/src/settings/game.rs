#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, print_debug};
use korangar_interface::element::StateElement;
use ron::ser::PrettyConfig;
use rust_state::RustState;
use serde::{Deserialize, Serialize};

fn default_enable_signboards() -> bool {
    true
}

fn default_dialog_chars_per_second() -> f32 {
    40.0
}

fn default_dialog_portrait_dim() -> f32 {
    0.45
}

fn default_show_equipment() -> bool {
    true
}

#[derive(Clone, Serialize, Deserialize, RustState, StateElement)]
pub struct GameSettings {
    pub auto_attack: bool,
    /// When false, signboard and town-info rows are not loaded.
    /// Missing settings default to on. `KORANGAR_ENABLE_SIGNBOARDS` still
    /// overrides this.
    #[serde(default = "default_enable_signboards")]
    pub enable_signboards: bool,

    /// Characters revealed per second in the NPC dialog. `^RRGGBB` codes are
    /// not counted.
    #[serde(default = "default_dialog_chars_per_second")]
    pub dialog_chars_per_second: f32,
    /// Opacity of the portrait that is not speaking. `1` leaves both sides
    /// fully lit. The tint stays the sprite color; only alpha changes.
    #[serde(default = "default_dialog_portrait_dim")]
    pub dialog_portrait_dim: f32,
    /// Let other players open this character's equipment window.
    /// The server echo (`0x02DA` / `0x02D9`) overwrites the saved value.
    #[serde(default = "default_show_equipment")]
    pub show_equipment: bool,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            auto_attack: true,
            enable_signboards: default_enable_signboards(),
            dialog_chars_per_second: default_dialog_chars_per_second(),
            dialog_portrait_dim: default_dialog_portrait_dim(),
            show_equipment: default_show_equipment(),
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
            #[serde(default = "default_enable_signboards")]
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
