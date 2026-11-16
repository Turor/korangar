//! Unread markers, partner whisper tabs, and `CZ_WHISPER` (`0x0096`).

use korangar::e2e::{ChatSession, ChatView, LiveConnect};

fn unread(view: &ChatView, label: &str) -> u32 {
    view.tabs
        .iter()
        .find(|tab| tab.label.eq_ignore_ascii_case(label))
        .unwrap_or_else(|| panic!("missing chat tab {label}"))
        .unread
}

fn badge<'a>(view: &'a ChatView, label: &str) -> Option<&'a str> {
    view.tabs
        .iter()
        .find(|tab| tab.label.eq_ignore_ascii_case(label))
        .unwrap_or_else(|| panic!("missing chat tab {label}"))
        .badge
        .as_deref()
}

fn shows(view: &ChatView, needle: &str) -> bool {
    view.visible.iter().any(|line| line.text.contains(needle))
}

#[test]
fn offline_partner_tabs_unread_and_whisper_packet() {
    let mut chat = ChatSession::offline("kgap04");
    assert!(chat.window_open());

    chat.feed_public("Bob : hi");
    chat.feed_party("rally");
    chat.feed_system("notice");
    let view = chat.view();
    assert_eq!(view.active, "All");
    assert_eq!(unread(&view, "All"), 0);
    assert!(badge(&view, "All").is_none());
    assert_eq!(unread(&view, "Party"), 1);
    assert_eq!(badge(&view, "Party"), Some("1"));
    assert_eq!(unread(&view, "System"), 1);
    assert_eq!(unread(&view, "Whisper"), 0);

    chat.inject_tab("Party").unwrap();
    assert_eq!(unread(&chat.view(), "Party"), 0);

    chat.feed_own_public("mine");
    chat.feed_public("Cara : yo");
    assert_eq!(unread(&chat.view(), "All"), 1, "own public lines stay read");

    chat.feed_whisper("Dana", "secret");
    let view = chat.view();
    assert!(view.tabs.iter().any(|tab| tab.label == "Dana" && tab.closable));
    assert_eq!(unread(&view, "Dana"), 1);
    assert_eq!(badge(&view, "Dana"), Some("1"));
    assert_eq!(unread(&view, "Whisper"), 1);

    chat.feed_whisper("Eve", "other");
    chat.inject_tab("Whisper").unwrap();
    let view = chat.view();
    assert_eq!(unread(&view, "Whisper"), 0);
    assert!(shows(&view, "secret"));
    assert!(shows(&view, "other"));

    chat.inject_tab("Dana").unwrap();
    let view = chat.view();
    assert_eq!(view.active, "Dana");
    assert_eq!(unread(&view, "Dana"), 0);
    assert!(view.armed);
    assert_eq!(view.whisper_target, "Dana");
    assert!(shows(&view, "secret"));
    assert!(!shows(&view, "other"));

    let intent = chat
        .inject_text_enter("hello partner")
        .unwrap()
        .expect("the partner tab sends a whisper");
    assert_eq!(intent.header, 0x0096);
    assert_eq!(intent.target, "Dana");
    assert_eq!(intent.text, "hello partner");
    assert!(chat.view().armed, "staying on the partner tab keeps whisper mode");

    chat.acknowledge(2);
    let view = chat.view();
    assert!(view.visible.iter().any(|line| line.error && line.text == "Dana is ignoring you."));

    chat.close_partner("Dana").unwrap();
    let view = chat.view();
    assert!(view.window_open);
    assert!(chat.window_open());
    assert_eq!(view.active, "Whisper");
    assert!(!view.armed);
    assert!(!view.tabs.iter().any(|tab| tab.label == "Dana"));

    let public = chat.inject_text_enter("public now").unwrap();
    assert!(public.is_none(), "leaving the partner tab disarms the next line");
}

/// One TEST session whispers `kgap01` or `kgap02`, never anyone else.
#[test]
fn live_partner_tab_sends_whisper_on_test() {
    let mut chat = match ChatSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    chat.enter_world().unwrap_or_else(|error| panic!("{error}"));
    let me = chat.character_name().trim().to_owned();
    assert!(!me.is_empty(), "TEST character name is empty");
    let partner = if me.eq_ignore_ascii_case("kgap01") { "kgap02" } else { "kgap01" };
    assert!(!me.eq_ignore_ascii_case(partner));
    eprintln!("E2E_LIVE user={} partner={partner}", chat.username());

    chat.focus_partner(partner);
    let armed = chat.view();
    assert!(armed.armed);
    assert!(armed.tabs.iter().any(|tab| tab.label.eq_ignore_ascii_case(partner)));

    let intent = chat
        .inject_text_enter("g48")
        .unwrap_or_else(|error| panic!("{error}"))
        .expect("armed partner tab whispers");
    assert_eq!(intent.header, 0x0096);
    assert!(intent.target.eq_ignore_ascii_case(partner));
    assert_eq!(intent.text, "g48");
    assert!(chat.sent_headers().contains(&0x0096), "headers {:04x?}", chat.sent_headers());
    assert!(chat.window_open());
}
