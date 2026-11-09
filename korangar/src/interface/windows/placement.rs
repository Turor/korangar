//! First-open HUD frames for a 1280×720 client.
//!
//! Gear sits on the upper left. Chat is the bottom-left corner. The hotbar is
//! bottom-center. Inventory is a column inside Gear, so it does not open as a
//! second window on top of these frames.

/// One window rectangle in screen pixels. Origin is the top-left of the client.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudFrame {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

/// Gear, chat, and hotbar the first time each window is opened.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstOpenHud {
    pub gear: HudFrame,
    pub chat: HudFrame,
    pub hotbar: HudFrame,
}

pub const REFERENCE_SCREEN_WIDTH: f32 = 1280.0;
pub const REFERENCE_SCREEN_HEIGHT: f32 = 720.0;

/// Frames used when a window class has no saved position yet.
pub fn first_open_hud(screen_width: f32, screen_height: f32) -> FirstOpenHud {
    let hotbar_width = 420.0;
    let hotbar_height = 48.0;
    let chat_width = 340.0;
    let chat_height = 160.0;

    FirstOpenHud {
        gear: HudFrame {
            left: 16.0,
            top: 36.0,
            width: 1040.0,
            height: 500.0,
        },
        chat: HudFrame {
            left: 8.0,
            top: screen_height - chat_height - 8.0,
            width: chat_width,
            height: chat_height,
        },
        hotbar: HudFrame {
            left: (screen_width - hotbar_width) / 2.0,
            top: screen_height - hotbar_height - 8.0,
            width: hotbar_width,
            height: hotbar_height,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{REFERENCE_SCREEN_HEIGHT, REFERENCE_SCREEN_WIDTH, HudFrame, first_open_hud};

    fn frames_overlap(a: HudFrame, b: HudFrame) -> bool {
        a.left < b.left + b.width && a.left + a.width > b.left && a.top < b.top + b.height && a.top + a.height > b.top
    }

    #[test]
    fn first_open_frames_do_not_overlap_at_1280x720() {
        let hud = first_open_hud(REFERENCE_SCREEN_WIDTH, REFERENCE_SCREEN_HEIGHT);
        assert!(!frames_overlap(hud.gear, hud.chat));
        assert!(!frames_overlap(hud.gear, hud.hotbar));
        assert!(!frames_overlap(hud.chat, hud.hotbar));

        let right = hud.gear.left + hud.gear.width;
        let bottom = hud.gear.top + hud.gear.height;
        assert!(right <= REFERENCE_SCREEN_WIDTH);
        assert!(bottom <= REFERENCE_SCREEN_HEIGHT);
        assert!(hud.chat.left + hud.chat.width < hud.hotbar.left);
        assert!(hud.hotbar.left + hud.hotbar.width / 2.0 == REFERENCE_SCREEN_WIDTH / 2.0);
        assert!(hud.chat.top + hud.chat.height <= REFERENCE_SCREEN_HEIGHT);
        assert!(hud.hotbar.top > hud.gear.top + hud.gear.height);
    }
}
