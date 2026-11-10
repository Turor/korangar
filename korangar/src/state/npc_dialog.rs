//! Cutin and progress-bar state for an NPC script.
//!
//! The fields stay primitive so they can live on
//! [`ClientState`](super::ClientState). Packet bytes and the input reply live
//! in `korangar-networking`.

use korangar_interface::element::StateElement;
use korangar_networking::{progress_complete, progress_fraction};
use rust_state::RustState;

use crate::graphics::{Color, ScreenPosition, ScreenSize};

/// Where a cutin sits, and the progress bar drawn over the player.
#[derive(Default, RustState, StateElement)]
pub struct NpcPresentation {
    cutin_path: String,
    cutin_visible: bool,
    /// 0 bottom-left, 1 bottom-center, 2 bottom-right, 3 centered.
    cutin_anchor: u8,
    progress_color: u32,
    progress_duration_ms: u32,
    progress_started_ms: u32,
    progress_active: bool,
    npc_bgm: String,
    npc_bgm_active: bool,
}

impl NpcPresentation {
    pub fn show_cutin(&mut self, anchor: u8, relative_path: String) {
        self.cutin_anchor = anchor;
        self.cutin_path = relative_path;
        self.cutin_visible = true;
    }

    pub fn clear_cutin(&mut self) {
        self.cutin_visible = false;
        self.cutin_path.clear();
    }

    pub fn cutin(&self) -> Option<(&str, u8)> {
        if self.cutin_visible && !self.cutin_path.is_empty() {
            Some((self.cutin_path.as_str(), self.cutin_anchor))
        } else {
            None
        }
    }

    pub fn start_progress(&mut self, color: u32, seconds: u32, now_ms: u32) {
        self.progress_color = color;
        self.progress_duration_ms = seconds.saturating_mul(1000);
        self.progress_started_ms = now_ms;
        self.progress_active = true;
    }

    /// `true` when the bar just reached its end and `0x02F1` should be sent.
    pub fn poll(&mut self, now_ms: u32) -> bool {
        if self.progress_active && progress_complete(self.progress_started_ms, self.progress_duration_ms, now_ms) {
            self.progress_active = false;
            true
        } else {
            false
        }
    }

    /// Player started walking. Caller sends `0x02F2` and then an early
    /// `0x02F1`.
    pub fn cancel_for_move(&mut self) -> bool {
        self.take_active()
    }

    /// Server sent `0x02F2`. Caller sends only an early `0x02F1`.
    pub fn cancel_from_server(&mut self) -> bool {
        self.take_active()
    }

    fn take_active(&mut self) -> bool {
        if self.progress_active {
            self.progress_active = false;
            true
        } else {
            false
        }
    }

    pub fn progress_active(&self) -> bool {
        self.progress_active
    }

    pub fn progress_fraction(&self, now_ms: u32) -> f32 {
        if !self.progress_active {
            return 0.0;
        }
        progress_fraction(self.progress_started_ms, self.progress_duration_ms, now_ms)
    }

    pub fn progress_color(&self) -> u32 {
        self.progress_color
    }

    pub fn set_npc_bgm(&mut self, track: String) {
        self.npc_bgm = track;
        self.npc_bgm_active = true;
    }

    pub fn clear_npc_bgm(&mut self) {
        self.npc_bgm.clear();
        self.npc_bgm_active = false;
    }

    pub fn npc_bgm_active(&self) -> bool {
        self.npc_bgm_active
    }

    pub fn npc_bgm(&self) -> &str {
        self.npc_bgm.as_str()
    }

    /// Drop cutin, bar, and NPC BGM when the map changes.
    pub fn clear_map(&mut self) {
        self.clear_cutin();
        self.progress_active = false;
        self.clear_npc_bgm();
    }
}

/// Screen rectangle for a cutin. Images larger than half the screen are scaled
/// down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutinRect {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

pub fn place_cutin(anchor: u8, image_w: f32, image_h: f32, screen_w: f32, screen_h: f32) -> CutinRect {
    let mut width = image_w.max(1.0);
    let mut height = image_h.max(1.0);
    let scale = (screen_w * 0.5 / width).min(screen_h * 0.6 / height).min(1.0);
    width *= scale;
    height *= scale;

    let margin = 8.0;
    let (left, top) = match anchor {
        0 => (margin, screen_h - height - margin),
        1 => ((screen_w - width) * 0.5, screen_h - height - margin),
        2 => (screen_w - width - margin, screen_h - height - margin),
        _ => ((screen_w - width) * 0.5, (screen_h - height) * 0.5),
    };

    CutinRect { left, top, width, height }
}

