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
use crate::state::item_link::{ChatSegment, ItemIdentity, ParsedItemLink, chat_segments};

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

/// Wrap `text` into rows. Item links are one fragment. `measure` returns the
/// natural width and height of a string.
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
    for segment in &segments {
        for piece in segment_pieces(segment) {
            if piece.is_empty() {
                continue;
            }
            let (width, _) = measure(piece);
            let whitespace = piece.chars().all(char::is_whitespace);
            if row_width > 0.0 && max_width > 0.0 && row_width + width > max_width {
                if whitespace {
                    continue;
                }
                rows.push(LaidRow { fragments: Vec::new() });
                row_width = 0.0;
            }
            rows.last_mut().expect("a row is open").fragments.push(LaidFragment {
                text: piece.to_owned(),
                accent: segment.accent,
                width,
                hit: segment
                    .link
                    .as_ref()
                    .map(|link| hit_for(link, resolve(link.item_id).as_ref(), gold)),
            });
            row_width += width;
        }
    }
    if rows.iter().all(|row| row.fragments.is_empty()) {
        rows.clear();
    }
    let height = line_height * (rows.len().max(1) as f32);
    LaidChatLine { height, rows }
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

fn segment_pieces(segment: &ChatSegment) -> Vec<&str> {
    if segment.link.is_some() {
        vec![segment.text.as_str()]
    } else {
        plain_tokens(&segment.text)
    }
}

fn plain_tokens(text: &str) -> Vec<&str> {
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
}
