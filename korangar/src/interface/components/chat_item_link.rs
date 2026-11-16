//! Draw `<ITEML>` runs inside a chat line and open the pinned details window.

use korangar_interface::event::{ClickHandler, EventQueue};
use korangar_interface::layout::area::Area;
use korangar_interface::layout::tooltip::TooltipExt;
use korangar_interface::layout::{MouseButton, WindowLayout};
use korangar_interface::prelude::{HorizontalAlignment, VerticalAlignment};
use rust_state::State;

use crate::graphics::Color;
use crate::input::InputEvent;
use crate::loaders::{FontSize, OverflowBehavior};
use crate::state::ClientState;
use crate::state::item_link::{ItemIdentity, ParsedItemLink, chat_segments};

pub(crate) struct ChatLinkHit {
    key: u64,
    title: String,
    body: String,
}

impl ClickHandler<ClientState> for ChatLinkHit {
    fn handle_click(&self, _: &State<ClientState>, queue: &mut EventQueue<ClientState>) {
        queue.queue(InputEvent::OpenItemDetails {
            key: self.key,
            title: self.title.clone(),
            body: self.body.clone(),
        });
    }
}

pub(crate) struct LaidFragment {
    pub text: String,
    pub accent: bool,
    pub width: f32,
    pub hit: Option<ChatLinkHit>,
}

pub(crate) struct LaidRow {
    pub fragments: Vec<LaidFragment>,
}

pub(crate) struct LaidChatLine {
    pub height: f32,
    pub rows: Vec<LaidRow>,
}

/// Wrap `text` into rows. A plain run is one `add_text` so `^RRGGBB` covers
/// every word. A link stays one accent fragment. A plain run that has to be
/// split by hand copies the active color onto the next piece.
pub(crate) fn lay_out_chat_line(
    text: &str,
    max_width: f32,
    gold: Color,
    resolve: impl Fn(u32) -> Option<ItemIdentity>,
    measure: impl Fn(&str) -> (f32, f32),
) -> LaidChatLine {
    let (_, measured_height) = measure("Mg");
    let line_height = if measured_height > 0.0 { measured_height } else { 1.0 };
    let segments = chat_segments(text, &resolve);
    let mut rows = vec![LaidRow { fragments: Vec::new() }];
    let mut row_width = 0.0;
    let mut inherited: Option<String> = None;
    for segment in &segments {
        if let Some(link) = segment.link.as_ref() {
            place_piece(
                &mut rows,
                &mut row_width,
                max_width,
                &measure,
                &segment.text,
                true,
                Some(hit_for(link, resolve(link.item_id).as_ref(), gold)),
            );
            continue;
        }
        let mut plain = segment.text.clone();
        if let Some(code) = inherited.as_deref()
            && !starts_with_color(&plain)
        {
            plain = format!("^{code}{plain}");
        }
        place_plain(&mut rows, &mut row_width, max_width, &measure, &plain);
        if let Some(code) = color_at_end(&plain) {
            inherited = Some(code);
        }
    }
    if rows.iter().all(|row| row.fragments.is_empty()) {
        rows.clear();
    }
    let height = line_height * (rows.len().max(1) as f32);
    LaidChatLine { height, rows }
}

fn place_plain(rows: &mut Vec<LaidRow>, row_width: &mut f32, max_width: f32, measure: &impl Fn(&str) -> (f32, f32), plain: &str) {
    if plain.is_empty() {
        return;
    }
    let (width, _) = measure(plain);
    if fits(*row_width, width, max_width) {
        push_fragment(rows, plain, false, width, None);
        *row_width += width;
        return;
    }
    if *row_width > 0.0 && (max_width <= 0.0 || width <= max_width) {
        rows.push(LaidRow { fragments: Vec::new() });
        *row_width = 0.0;
        push_fragment(rows, plain, false, width, None);
        *row_width = width;
        return;
    }
    for piece in pieces_with_color(plain) {
        place_piece(rows, row_width, max_width, measure, &piece, false, None);
    }
}