/// Packet color `0` uses the theme token. Any other value is the server's
/// `0xRRGGBB`.
pub fn progress_fill_color(packet_color: u32, fallback: Color) -> Color {
    if packet_color == 0 {
        fallback
    } else {
        Color::rgb_u8(
            ((packet_color >> 16) & 0xFF) as u8,
            ((packet_color >> 8) & 0xFF) as u8,
            (packet_color & 0xFF) as u8,
        )
    }
}

/// Screen position of the progress bar, above the player body.
/// `None` when the player is behind the camera.
pub fn progress_anchor(clip_w: f32, screen_x: f32, screen_y: f32) -> Option<ScreenPosition> {
    if clip_w <= 0.0 {
        return None;
    }
    Some(ScreenPosition {
        left: screen_x,
        top: screen_y - 48.0,
    })
}

pub fn progress_bar_size(theme_width: f32, theme_height: f32, fraction: f32) -> (ScreenSize, ScreenSize) {
    let full = ScreenSize {
        width: theme_width,
        height: theme_height,
    };
    let fill = ScreenSize {
        width: theme_width * fraction.clamp(0.0, 1.0),
        height: theme_height,
    };
    (full, fill)
}

#[cfg(test)]
mod tests {
    use super::{NpcPresentation, place_cutin, progress_anchor, progress_fill_color};
    use crate::graphics::{Color, ScreenPosition};

    #[test]
    fn cutin_anchors_cover_the_four_screen_slots() {
        let screen_w = 800.0;
        let screen_h = 600.0;
        let left = place_cutin(0, 100.0, 40.0, screen_w, screen_h);
        assert_eq!(left.left, 8.0);
        assert!(left.top > screen_h * 0.5);

        let center = place_cutin(1, 100.0, 40.0, screen_w, screen_h);
        assert!((center.left - (screen_w - 100.0) * 0.5).abs() < 0.1);

        let right = place_cutin(2, 100.0, 40.0, screen_w, screen_h);
        assert!((right.left - (screen_w - 100.0 - 8.0)).abs() < 0.1);

        let middle = place_cutin(3, 100.0, 40.0, screen_w, screen_h);
        assert!((middle.left - center.left).abs() < 0.1);
        assert!(middle.top < left.top);

        let scaled = place_cutin(3, 2000.0, 1000.0, screen_w, screen_h);
        assert!(scaled.width <= screen_w * 0.5 + 0.1);
        assert!(scaled.height <= screen_h * 0.6 + 0.1);
    }

    #[test]
    fn progress_bar_completes_once_and_ignores_a_second_move() {
        let mut presentation = NpcPresentation::default();
        presentation.start_progress(0, 2, 1000);
        assert!(!presentation.poll(2999));
        assert!(presentation.progress_active());
        assert!(presentation.poll(3000));
        assert!(!presentation.poll(4000));
        assert!(!presentation.cancel_for_move());
        assert!(!presentation.cancel_from_server());

        presentation.start_progress(0x00FF_0000, 5, 0);
        assert!(presentation.cancel_for_move());
        assert!(!presentation.cancel_from_server());
    }

    #[test]
    fn zero_progress_color_keeps_the_theme_token() {
        let fallback = Color::rgb_u8(1, 2, 3);
        assert_eq!(progress_fill_color(0, fallback), fallback);
        let red = progress_fill_color(0x00FF_0000, fallback);
        assert_eq!(red.red_as_u8(), 255);
        assert_eq!(red.green_as_u8(), 0);
        assert_eq!(red.blue_as_u8(), 0);
    }

    #[test]
    fn progress_anchor_hides_behind_the_camera() {
        assert_eq!(
            progress_anchor(1.0, 100.0, 200.0),
            Some(ScreenPosition { left: 100.0, top: 152.0 })
        );
        assert!(progress_anchor(0.0, 100.0, 200.0).is_none());
    }
}
