//! First-open HUD frames, scaled from a 1280×720 reference.
//!
//! Gear is the upper left. Inventory is its own window, immediately to the
//! right of Gear. Stats sit inside Gear's right half. Chat is the bottom-left
//! corner. The hotbar is bottom-center. The minimap is the top-right, with
//! status under it. Quest, friends, party, and guild stack on the right, clear
//! of Gear. The skill tree opens in the center of the screen.

use super::equipment::{GEAR_WINDOW_HEIGHT, GEAR_WINDOW_WIDTH};

/// One window rectangle in screen pixels. Origin is the top-left of the client.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudFrame {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

impl HudFrame {
    pub fn right(self) -> f32 {
        self.left + self.width
    }

    pub fn bottom(self) -> f32 {
        self.top + self.height
    }

    pub fn contains(self, inner: HudFrame) -> bool {
        inner.left >= self.left - 0.5
            && inner.top >= self.top - 0.5
            && inner.right() <= self.right() + 0.5
            && inner.bottom() <= self.bottom() + 0.5
    }
}

/// Windows placed the first time each class is opened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstOpenHud {
    pub gear: HudFrame,
    pub inventory: HudFrame,
    pub stats: HudFrame,
    pub chat: HudFrame,
    pub hotbar: HudFrame,
    pub minimap: HudFrame,
    pub status: HudFrame,
    pub skill_tree: HudFrame,
    pub quest: HudFrame,
    pub friends: HudFrame,
    pub party: HudFrame,
    pub guild: HudFrame,
}

pub const REFERENCE_SCREEN_WIDTH: f32 = 1280.0;
pub const REFERENCE_SCREEN_HEIGHT: f32 = 720.0;

/// Which screen edge a saved-less window hangs from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomCenter,
    Center,
}

/// Offset from [`ScreenAnchor`] that reproduces `frame` on a screen of this
/// size.
pub fn frame_anchor_offset(anchor: ScreenAnchor, frame: HudFrame, screen_width: f32, screen_height: f32) -> (f32, f32) {
    match anchor {
        ScreenAnchor::TopLeft => (frame.left, frame.top),
        ScreenAnchor::TopRight => (frame.left - screen_width, frame.top),
        ScreenAnchor::BottomLeft => (frame.left, frame.top - screen_height),
        ScreenAnchor::BottomCenter => (frame.left - screen_width / 2.0, frame.top - screen_height),
        ScreenAnchor::Center => (frame.left - screen_width / 2.0, frame.top - screen_height / 2.0),
    }
}

/// Frames used when a window class has no saved position yet.
pub fn first_open_hud(screen_width: f32, screen_height: f32) -> FirstOpenHud {
    let scale = (screen_width / REFERENCE_SCREEN_WIDTH).min(screen_height / REFERENCE_SCREEN_HEIGHT);
    let margin = 8.0 * scale;

    let chat = HudFrame {
        left: margin,
        top: screen_height - 160.0 * scale - margin,
        width: 340.0 * scale,
        height: 160.0 * scale,
    };
    let mut hotbar = HudFrame {
        left: 0.0,
        top: screen_height - 48.0 * scale - margin,
        width: 420.0 * scale,
        height: 48.0 * scale,
    };
    hotbar.left = (screen_width - hotbar.width) / 2.0;
    if hotbar.left < chat.right() + margin {
        hotbar.left = chat.right() + margin;
        hotbar.width = (screen_width - margin - hotbar.left).min(hotbar.width);
    }

    let gear = HudFrame {
        left: margin,
        top: 36.0 * scale,
        width: (GEAR_WINDOW_WIDTH * scale).min(screen_width * 0.50).min(880.0),
        height: (GEAR_WINDOW_HEIGHT * scale).min(chat.top - 36.0 * scale - margin),
    };

    let minimap_side = (200.0 * scale).min(screen_width * 0.16);
    let minimap = HudFrame {
        left: screen_width - minimap_side - margin,
        top: gear.top,
        width: minimap_side,
        height: minimap_side,
    };
    let inventory = HudFrame {
        left: gear.right() + margin,
        top: gear.top,
        width: (440.0 * scale).min(minimap.left - margin - (gear.right() + margin)).max(160.0),
        height: 180.0 * scale,
    };
    let status = HudFrame {
        left: minimap.left,
        top: minimap.bottom() + margin,
        width: minimap.width,
        height: 64.0 * scale,
    };

    let social_width = (420.0 * scale).min(screen_width - gear.right() - margin * 2.0);
    let social_left = screen_width - social_width - margin;
    let social_top = inventory.bottom().max(status.bottom()) + margin;
    let social_bottom = hotbar.top - margin;
    let social_gap = margin;
    let social_height = ((social_bottom - social_top) - social_gap * 3.0) / 4.0;
    let social = |index: f32| HudFrame {
        left: social_left,
        top: social_top + index * (social_height + social_gap),
        width: social_width,
        height: social_height,
    };

    let stats = HudFrame {
        left: gear.left + gear.width * 0.52,
        top: gear.top + 28.0 * scale,
        width: gear.width * 0.42,
        height: gear.height * 0.70,
    };

    let _stats_inside_gear = gear.contains(stats);

    let skill_tree = HudFrame {
        left: (screen_width - 560.0 * scale) / 2.0,
        top: (screen_height - 400.0 * scale) / 2.0,
        width: 560.0 * scale,
        height: 400.0 * scale,
    };

    FirstOpenHud {
        gear,
        inventory,
        stats,
        chat,
        hotbar,
        minimap,
        status,
        skill_tree,
        quest: social(0.0),
        friends: social(1.0),
        party: social(2.0),
        guild: social(3.0),
    }
}