fn place_piece(
    rows: &mut Vec<LaidRow>,
    row_width: &mut f32,
    max_width: f32,
    measure: &impl Fn(&str) -> (f32, f32),
    piece: &str,
    accent: bool,
    hit: Option<ChatLinkHit>,
) {
    if piece.is_empty() {
        return;
    }
    let (width, _) = measure(piece);
    let whitespace = is_whitespace_piece(piece);
    if *row_width > 0.0 && max_width > 0.0 && *row_width + width > max_width {
        if whitespace {
            return;
        }
        rows.push(LaidRow { fragments: Vec::new() });
        *row_width = 0.0;
    }
    push_fragment(rows, piece, accent, width, hit);
    *row_width += width;
}

fn push_fragment(rows: &mut [LaidRow], text: &str, accent: bool, width: f32, hit: Option<ChatLinkHit>) {
    rows.last_mut().expect("a row is open").fragments.push(LaidFragment {
        text: text.to_owned(),
        accent,
        width,
        hit,
    });
}

fn fits(row_width: f32, width: f32, max_width: f32) -> bool {
    max_width <= 0.0 || row_width + width <= max_width
}

pub(crate) struct ChatPaint<'a> {
    pub line: &'a LaidChatLine,
    pub left: f32,
    pub top: f32,
    pub font_size: FontSize,
    pub plain: Color,
    pub accent: Color,
    pub highlight: Color,
}

pub(crate) fn paint_chat_line<'a>(layout: &mut WindowLayout<'a, ClientState>, paint: ChatPaint<'a>) {
    let row_count = paint.line.rows.len().max(1) as f32;
    let row_height = paint.line.height / row_count;
    let mut top = paint.top;
    for row in &paint.line.rows {
        let mut left = paint.left;
        for fragment in &row.fragments {
            let text_area = Area {
                left,
                top,
                width: fragment.width + 2.0,
                height: row_height,
            };
            let color = if fragment.accent { paint.accent } else { paint.plain };
            layout.add_text(
                text_area,
                fragment.text.as_str(),
                paint.font_size,
                color,
                paint.highlight,
                HorizontalAlignment::Left { offset: 0.0, border: 0.0 },
                VerticalAlignment::Top { offset: 0.0 },
                OverflowBehavior::LineBreak,
            );
            if let Some(hit) = fragment.hit.as_ref() {
                let hit_area = Area {
                    width: fragment.width,
                    ..text_area
                };
                if hit_area.check().run(layout) {
                    layout.register_click_handler(MouseButton::Left, hit);
                    struct ChatLinkTooltip;
                    layout.add_immediate_tooltip(hit.body.as_str(), ChatLinkTooltip.tooltip_id());
                }
            }
            left += fragment.width;
        }
        top += row_height;
    }
}

fn hit_for(link: &ParsedItemLink, identity: Option<&ItemIdentity>, gold: Color) -> ChatLinkHit {
    let (key, title, body) = link.pinned_details(identity, gold);
    ChatLinkHit { key, title, body }
}

fn pieces_with_color(text: &str) -> Vec<String> {
    let mut active: Option<String> = None;
    let mut pieces = Vec::new();
    for token in whitespace_tokens(text) {
        let mut piece = token.to_owned();
        if !starts_with_color(&piece)
            && let Some(code) = &active
        {
            piece = format!("^{code}{piece}");
        }
        if let Some(code) = color_at_end(token) {
            active = Some(code);
        }
        pieces.push(piece);
    }
    pieces
}

fn whitespace_tokens(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    let mut space_run = false;
    for (index, character) in text.char_indices() {
        let space = character.is_whitespace();
        match start {
            None => {
                start = Some(index);
                space_run = space;
            }
            Some(from) if space != space_run => {
                tokens.push(&text[from..index]);
                start = Some(index);
                space_run = space;
            }
            Some(_) => {}
        }
    }
    if let Some(from) = start {
        tokens.push(&text[from..]);
    }
    tokens
}

fn starts_with_color(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 7 && bytes[0] == b'^' && bytes[1..7].iter().all(|byte| byte.is_ascii_hexdigit())
}

