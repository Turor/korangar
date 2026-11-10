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

/// Gap between a portrait frame and the dialog window.
pub const FRAME_GAP: f32 = 8.0;
/// Width of the themed border around a portrait.
pub const FRAME_BORDER: f32 = 2.0;
/// Space between the border and the art.
pub const FRAME_PADDING: f32 = 4.0;
/// The text window never gets narrower than this; portraits shrink first.
pub const MIN_WINDOW_WIDTH: f32 = 320.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortraitFrame {
    pub left: f32,
    pub top: f32,
    pub size: f32,
}

impl PortraitFrame {
    /// The area inside the border and padding, as `(left, top, size)`.
    pub fn inner(&self) -> (f32, f32, f32) {
        let inset = FRAME_BORDER + FRAME_PADDING;
        (self.left + inset, self.top + inset, (self.size - inset * 2.0).max(0.0))
    }
}

/// The bottom band: a square portrait frame on each side, as tall as the
/// dialog, and the dialog window between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandLayout {
    pub band: DialogBand,
    pub player_frame: PortraitFrame,
    pub npc_frame: PortraitFrame,
    pub window_left: f32,
    pub window_width: f32,
}

pub fn band_layout(screen_w: f32, screen_h: f32) -> BandLayout {
    let band = dialog_band(screen_w, screen_h);
    let top = screen_h - band.height - band.margin;
    let room_for_frames = ((band.width - MIN_WINDOW_WIDTH) / 2.0 - FRAME_GAP).max(0.0);
    let size = band.height.min(room_for_frames);
    let side = if size > 0.0 { size + FRAME_GAP } else { 0.0 };
    BandLayout {
        band,
        player_frame: PortraitFrame {
            left: band.margin,
            top: top + band.height - size,
            size,
        },
        npc_frame: PortraitFrame {
            left: band.margin + band.width - size,
            top: top + band.height - size,
            size,
        },
        window_left: band.margin + side,
        window_width: band.width - side * 2.0,
    }
}

/// Offset for [`korangar_interface::window::AnchorPoint::BottomCenter`].
/// The anchor position is the window's top-left.
pub fn bottom_anchor_offset(screen_w: f32, layout: BandLayout) -> (f32, f32) {
    (layout.window_left - screen_w / 2.0, -(layout.band.height + layout.band.margin))
}

/// Scale for art of `art_w` x `art_h` inside a `box_size` square. Sprites
/// that can grow at least 2x use a whole factor so every source pixel stays
/// square; smaller steps fill the box and are drawn with nearest sampling on
/// whole screen pixels.
pub fn portrait_scale(art_w: f32, art_h: f32, box_size: f32) -> f32 {
    if art_w <= 0.0 || art_h <= 0.0 || box_size <= 0.0 {
        return 0.0;
    }
    let fit = (box_size / art_w).min(box_size / art_h);
    if fit >= 2.0 { fit.floor() } else { fit }
}

/// Where a part at `(x, y, w, h)` (sprite space, relative to the art's
/// top-left) lands when the art is scaled by `scale` and stands
/// bottom-centred in the box. Every edge is on a whole pixel.
pub fn place_portrait_part(
    art_w: f32,
    art_h: f32,
    part: (f32, f32, f32, f32),
    scale: f32,
    box_left: f32,
    box_top: f32,
    box_size: f32,
) -> (f32, f32, f32, f32) {
    let origin_left = (box_left + (box_size - art_w * scale) / 2.0).round();
    let origin_top = (box_top + box_size - art_h * scale).round();
    let (x, y, w, h) = part;
    let left = (origin_left + x * scale).round();
    let top = (origin_top + y * scale).round();
    let right = (origin_left + (x + w) * scale).round();
    let bottom = (origin_top + (y + h) * scale).round();
    (left, top, right - left, bottom - top)
}

/// Options per row before the menu wraps to another row.
pub const CHOICES_PER_ROW: usize = 5;

/// `(columns, rows)` for `count` options laid out side by side.
pub fn choice_grid(count: usize) -> (usize, usize) {
    if count == 0 {
        return (0, 0);
    }
    let columns = count.min(CHOICES_PER_ROW);
    (columns, count.div_ceil(columns))
}

/// Keys that act on an open menu this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChoiceKeys {
    /// Left or Up.
    pub previous: bool,
    /// Right, Down or Tab.
    pub next: bool,
    /// Enter or Space.
    pub confirm: bool,
    /// `1`..=`9`, as typed.
    pub digit: Option<u8>,
}

impl ChoiceKeys {
    pub fn any(&self) -> bool {
        self.previous || self.next || self.confirm || self.digit.is_some()
    }

    /// Enter, Space and the digit keys from typed characters.
    pub fn from_characters(characters: &[char]) -> Self {
        Self {
            confirm: characters.iter().any(|character| matches!(character, '\r' | '\n' | ' ')),
            digit: characters.iter().find_map(|character| {
                character
                    .to_digit(10)
                    .filter(|digit| (1..=9).contains(digit))
                    .map(|digit| digit as u8)
            }),
            next: characters.contains(&'\t'),
            previous: false,
        }
    }
}