#[cfg(test)]
mod tests {
    use super::{HudFrame, REFERENCE_SCREEN_HEIGHT, REFERENCE_SCREEN_WIDTH, ScreenAnchor, first_open_hud, frame_anchor_offset};

    fn frames_overlap(a: HudFrame, b: HudFrame) -> bool {
        a.left < b.right() && a.right() > b.left && a.top < b.bottom() && a.bottom() > b.top
    }

    fn on_screen(frame: HudFrame, width: f32, height: f32) {
        assert!(frame.left >= -0.5, "left {}", frame.left);
        assert!(frame.top >= -0.5, "top {}", frame.top);
        assert!(frame.right() <= width + 0.5, "right {}", frame.right());
        assert!(frame.bottom() <= height + 0.5, "bottom {}", frame.bottom());
        assert!(frame.width > 40.0 && frame.height > 24.0);
    }

    fn assert_layout(width: f32, height: f32) {
        let hud = first_open_hud(width, height);
        let frames = [
            hud.gear,
            hud.inventory,
            hud.chat,
            hud.hotbar,
            hud.minimap,
            hud.status,
            hud.quest,
            hud.friends,
            hud.party,
            hud.guild,
        ];
        for frame in frames {
            on_screen(frame, width, height);
        }
        on_screen(hud.stats, width, height);
        on_screen(hud.skill_tree, width, height);

        for (index, frame) in frames.iter().copied().enumerate() {
            for other in frames.iter().copied().skip(index + 1) {
                assert!(!frames_overlap(frame, other), "overlap {frame:?} {other:?}");
            }
        }

        assert!(hud.gear.contains(hud.stats), "stats belong in Gear's right half");
        assert!(hud.stats.left >= hud.gear.left + hud.gear.width * 0.5);
        assert!(hud.inventory.left >= hud.gear.right() - 0.5);
        assert!(hud.quest.left >= hud.gear.right() - 0.5);
        assert!(hud.hotbar.left + hud.hotbar.width / 2.0 - width / 2.0 < 1.0);
        let skill_center_x = hud.skill_tree.left + hud.skill_tree.width / 2.0;
        let skill_center_y = hud.skill_tree.top + hud.skill_tree.height / 2.0;
        assert!((skill_center_x - width / 2.0).abs() < 1.0);
        assert!((skill_center_y - height / 2.0).abs() < 1.0);
        assert!(hud.minimap.right() <= width);
        assert!(hud.status.top >= hud.minimap.bottom() - 0.5);
        assert!(hud.friends.top >= hud.quest.bottom() - 0.5);
        assert!(hud.party.top >= hud.friends.bottom() - 0.5);
        assert!(hud.guild.top >= hud.party.bottom() - 0.5);
    }

    #[test]
    fn first_open_frames_fit_1280x720_and_1920x1080() {
        assert_layout(REFERENCE_SCREEN_WIDTH, REFERENCE_SCREEN_HEIGHT);
        assert_layout(1920.0, 1080.0);
        let small = first_open_hud(REFERENCE_SCREEN_WIDTH, REFERENCE_SCREEN_HEIGHT);
        let large = first_open_hud(1920.0, 1080.0);
        assert!(large.gear.width > small.gear.width);
        assert!(large.inventory.left > small.inventory.left);
    }

    #[test]
    fn anchor_offsets_round_trip() {
        let hud = first_open_hud(REFERENCE_SCREEN_WIDTH, REFERENCE_SCREEN_HEIGHT);
        let cases = [
            (ScreenAnchor::TopLeft, hud.gear),
            (ScreenAnchor::TopLeft, hud.inventory),
            (ScreenAnchor::BottomLeft, hud.chat),
            (ScreenAnchor::BottomCenter, hud.hotbar),
            (ScreenAnchor::TopRight, hud.minimap),
            (ScreenAnchor::Center, hud.skill_tree),
        ];
        for (anchor, frame) in cases {
            let (left, top) = frame_anchor_offset(anchor, frame, REFERENCE_SCREEN_WIDTH, REFERENCE_SCREEN_HEIGHT);
            let restored = match anchor {
                ScreenAnchor::TopLeft => HudFrame { left, top, ..frame },
                ScreenAnchor::TopRight => HudFrame {
                    left: REFERENCE_SCREEN_WIDTH + left,
                    top,
                    ..frame
                },
                ScreenAnchor::BottomLeft => HudFrame {
                    left,
                    top: REFERENCE_SCREEN_HEIGHT + top,
                    ..frame
                },
                ScreenAnchor::BottomCenter => HudFrame {
                    left: REFERENCE_SCREEN_WIDTH / 2.0 + left,
                    top: REFERENCE_SCREEN_HEIGHT + top,
                    ..frame
                },
                ScreenAnchor::Center => HudFrame {
                    left: REFERENCE_SCREEN_WIDTH / 2.0 + left,
                    top: REFERENCE_SCREEN_HEIGHT / 2.0 + top,
                    ..frame
                },
            };
            assert!((restored.left - frame.left).abs() < 0.01);
            assert!((restored.top - frame.top).abs() < 0.01);
        }
    }
}
