//! First-open HUD frames, scaled from a 1280x720 reference.
//!
//! The status panel is the upper left. Gear keeps its reference pixel size, so
//! a 44px socket stays 44px at 1024x576, and sits to the right of the status
//! panel when that still clears the minimap. Inventory, the character menu,
//! and standalone Stats stack in a column to the right of Gear when there is
//! room; on a 1024x576 screen they pack under the status panel in the left
//! gutter instead, with chat under that stack. Chat is otherwise the
//! bottom-left corner, below Gear. The hotbar is bottom-center. The minimap
//! is the top-right, with status under it. Quest, friends, party, and guild
//! stack below Stats (or below status, when the middle column is packed on
//! the left), clear of Gear. The skill tree opens in the center.

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

    #[allow(dead_code)]
    pub fn contains(self, inner: HudFrame) -> bool {
        inner.left >= self.left - 0.5
            && inner.top >= self.top - 0.5
            && inner.right() <= self.right() + 0.5
            && inner.bottom() <= self.bottom() + 0.5
    }
}

fn frames_overlap_x(a: HudFrame, b: HudFrame) -> bool {
    a.left < b.right() && a.right() > b.left
}

/// Windows placed the first time each class is opened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstOpenHud {
    pub status_panel: HudFrame,
    pub gear: HudFrame,
    pub inventory: HudFrame,
    pub character: HudFrame,
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

    // Unscaled. Scaling this frame is what drew ~36px sockets at 1024x576.
    let status_panel = HudFrame {
        left: margin,
        top: 36.0 * scale,
        width: 240.0,
        height: 248.0,
    };
    let mut gear = HudFrame {
        left: margin,
        top: 36.0 * scale,
        width: GEAR_WINDOW_WIDTH,
        height: GEAR_WINDOW_HEIGHT,
    };

    let minimap_side = (200.0 * scale).min(screen_width * 0.16);
    let minimap_width = minimap_side.max(220.0);
    let right_margin = margin.max(16.0);
    let minimap_left = screen_width - minimap_width - right_margin;
    let proposed_gear_left = status_panel.right() + margin;
    // Sit beside the always-on status panel when Gear still clears the minimap.
    if proposed_gear_left + GEAR_WINDOW_WIDTH <= minimap_left - margin {
        gear.left = proposed_gear_left;
    } else if status_panel.bottom() + margin + GEAR_WINDOW_HEIGHT <= screen_height - margin {
        gear.top = status_panel.bottom() + margin;
    }

    let mut chat = HudFrame {
        left: margin,
        top: screen_height - 160.0 * scale - margin,
        width: (340.0 * scale).min(gear.width),
        height: 160.0 * scale,
    };
    let mut hotbar = HudFrame {
        left: 0.0,
        top: screen_height - 48.0 * scale - margin,
        width: 420.0 * scale,
        height: 48.0 * scale,
    };
    hotbar.left = (screen_width - hotbar.width) / 2.0;

    let minimap = HudFrame {
        left: minimap_left,
        top: gear.top,
        width: minimap_width,
        height: minimap_side.max(220.0),
    };
    let status = HudFrame {
        left: minimap.left,
        top: minimap.bottom() + margin,
        width: minimap.width,
        height: 64.0 * scale,
    };

    let preferred_inventory_height = 120.0 * scale;
    let preferred_body_height = 150.0 * scale;
    let column_left_right = gear.right() + margin;
    let column_gap = minimap.left - margin - column_left_right;
    let pack_left_gutter = column_gap < 160.0;

    let (column_left, column_top, column_width, inventory_height, character_height, stats_height) = if pack_left_gutter {
        let top = status_panel.bottom() + margin;
        let width = status_panel.width;
        let chat_preferred = (160.0 * scale).max(80.0);
        let budget = (screen_height - margin - top).max(0.0);
        let available = (budget - margin * 3.0).max(24.1 * 4.0);
        let chat_height = chat_preferred.min(available * 0.3).max(32.0);
        let rest = (available - chat_height).max(24.1 * 3.0);
        let preferred_sum = preferred_inventory_height + preferred_body_height * 2.0;
        let (inv, body) = if preferred_sum <= rest {
            (preferred_inventory_height, preferred_body_height)
        } else {
            (
                (preferred_inventory_height / preferred_sum * rest).max(24.1),
                (preferred_body_height / preferred_sum * rest).max(24.1),
            )
        };
        (status_panel.left, top, width, inv, body, body)
    } else {
        (
            column_left_right,
            gear.top,
            column_gap,
            preferred_inventory_height,
            preferred_body_height,
            preferred_body_height,
        )
    };

    let inventory = HudFrame {
        left: column_left,
        top: column_top,
        width: column_width,
        height: inventory_height,
    };
    let character = HudFrame {
        left: column_left,
        top: inventory.bottom() + margin,
        width: column_width,
        height: character_height,
    };
    let stats = HudFrame {
        left: column_left,
        top: character.bottom() + margin,
        width: column_width,
        height: stats_height,
    };

    if pack_left_gutter {
        chat.left = status_panel.left;
        chat.width = status_panel.width;
        chat.top = stats.bottom() + margin;
        chat.height = (screen_height - margin - chat.top).max(32.0);
    } else if chat.top < gear.bottom() + margin {
        chat.top = gear.bottom() + margin;
        chat.height = screen_height - margin - chat.top;
    }
    if hotbar.left < chat.right() + margin {
        hotbar.left = chat.right() + margin;
        hotbar.width = (screen_width - margin - hotbar.left).min(hotbar.width);
    }
    if hotbar.top < chat.top && frames_overlap_x(hotbar, chat) {
        hotbar.top = chat.top;
        hotbar.height = (screen_height - margin - hotbar.top).max(32.0);
    }

    let social_width = (420.0 * scale).min(screen_width - gear.right() - margin * 2.0);
    let social_left = screen_width - social_width - margin;
    // When the middle column is packed under the status panel, social stays in
    // the right stack under the minimap status strip.
    let social_top = if pack_left_gutter {
        status.bottom() + margin
    } else {
        stats.bottom().max(status.bottom()) + margin
    };
    let social_bottom = hotbar.top - margin;
    let social_gap = margin;
    let social_height = ((social_bottom - social_top) - social_gap * 3.0) / 4.0;
    let social = |index: f32| HudFrame {
        left: social_left,
        top: social_top + index * (social_height + social_gap),
        width: social_width,
        height: social_height,
    };

    let skill_tree = HudFrame {
        left: (screen_width - 560.0 * scale) / 2.0,
        top: (screen_height - 400.0 * scale) / 2.0,
        width: 560.0 * scale,
        height: 400.0 * scale,
    };

    FirstOpenHud {
        status_panel,
        gear,
        inventory,
        character,
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
            hud.status_panel,
            hud.gear,
            hud.inventory,
            hud.character,
            hud.stats,
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
        on_screen(hud.skill_tree, width, height);

        for (index, frame) in frames.iter().copied().enumerate() {
            for other in frames.iter().copied().skip(index + 1) {
                assert!(!frames_overlap(frame, other), "overlap {frame:?} {other:?}");
            }
        }

        assert!(!frames_overlap(hud.gear, hud.stats), "standalone stats sit on Gear");
        assert!(!frames_overlap(hud.gear, hud.character), "character menu sits on Gear");
        assert!(!frames_overlap(hud.chat, hud.gear), "chat overlaps gear");
        assert!(hud.status_panel.left <= hud.gear.left + 0.5);
        assert!((hud.status_panel.top - hud.gear.top).abs() < 0.5 || hud.gear.top >= hud.status_panel.bottom() - 0.5);
        assert!((hud.gear.width - super::GEAR_WINDOW_WIDTH).abs() < 0.5);
        assert!((hud.gear.height - super::GEAR_WINDOW_HEIGHT).abs() < 0.5);
        assert!(hud.quest.left >= hud.gear.right() - 0.5);
        assert!(hud.hotbar.left + hud.hotbar.width / 2.0 - width / 2.0 < 1.0);
        let skill_center_x = hud.skill_tree.left + hud.skill_tree.width / 2.0;
        let skill_center_y = hud.skill_tree.top + hud.skill_tree.height / 2.0;
        assert!((skill_center_x - width / 2.0).abs() < 1.0);
        assert!((skill_center_y - height / 2.0).abs() < 1.0);
        assert!(hud.minimap.right() <= width);
        assert!(hud.minimap.width >= 220.0);
        assert!(width - hud.minimap.right() >= 16.0 - 0.5, "minimap close button clipped");
        assert!(hud.status.top >= hud.minimap.bottom() - 0.5);
        assert!(hud.friends.top >= hud.quest.bottom() - 0.5);
        assert!(hud.party.top >= hud.friends.bottom() - 0.5);
        assert!(hud.guild.top >= hud.party.bottom() - 0.5);
    }

    #[test]
    fn first_open_frames_fit_1024_1280_and_1920() {
        assert_layout(1024.0, 576.0);
        assert_layout(REFERENCE_SCREEN_WIDTH, REFERENCE_SCREEN_HEIGHT);
        assert_layout(1920.0, 1080.0);
        let small = first_open_hud(1024.0, 576.0);
        let mid = first_open_hud(REFERENCE_SCREEN_WIDTH, REFERENCE_SCREEN_HEIGHT);
        let large = first_open_hud(1920.0, 1080.0);
        assert!((small.gear.width - mid.gear.width).abs() < 0.5);
        assert!((large.gear.width - mid.gear.width).abs() < 0.5);
        assert!((small.gear.height - super::GEAR_WINDOW_HEIGHT).abs() < 0.5);
        let wide = first_open_hud(1280.0, 720.0);
        assert!(
            !frames_overlap(wide.status_panel, wide.gear),
            "status panel overlaps Gear at 1280"
        );
        assert!(!frames_overlap(wide.status_panel, wide.chat), "status panel overlaps chat");
        let compact = first_open_hud(1024.0, 576.0);
        assert!(compact.status_panel.left >= 0.0);
        assert!(compact.status_panel.right() <= 1024.0);
        assert!(
            !frames_overlap(compact.status_panel, compact.chat),
            "status panel overlaps chat at 1024"
        );

        assert!(large.inventory.left > mid.inventory.left);
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