/// New focus and the option to pick (zero based), if any. Focus wraps.
pub fn choice_key_outcome(focus: usize, count: usize, keys: ChoiceKeys) -> (usize, Option<usize>) {
    if count == 0 {
        return (0, None);
    }
    let mut focus = focus.min(count - 1);
    if let Some(digit) = keys.digit {
        let index = usize::from(digit) - 1;
        if index < count {
            return (index, Some(index));
        }
    }
    if keys.next {
        focus = (focus + 1) % count;
    } else if keys.previous {
        focus = (focus + count - 1) % count;
    }
    let pick = keys.confirm.then_some(focus);
    (focus, pick)
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

        let layout = band_layout(1280.0, 720.0);
        let band = layout.band;
        let (left_offset, top_offset) = bottom_anchor_offset(1280.0, layout);
        let left = 1280.0 / 2.0 + left_offset;
        let top = 720.0 + top_offset;
        assert!((left - layout.window_left).abs() < 0.1);
        assert!((top + band.height + band.margin - 720.0).abs() < 0.1);

        let (player, npc) = (layout.player_frame, layout.npc_frame);
        // The frames are as tall as the dialog and sit beside it, not over it.
        assert!((player.size - band.height).abs() < 0.1);
        assert!((player.top - top).abs() < 0.1);
        assert!(player.left + player.size + FRAME_GAP <= layout.window_left + 0.1);
        assert!(layout.window_left + layout.window_width + FRAME_GAP <= npc.left + 0.1);
        assert!((npc.left + npc.size - (band.margin + band.width)).abs() < 0.1);
        assert_eq!(player.size, npc.size);
    }

    #[test]
    fn narrow_screens_shrink_the_frames_before_the_text() {
        let layout = band_layout(640.0, 500.0);
        assert!(layout.window_width >= MIN_WINDOW_WIDTH - 0.1);
        assert!(layout.player_frame.size < layout.band.height);

        let tiny = band_layout(352.0, 400.0);
        assert_eq!(tiny.player_frame.size, 0.0);
        assert!((tiny.window_left - tiny.band.margin).abs() < 0.1);
    }

    #[test]
    fn portraits_scale_by_whole_steps_and_land_on_whole_pixels() {
        // A 60x90 sprite in a 190 box grows 2x, not 2.1x.
        assert_eq!(portrait_scale(60.0, 90.0, 190.0), 2.0);
        // Below 2x the art fills the box.
        assert!((portrait_scale(60.0, 120.0, 180.0) - 1.5).abs() < 0.001);
        // A large cutin shrinks to fit.
        assert!((portrait_scale(400.0, 500.0, 200.0) - 0.4).abs() < 0.001);
        assert_eq!(portrait_scale(0.0, 10.0, 100.0), 0.0);

        let scale = portrait_scale(60.0, 120.0, 180.0);
        let (left, top, width, height) = place_portrait_part(60.0, 120.0, (5.0, 7.0, 11.0, 13.0), scale, 10.3, 20.7, 180.0);
        for value in [left, top, width, height] {
            assert_eq!(value, value.round());
        }
        // The whole figure stands on the bottom of the box.
        let (_, top, _, height) = place_portrait_part(60.0, 120.0, (0.0, 0.0, 60.0, 120.0), scale, 10.0, 20.0, 180.0);
        assert_eq!(top + height, 200.0);
    }

    #[test]
    fn menus_lay_out_in_rows_of_five() {
        assert_eq!(choice_grid(0), (0, 0));
        assert_eq!(choice_grid(2), (2, 1));
        assert_eq!(choice_grid(5), (5, 1));
        assert_eq!(choice_grid(9), (5, 2));
    }

    #[test]
    fn menu_keys_move_focus_and_pick() {
        let right = ChoiceKeys {
            next: true,
            ..Default::default()
        };
        let left = ChoiceKeys {
            previous: true,
            ..Default::default()
        };
        let confirm = ChoiceKeys {
            confirm: true,
            ..Default::default()
        };
        assert_eq!(choice_key_outcome(0, 3, right), (1, None));
        assert_eq!(choice_key_outcome(2, 3, right), (0, None));
        assert_eq!(choice_key_outcome(0, 3, left), (2, None));
        assert_eq!(choice_key_outcome(1, 3, confirm), (1, Some(1)));
        assert_eq!(choice_key_outcome(0, 3, ChoiceKeys::from_characters(&['3'])), (2, Some(2)));
        // A digit past the last option does nothing.
        assert_eq!(choice_key_outcome(0, 3, ChoiceKeys::from_characters(&['7'])), (0, None));
        assert_eq!(choice_key_outcome(0, 0, confirm), (0, None));
        assert!(ChoiceKeys::from_characters(&[' ']).confirm);
        assert!(ChoiceKeys::from_characters(&['\t']).next);
        assert!(!ChoiceKeys::from_characters(&['0', 'a']).any());
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
