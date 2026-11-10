//! Pure state for the visual-novel NPC dialog.
//!
//! Packet bytes stay in `korangar-networking`. This module decides how far the
//! typewriter has revealed, whether a click advances, which Escape packet the
//! current page needs, and which art belongs on each side of the screen.
//! Color codes follow the same `^RRGGBB` / `^000000` rules as the font
//! `ColorSpanIterator`: six hex digits set the color, `^000000` returns to
//! the caller color, and `^000001` is the highlight token.

/// Idle action slot for a three-quarter view facing screen-right. ACT
/// directions run clockwise from south: 0 S, 1 SW, 2 W, ..., 6 E, 7 SE.
pub const FACING_RIGHT: usize = 7;
/// Idle action slot for a three-quarter view facing screen-left (SW).
pub const FACING_LEFT: usize = 1;

/// Hercules `MAX_MENU_OPTIONS`. Wire byte `0xFF`, which is `-1` as `i8`.
pub const MENU_CANCEL_OPTION: i8 = -1;

const COLOR_CODE_BYTES: usize = 7;
const DEFAULT_CHARS_PER_SECOND: f32 = 40.0;
const DEFAULT_PORTRAIT_DIM: f32 = 0.45;

/// One visible step. A `^` plus six hex digits is consumed for free so the
/// typewriter never splits a color code or counts it as a character.
pub fn next_visible_end(text: &str, shown_bytes: usize) -> usize {
    let mut index = align_char_boundary(text, shown_bytes);
    index = skip_color_codes(text, index);
    if index >= text.len() {
        return text.len();
    }
    let Some(character) = text[index..].chars().next() else {
        return text.len();
    };
    skip_color_codes(text, index + character.len_utf8())
}

/// Advance the reveal by whole characters. `carry_ms` is the leftover time
/// that did not yet buy a character.
pub fn advance_reveal(text: &str, shown_bytes: usize, carry_ms: f32, delta_ms: u32, chars_per_sec: f32) -> (usize, f32) {
    if !chars_per_sec.is_finite() || chars_per_sec <= 0.0 {
        return (shown_bytes.min(text.len()), 0.0);
    }

    let mut end = skip_color_codes(text, align_char_boundary(text, shown_bytes));
    let mut carry = carry_ms.max(0.0) + delta_ms as f32;
    let ms_per_char = 1000.0 / chars_per_sec;

    while carry >= ms_per_char && end < text.len() {
        let next = next_visible_end(text, end);
        if next == end {
            break;
        }
        end = next;
        carry -= ms_per_char;
    }

    (end, carry)
}

fn align_char_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    if text.is_char_boundary(index) {
        index
    } else {
        (index..text.len())
            .find(|candidate| text.is_char_boundary(*candidate))
            .unwrap_or(text.len())
    }
}

fn skip_color_codes(text: &str, mut index: usize) -> usize {
    while let Some(end) = color_code_end(text, index) {
        index = end;
    }
    index
}

/// `Some(end)` when `text[index..]` starts with `^` and six hex digits.
pub fn color_code_end(text: &str, index: usize) -> Option<usize> {
    let rest = text.get(index..)?;
    let bytes = rest.as_bytes();
    if bytes.first().copied() != Some(b'^') || bytes.len() < COLOR_CODE_BYTES {
        return None;
    }
    let hex = &bytes[1..COLOR_CODE_BYTES];
    if hex.iter().all(|byte| byte.is_ascii_hexdigit()) {
        Some(index + COLOR_CODE_BYTES)
    } else {
        None
    }
}

/// What a primary click does while a script is on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickDecision {
    /// The page is still revealing. Finish the text and do not advance.
    CompleteReveal,
    /// Send `CZ_REQ_NEXT_SCRIPT`.
    Next,
    /// Send `CZ_CLOSE_DIALOG`.
    Close,
    /// A menu, input, or prompt owns the click.
    Ignore,
}

pub fn route_click(revealing: bool, awaiting_next: bool, awaiting_close: bool, blocked: bool) -> ClickDecision {
    if blocked {
        return ClickDecision::Ignore;
    }
    if revealing {
        return ClickDecision::CompleteReveal;
    }
    if awaiting_next {
        return ClickDecision::Next;
    }
    if awaiting_close {
        return ClickDecision::Close;
    }
    ClickDecision::Ignore
}

