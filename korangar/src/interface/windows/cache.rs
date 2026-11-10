use std::collections::HashMap;

#[cfg(feature = "debug")]
use korangar_debug::logging::{Colorize, print_debug};
use korangar_interface::window::{Anchor, AnchorPoint};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use super::WindowClass;
use super::placement::{HudFrame, REFERENCE_SCREEN_HEIGHT, REFERENCE_SCREEN_WIDTH, ScreenAnchor, first_open_hud, frame_anchor_offset};
use crate::graphics::{ScreenPosition, ScreenSize};
use crate::state::ClientState;

#[derive(Serialize, Deserialize)]
pub struct WindowState {
    pub anchor: Anchor<ClientState>,
    pub size: ScreenSize,
}

impl WindowState {
    pub fn new(anchor: Anchor<ClientState>, size: ScreenSize) -> Self {
        Self { anchor, size }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct WindowCache {
    entries: HashMap<WindowClass, WindowState>,
}

impl WindowCache {
    // Since `WindowClass` has some variants with debug features enabled, we use a
    // differen file to store the window cache. This avoids failing to load and
    // thereby wiping the previous window cache when switching between debug and
    // non-debug builds.
    #[cfg(not(feature = "debug"))]
    const FILE_NAME: &'static str = "client/window_cache.ron";
    #[cfg(feature = "debug")]
    const FILE_NAME: &'static str = "client/window_cache_debug.ron";

    fn load() -> Option<Self> {
        #[cfg(feature = "debug")]
        print_debug!("loading window cache from {}", Self::FILE_NAME.magenta());

        std::fs::read_to_string(Self::FILE_NAME)
            .ok()
            .and_then(|data| ron::from_str(&data).ok())
            .map(|entries| Self { entries })
    }

    fn save(&self) {
        #[cfg(feature = "debug")]
        print_debug!("saving window cache to {}", Self::FILE_NAME.magenta());

        let data = ron::ser::to_string_pretty(&self.entries, PrettyConfig::new()).unwrap();
        std::fs::write(Self::FILE_NAME, data).expect("unable to write file");
    }
}

impl korangar_interface::application::WindowCache<ClientState> for WindowCache {
    fn create() -> Self {
        Self::load().unwrap_or_else(|| {
            #[cfg(feature = "debug")]
            print_debug!(
                "failed to load window cache from {}. creating empty cache",
                Self::FILE_NAME.magenta()
            );

            Default::default()
        })
    }

    fn get_window_state(&self, class: WindowClass, screen: ScreenSize) -> Option<(Anchor<ClientState>, ScreenSize)> {
        resolve_window_state(self.entries.get(&class), class, screen)
    }

    fn register_window(&mut self, class: WindowClass, anchor: Anchor<ClientState>, size: ScreenSize) {
        if let Some(entry) = self.entries.get_mut(&class) {
            entry.anchor = anchor;
            entry.size = size;
        } else {
            let entry = WindowState::new(anchor, size);
            self.entries.insert(class, entry);
        }
    }

    fn update_anchor(&mut self, class: WindowClass, anchor: Anchor<ClientState>) {
        if let Some(entry) = self.entries.get_mut(&class) {
            entry.anchor = anchor;
        } else if let Some((_, size)) = preset_window(class, REFERENCE_SCREEN_WIDTH, REFERENCE_SCREEN_HEIGHT) {
            self.entries.insert(class, WindowState::new(anchor, size));
        }
    }

    fn update_size(&mut self, class: WindowClass, size: ScreenSize) {
        if let Some(entry) = self.entries.get_mut(&class) {
            entry.size = size;
        } else if let Some((anchor, _)) = preset_window(class, REFERENCE_SCREEN_WIDTH, REFERENCE_SCREEN_HEIGHT) {
            self.entries.insert(class, WindowState::new(anchor, size));
        }
    }
}

/// Saved anchor wins. With no save, [`preset_window`] supplies the default.
fn resolve_window_state(saved: Option<&WindowState>, class: WindowClass, screen: ScreenSize) -> Option<(Anchor<ClientState>, ScreenSize)> {
    if let Some(entry) = saved {
        return Some((entry.anchor, entry.size));
    }
    preset_window(class, screen.width, screen.height)
}

/// Anchor and size for a window that has never been moved.
///
/// Offsets come from [`first_open_hud`] at the live client size.
fn preset_window(class: WindowClass, screen_width: f32, screen_height: f32) -> Option<(Anchor<ClientState>, ScreenSize)> {
    if class == WindowClass::MenuBar {
        return Some(super::menu_bar::default_menu_bar_window(screen_width, screen_height));
    }
    let hud = first_open_hud(screen_width, screen_height);
    let (anchor_kind, frame) = match class {
        WindowClass::Equipment => (ScreenAnchor::TopLeft, hud.gear),
        WindowClass::Inventory => (ScreenAnchor::TopLeft, hud.inventory),
        WindowClass::CharacterOverview => (ScreenAnchor::TopLeft, hud.character),
        WindowClass::Stats => (ScreenAnchor::TopLeft, hud.stats),
        WindowClass::Chat => (ScreenAnchor::BottomLeft, hud.chat),
        WindowClass::Hotbar => (ScreenAnchor::BottomCenter, hud.hotbar),
        WindowClass::MvpMinimap => (ScreenAnchor::TopRight, hud.minimap),
        WindowClass::StatusEffects => (ScreenAnchor::TopRight, hud.status),
        WindowClass::StatusPanel => (ScreenAnchor::TopLeft, hud.status_panel),
        WindowClass::SkillTree => (ScreenAnchor::Center, hud.skill_tree),
        WindowClass::QuestJournal => (ScreenAnchor::TopRight, hud.quest),
        WindowClass::FriendList => (ScreenAnchor::TopRight, hud.friends),
        WindowClass::PartyInvite => (ScreenAnchor::TopRight, hud.party),
        // The social slot for guild sits on the hotbar. A create form or roster
        // does not fit there, so open guild windows in the upper middle.
        WindowClass::Guild | WindowClass::GuildInvite => {
            let width = 480.0_f32.min(screen_width - 32.0);
            let height = (screen_height - 160.0).clamp(200.0, 520.0);
            (ScreenAnchor::TopLeft, HudFrame {
                left: (screen_width - width) / 2.0,
                top: 64.0,
                width,
                height,
            })
        }
        _ => return None,
    };
    let (left, top) = frame_anchor_offset(anchor_kind, frame, screen_width, screen_height);
    let anchor_point = match anchor_kind {
        ScreenAnchor::TopLeft => AnchorPoint::TopLeft,
        ScreenAnchor::TopRight => AnchorPoint::TopRight,
        ScreenAnchor::BottomLeft => AnchorPoint::BottomLeft,
        ScreenAnchor::BottomCenter => AnchorPoint::BottomCenter,
        ScreenAnchor::Center => AnchorPoint::Center,
    };
    Some((Anchor::at(anchor_point, ScreenPosition { left, top }), ScreenSize {
        width: frame.width,
        height: frame.height,
    }))
}

impl Drop for WindowCache {
    fn drop(&mut self) {
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::{WindowState, resolve_window_state};
    use crate::graphics::{ScreenPosition, ScreenSize};
    use crate::interface::windows::WindowClass;

    #[test]
    fn menu_bar_position_persists_like_other_windows() {
        let screen = ScreenSize {
            width: 1280.0,
            height: 720.0,
        };
        let (anchor, size) = resolve_window_state(None, WindowClass::MenuBar, screen).expect("menu bar preset");
        let start = anchor.to_position(screen);
        let expected = super::super::menu_bar::default_menu_bar_frame(screen.width, screen.height);
        assert!((start.left - expected.left).abs() < 0.5);
        assert!((start.top - expected.top).abs() < 0.5);
        assert!((size.width - expected.width).abs() < 0.5);
        assert!((size.height - expected.height).abs() < 0.5);
        assert!(
            screen.height - (start.top + size.height) > 40.0,
            "default bar still covers the bottom HUD"
        );

        let mut moved = anchor;
        let dragged = ScreenPosition { left: 40.0, top: 90.0 };
        moved.update(screen, dragged, size);
        let saved = WindowState::new(moved, size);
        let (again, again_size) = resolve_window_state(Some(&saved), WindowClass::MenuBar, screen).expect("saved menu bar");
        let remembered = again.to_position(screen);
        assert!((remembered.left - dragged.left).abs() < 0.5);
        assert!((remembered.top - dragged.top).abs() < 0.5);
        assert_eq!(again_size.width, size.width);
        assert_eq!(again_size.height, size.height);
    }
}