fn color_at_end(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut found = None;
    let mut index = 0;
    while index < bytes.len() {
        if !text.is_char_boundary(index) {
            index += 1;
            continue;
        }
        if bytes[index] == b'^'
            && index + 7 <= bytes.len()
            && text.is_char_boundary(index + 7)
            && bytes[index + 1..index + 7].iter().all(|byte| byte.is_ascii_hexdigit())
        {
            found = Some(text[index + 1..index + 7].to_owned());
            index += 7;
            continue;
        }
        index += text[index..].chars().next().map(char::len_utf8).unwrap_or(1);
    }
    found
}

fn is_whitespace_piece(text: &str) -> bool {
    let visible = strip_color_codes(text);
    !visible.is_empty() && visible.chars().all(char::is_whitespace)
}

fn strip_color_codes(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'^' && index + 7 <= bytes.len() && bytes[index + 1..index + 7].iter().all(|byte| byte.is_ascii_hexdigit()) {
            index += 7;
            continue;
        }
        let character = text[index..].chars().next().unwrap_or(' ');
        out.push(character);
        index += character.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_is_one_fragment_and_the_next_word_wraps() {
        let text = "hi <ITEML>000020jn%07&00)01</ITEML> tail";
        let line = lay_out_chat_line(
            text,
            40.0,
            Color::rgb(1.0, 0.8, 0.2),
            |item_id| {
                (item_id == 1201).then(|| ItemIdentity {
                    name: "Knife".to_owned(),
                    slots: 3,
                    description: String::new(),
                })
            },
            |sample| (sample.chars().count() as f32 * 10.0, 16.0),
        );
        assert!(line.rows.len() >= 2, "the link should wrap onto its own row");
        let link = line
            .rows
            .iter()
            .flat_map(|row| &row.fragments)
            .find(|fragment| fragment.accent)
            .expect("accent link");
        assert_eq!(link.text, "[+7 Knife [3]]");
        assert!(link.hit.is_some());
        let joined = line
            .rows
            .iter()
            .flat_map(|row| row.fragments.iter().map(|fragment| fragment.text.as_str()))
            .collect::<Vec<_>>()
            .join("");
        assert!(joined.contains("hi"));
        assert!(joined.contains("tail"));
    }

    #[test]
    fn color_code_covers_every_word_in_one_plain_run() {
        let line = lay_out_chat_line("^FF0000two words", 4_000.0, Color::rgb(1.0, 0.8, 0.2), |_| None, char_measure);
        let fragments = fragments_of(&line);
        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, "^FF0000two words");
        assert!(!fragments[0].accent);
    }

    #[test]
    fn color_before_a_link_continues_until_the_reset() {
        let text = "^FF0000look <ITEML>000020jn%07&00)01</ITEML> red^000000 end";
        let line = lay_out_chat_line(
            text,
            4_000.0,
            Color::rgb(1.0, 0.8, 0.2),
            |item_id| {
                (item_id == 1201).then(|| ItemIdentity {
                    name: "Knife".to_owned(),
                    slots: 3,
                    description: String::new(),
                })
            },
            char_measure,
        );
        let fragments = fragments_of(&line);
        let before = fragments.iter().find(|fragment| fragment.text.contains("look")).expect("look");
        assert!(before.text.starts_with("^FF0000"), "{}", before.text);
        assert!(
            fragments
                .iter()
                .any(|fragment| fragment.accent && fragment.text == "[+7 Knife [3]]")
        );
        let after = fragments.iter().find(|fragment| fragment.text.contains("red")).expect("red");
        assert!(after.text.starts_with("^FF0000"), "{}", after.text);
        assert!(after.text.contains("^000000"), "{}", after.text);
        assert!(after.text.contains("end"), "{}", after.text);
    }

    fn char_measure(sample: &str) -> (f32, f32) {
        (sample.chars().count() as f32 * 10.0, 16.0)
    }

    fn fragments_of(line: &LaidChatLine) -> Vec<&LaidFragment> {
        line.rows.iter().flat_map(|row| row.fragments.iter()).collect()
    }
}