/// Menus, number/text input, and a real item or cutin prompt block stray
/// clicks. A cutin that illustrates a next/close page does not.
pub fn blocks_stray_click(choice: bool, input: bool, item_prompt: bool, cutin_without_page: bool) -> bool {
    choice || input || item_prompt || cutin_without_page
}

/// Escape packet for the page that is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeKind {
    /// `CZ_CHOOSE_MENU` with [`MENU_CANCEL_OPTION`].
    MenuCancel,
    /// Number `0` or an empty string, which is how Hercules leaves an input.
    InputCancel,
    /// `CZ_CLOSE_DIALOG`.
    CloseDialog,
}

pub fn escape_action(menu: bool, input: bool, plain_dialog: bool) -> Option<EscapeKind> {
    if menu {
        Some(EscapeKind::MenuCancel)
    } else if input {
        Some(EscapeKind::InputCancel)
    } else if plain_dialog {
        Some(EscapeKind::CloseDialog)
    } else {
        None
    }
}

/// Art on the right side of the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightPortraitSource {
    Cutin,
    Sprite,
    None,
}

/// What the entity id on the right refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortraitSubject {
    Npc,
    Monster,
    Warp,
    Hidden,
    Player,
    Missing,
}

/// A server cutin wins. An NPC or monster without one uses its idle sprite.
/// Warps, hidden NPCs, and a missing entity draw nothing.
pub fn right_portrait_source(cutin_visible: bool, subject: PortraitSubject) -> RightPortraitSource {
    if cutin_visible {
        return RightPortraitSource::Cutin;
    }
    match subject {
        PortraitSubject::Npc | PortraitSubject::Monster => RightPortraitSource::Sprite,
        PortraitSubject::Warp | PortraitSubject::Hidden | PortraitSubject::Player | PortraitSubject::Missing => RightPortraitSource::None,
    }
}

/// Left-side portrait. The renderer draws the same body and head sprites the
/// world player uses, then the headgear slots and dye palettes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerPortraitParts {
    pub includes_body: bool,
    pub includes_head: bool,
    pub head_top: u16,
    pub head_mid: u16,
    pub head_bottom: u16,
    pub hair_palette: u16,
    pub body_palette: u16,
    pub facing: u8,
}

pub fn player_portrait_parts(head_top: u16, head_mid: u16, head_bottom: u16, hair_palette: u16, body_palette: u16) -> PlayerPortraitParts {
    PlayerPortraitParts {
        includes_body: true,
        includes_head: true,
        head_top,
        head_mid,
        head_bottom,
        hair_palette,
        body_palette,
        facing: FACING_RIGHT as u8,
    }
}

/// One run of text that shares a color. `None` is the caller's default.
/// `"highlight"` is `^000001`. A hex string is `^RRGGBB`.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ColorSegment<'a> {
    pub text: &'a str,
    pub color: Option<&'a str>,
}

#[cfg(test)]
fn color_segments(text: &str) -> Vec<ColorSegment<'_>> {
    let mut segments = Vec::new();
    let mut color: Option<&str> = None;
    let mut start = 0;
    let mut index = 0;

    while index < text.len() {
        if let Some(code_end) = color_code_end(text, index) {
            if index > start {
                segments.push(ColorSegment {
                    text: &text[start..index],
                    color,
                });
            }
            let code = &text[index + 1..code_end];
            color = match code {
                "000000" => None,
                "000001" => Some("highlight"),
                other => Some(other),
            };
            index = code_end;
            start = code_end;
        } else {
            index += text[index..].chars().next().map(char::len_utf8).unwrap_or(1);
        }
    }

    if start < text.len() {
        segments.push(ColorSegment {
            text: &text[start..],
            color,
        });
    }

    segments
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogBand {
    pub width: f32,
    pub height: f32,
    pub margin: f32,
}

/// Wide band along the bottom. Width tracks the window; height stays in a
/// readable range.
pub fn dialog_band(screen_w: f32, screen_h: f32) -> DialogBand {
    let margin = 16.0;
    let width = (screen_w - margin * 2.0).clamp(320.0, screen_w.max(320.0));
    let height = (screen_h * 0.28).clamp(150.0, 240.0);
    DialogBand { width, height, margin }
}

/// Offset for [`korangar_interface::window::AnchorPoint::BottomCenter`].
/// The anchor position is the window's top-left.
pub fn bottom_anchor_offset(screen_w: f32, band: DialogBand) -> (f32, f32) {
    (band.margin - screen_w / 2.0, -(band.height + band.margin))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortraitFrame {
    pub left: f32,
    pub top: f32,
    pub size: f32,
}

pub fn portrait_frames(screen_w: f32, screen_h: f32, band: DialogBand) -> (PortraitFrame, PortraitFrame) {
    let size = (screen_h * 0.62).min(screen_w * 0.34).max(160.0);
    let dialog_top = screen_h - band.height - band.margin;
    let top = (dialog_top - size + 24.0).max(0.0);
    let left = PortraitFrame {
        left: band.margin,
        top,
        size,
    };
    let right = PortraitFrame {
        left: (screen_w - band.margin - size).max(0.0),
        top,
        size,
    };
    (left, right)
}

/// The NPC speaks until the player has to choose or type.
pub fn npc_is_speaking(choice: bool, input: bool) -> bool {
    !choice && !input
}

/// `1` for the speaker. The other side uses the dim token.
pub fn side_alpha(is_speaker: bool, dim: f32) -> f32 {
    if is_speaker { 1.0 } else { portrait_dim_amount(dim) }
}

pub fn typewriter_rate(chars_per_sec: f32) -> f32 {
    if chars_per_sec.is_finite() && chars_per_sec > 0.0 {
        chars_per_sec.min(400.0)
    } else {
        DEFAULT_CHARS_PER_SECOND
    }
}

pub fn portrait_dim_amount(dim: f32) -> f32 {
    if dim.is_finite() {
        dim.clamp(0.15, 1.0)
    } else {
        DEFAULT_PORTRAIT_DIM
    }
}

/// `None` for palette 0, which is the sprite's built-in palette.
pub fn body_palette_file(body_sprite_path: &str, palette: u16) -> Option<String> {
    if palette == 0 {
        return None;
    }
    let file = body_sprite_path.rsplit(['\\', '/']).next()?;
    Some(format!("data\\palette\\몸\\{file}_{palette}.pal"))
}

pub fn head_palette_file(head_sprite_path: &str, palette: u16) -> Option<String> {
    if palette == 0 {
        return None;
    }
    let file = head_sprite_path.rsplit(['\\', '/']).next()?;
    Some(format!("data\\palette\\머리\\머리{file}_{palette}.pal"))
}

/// Accessory sprite paths without an extension. View id 0 is an empty slot.
/// `suffix` is the `AccNameTable` value (`_고글`). Numeric names are the
/// fallback when that table has no row.
pub fn headgear_sprite_candidates(view_id: u16, sex_folder: &str, suffix: Option<&str>) -> Vec<String> {
    if view_id == 0 {
        return Vec::new();
    }
    let mut paths = Vec::new();
    if let Some(suffix) = suffix.filter(|suffix| !suffix.is_empty()) {
        paths.push(format!("악세사리\\{sex_folder}\\{sex_folder}{suffix}"));
    }
    paths.push(format!("악세사리\\{sex_folder}\\{sex_folder}_{view_id}"));
    paths.push(format!("악세사리\\{sex_folder}\\{sex_folder}{view_id}"));
    paths
}

/// The name plate text. Hercules hides everything after `#` and after `::`,
/// so `Captain Carocc#int_land` shows `Captain Carocc`.
pub fn npc_display_name(raw: &str) -> Option<&str> {
    let visible = raw.split('#').next().unwrap_or_default();
    let visible = visible.split("::").next().unwrap_or_default().trim();
    (!visible.is_empty()).then_some(visible)
}

pub fn idle_animation_index(direction: usize, animation_count: usize) -> Option<usize> {
    if animation_count == 0 {
        None
    } else {
        Some(direction % animation_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_plate_hides_the_unique_suffix() {
        assert_eq!(npc_display_name("Captain Carocc#int_land"), Some("Captain Carocc"));
        assert_eq!(npc_display_name("Kafra Employee::kaf_izlude"), Some("Kafra Employee"));
        assert_eq!(npc_display_name("Wounded Swordsman"), Some("Wounded Swordsman"));
        assert_eq!(npc_display_name("#hidden"), None);
        assert_eq!(npc_display_name("  "), None);
    }

    #[test]
    fn typewriter_reveals_then_a_click_completes_then_the_next_advances() {
        let text = "Hello";
        let (after_one, carry) = advance_reveal(text, 0, 0.0, 10, 100.0);
        assert_eq!(&text[..after_one], "H");
        assert!(carry < 10.0);

        let (rest, _) = advance_reveal(text, after_one, 0.0, 10_000, 100.0);
        assert_eq!(rest, text.len());

        assert_eq!(route_click(true, true, false, false), ClickDecision::CompleteReveal);
        assert_eq!(route_click(false, true, false, false), ClickDecision::Next);
        assert_eq!(route_click(false, false, true, false), ClickDecision::Close);
    }

    #[test]
    fn typewriter_does_not_split_a_color_code_or_count_it() {
        let text = "^ff8800Hi";
        let (end, _) = advance_reveal(text, 0, 0.0, 10, 100.0);
        assert_eq!(&text[..end], "^ff8800H");
        assert!(color_code_end(text, 0).is_some());
        assert!(color_code_end(text, 1).is_none());
    }

    #[test]
    fn typewriter_keeps_fractional_time_and_steps_one_hangul_syllable() {
        let (end, carry) = advance_reveal("abcd", 0, 0.0, 25, 100.0);
        assert_eq!(end, 2);
        assert!((carry - 5.0).abs() < 0.01);

        let korean = "안녕";
        let (end, _) = advance_reveal(korean, 0, 0.0, 10, 100.0);
        assert_eq!(&korean[..end], "안");
    }

    #[test]
    fn clicks_advance_until_a_menu_input_or_prompt_is_showing() {
        assert_eq!(route_click(false, true, false, false), ClickDecision::Next);
        assert!(blocks_stray_click(true, false, false, false));
        assert!(blocks_stray_click(false, true, false, false));
        assert!(blocks_stray_click(false, false, true, false));
        assert!(blocks_stray_click(false, false, false, true));
        assert!(!blocks_stray_click(false, false, false, false));
        assert_eq!(
            route_click(false, true, false, blocks_stray_click(true, false, false, false)),
            ClickDecision::Ignore
        );
        assert_eq!(
            route_click(true, true, false, blocks_stray_click(false, true, false, false)),
            ClickDecision::Ignore
        );
    }

    #[test]
    fn escape_picks_the_packet_for_the_open_state() {
        assert_eq!(escape_action(true, true, true), Some(EscapeKind::MenuCancel));
        assert_eq!(MENU_CANCEL_OPTION, -1);
        assert_eq!(escape_action(false, true, true), Some(EscapeKind::InputCancel));
        assert_eq!(escape_action(false, false, true), Some(EscapeKind::CloseDialog));
        assert_eq!(escape_action(false, false, false), None);
    }

    #[test]
    fn right_side_prefers_cutin_then_sprite_then_nothing() {
        assert_eq!(right_portrait_source(true, PortraitSubject::Npc), RightPortraitSource::Cutin);
        assert_eq!(right_portrait_source(true, PortraitSubject::Warp), RightPortraitSource::Cutin);
        assert_eq!(right_portrait_source(false, PortraitSubject::Npc), RightPortraitSource::Sprite);
        assert_eq!(
            right_portrait_source(false, PortraitSubject::Monster),
            RightPortraitSource::Sprite
        );
        assert_eq!(right_portrait_source(false, PortraitSubject::Hidden), RightPortraitSource::None);
        assert_eq!(right_portrait_source(false, PortraitSubject::Warp), RightPortraitSource::None);
        assert_eq!(
            right_portrait_source(false, PortraitSubject::Missing),
            RightPortraitSource::None
        );
    }

    #[test]
    fn left_side_is_the_player_body_head_headgear_and_palette_facing_right() {
        let parts = player_portrait_parts(12, 4, 9, 3, 2);
        assert!(parts.includes_body);
        assert!(parts.includes_head);
        assert_eq!(parts.head_top, 12);
        assert_eq!(parts.head_mid, 4);
        assert_eq!(parts.head_bottom, 9);
        assert_eq!(parts.hair_palette, 3);
        assert_eq!(parts.body_palette, 2);
        assert_eq!(parts.facing, FACING_RIGHT as u8);
        assert_eq!(FACING_LEFT, 1);
    }

    #[test]
    fn color_segments_match_the_dialog_color_codes() {
        let text = "Welcome to ^ff8800Korangar^000000 version ^0000010.1.1^000000!";
        let segments = color_segments(text);
        assert_eq!(
            segments.iter().map(|segment| (segment.text, segment.color)).collect::<Vec<_>>(),
            vec![
                ("Welcome to ", None),
                ("Korangar", Some("ff8800")),
                (" version ", None),
                ("0.1.1", Some("highlight")),
                ("!", None),
            ]
        );

        assert!(color_segments("^FF0000^00FF00").is_empty());
        assert_eq!(color_segments("Text^FF0000")[0].text, "Text");
        assert_eq!(color_segments("^FFInvalid ^FFFF00Valid").len(), 2);
        assert!(color_segments("안녕").iter().all(|segment| segment.color.is_none()));
    }

    #[test]
    fn band_and_portraits_track_the_window() {
        let small = dialog_band(800.0, 600.0);
        let large = dialog_band(1920.0, 1080.0);
        assert!(large.width > small.width);
        assert!(large.height >= small.height);
        assert!((small.width - (800.0 - 32.0)).abs() < 0.1);

        let (left_offset, top_offset) = bottom_anchor_offset(1280.0, dialog_band(1280.0, 720.0));
        let band = dialog_band(1280.0, 720.0);
        let left = 1280.0 / 2.0 + left_offset;
        let top = 720.0 + top_offset;
        assert!((left - band.margin).abs() < 0.1);
        assert!((top + band.height + band.margin - 720.0).abs() < 0.1);

        let (player, npc) = portrait_frames(1280.0, 720.0, band);
        assert!(player.left < npc.left);
        assert!(player.top + player.size > top - 40.0);
        assert_eq!(player.size, npc.size);
    }

    #[test]
    fn palette_and_headgear_paths_follow_the_view_ids() {
        assert_eq!(
            body_palette_file("인간족\\몸통\\남\\초보자_남", 2).as_deref(),
            Some("data\\palette\\몸\\초보자_남_2.pal")
        );
        assert!(body_palette_file("인간족\\몸통\\남\\초보자_남", 0).is_none());
        assert_eq!(
            head_palette_file("인간족\\머리통\\남\\2_남", 3).as_deref(),
            Some("data\\palette\\머리\\머리2_남_3.pal")
        );
        assert!(headgear_sprite_candidates(0, "남", Some("_고글")).is_empty());
        let hats = headgear_sprite_candidates(5, "여", Some("_리본"));
        assert_eq!(hats[0], "악세사리\\여\\여_리본");
        assert!(hats.iter().any(|path| path.ends_with("여_5") || path.ends_with("여5")));
    }

    #[test]
    fn the_npc_is_dimmed_only_while_the_player_must_answer() {
        assert!(npc_is_speaking(false, false));
        assert!(!npc_is_speaking(true, false));
        assert!(!npc_is_speaking(false, true));
        assert_eq!(side_alpha(true, 0.45), 1.0);
        assert!((side_alpha(false, 0.45) - 0.45).abs() < 0.001);
        assert_eq!(typewriter_rate(0.0), DEFAULT_CHARS_PER_SECOND);
        assert!((portrait_dim_amount(0.45) - 0.45).abs() < 0.001);
        assert_eq!(idle_animation_index(FACING_RIGHT, 8), Some(7));
        assert_eq!(idle_animation_index(FACING_LEFT, 0), None);
    }
}
