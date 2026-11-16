//! In-client end-to-end scenarios. See `docs/e2e-harness.md`.
//!
//! Offline cases always run. Live cases print `E2E_SKIP` and return when
//! `KORANGAR_E2E` / an account file is unset, or when Hercules TEST is down.

#[path = "e2e/archives.rs"]
mod archives;
#[path = "e2e/minimap.rs"]
mod minimap;
#[path = "e2e/release.rs"]
mod release;
#[path = "e2e/social.rs"]
mod social;
#[path = "e2e/status_icons.rs"]
mod status_icons;

use std::path::PathBuf;

use korangar::e2e::{
    Capture, LiveConnect, LogicSession, Outbound, Phase, capture_client_settings, capture_display, capture_lavapipe, input_path_parity,
    lavapipe_icd_present, logic_mode_skips_image_assertions,
};

#[path = "e2e/chat.rs"]
mod chat;

fn live_session() -> Option<LogicSession> {
    match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            None
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => {
            eprintln!("E2E_LIVE user={}", session.username());
            Some(session)
        }
    }
}

fn shot_dir() -> PathBuf {
    std::env::var_os("KORANGAR_E2E_SHOT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("korangar-e2e-shots"))
}

#[test]
fn injected_input_matches_winit() {
    let parity = input_path_parity();
    assert!(parity.winit_escape && parity.injected_escape);
    assert_eq!(parity.winit_text, "Hi");
    assert_eq!(parity.injected_text, parity.winit_text);
    assert!(parity.winit_click && parity.injected_click);
    assert!(parity.winit_drag && parity.injected_drag);
}

#[test]
fn logic_settings_group_and_dialog_option() {
    let mut session = LogicSession::offline();
    let closed = session.settings_view();
    assert!(
        closed.open_member.is_none(),
        "an empty session must not invent the Settings catalog"
    );
    session.apply_line("open GameSettings").unwrap();
    session.apply_line("tab AudioSettings").unwrap();
    let view = session.settings_view();
    assert_eq!(view.group_name, Some("Settings"));
    assert_eq!(view.tab_labels, ["Interface", "Graphics", "Audio", "Game", "Keys"]);
    assert_eq!(view.open_member.as_deref(), Some("AudioSettings"));
    assert!(
        session
            .outbound()
            .iter()
            .any(|item| matches!(item, Outbound::SwitchWindowTab { to, .. } if to == "AudioSettings"))
    );

    session.preview_dialog(7, "Welcome", &["Storage", "Cancel"]);
    let index = session.choose_option_by_text("cancel").unwrap();
    assert_eq!(index, 1);
    assert_eq!(session.last_selected(), Some(1));
    assert!(session.dialog().unwrap().visible_text().contains("Welcome"));
    assert!(matches!(
        session.outbound().last(),
        Some(Outbound::ChooseDialogOption { npc_id: 7, option: 2 })
    ));
    assert!(logic_mode_skips_image_assertions());
}

#[test]
fn lavapipe_screenshot_when_present() {
    if !lavapipe_icd_present() {
        eprintln!("E2E_SKIP lavapipe ICD is not installed");
        return;
    }
    let path = shot_dir().join("lavapipe-probe.png");
    match capture_lavapipe(&path) {
        Ok(None) => eprintln!("E2E_SKIP lavapipe adapter was not selected"),
        Ok(Some(Capture { bytes, .. })) => {
            assert!(bytes > 8, "screenshot is empty");
            let magic = std::fs::read(&path).unwrap();
            assert_eq!(&magic[..8], b"\x89PNG\r\n\x1a\n");
        }
        Err(error) => panic!("{error}"),
    }
    if let Err(error) = capture_display(&shot_dir().join("display-root.png")) {
        panic!("{error}");
    }
}

#[test]
fn login_reaches_character_select() {
    let Some(mut session) = live_session() else {
        return;
    };
    session.login_to_character_select().unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(session.phase(), Phase::CharacterSelect);
    assert!(session.outbound().iter().any(|item| matches!(item, Outbound::Login)));
    assert!(session.outbound().iter().any(|item| matches!(item, Outbound::SelectServer)));
}

#[test]
fn settings_tabs_on_test() {
    let Some(mut session) = live_session() else {
        return;
    };
    let view = session.open_settings_group().unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(view.group_name, Some("Settings"));
    assert_eq!(view.tab_labels, ["Interface", "Graphics", "Audio", "Game", "Keys"]);
    assert_eq!(view.open_member.as_deref(), Some("AudioSettings"));
    assert_eq!(session.phase(), Phase::InWorld);
    assert!(
        session
            .outbound()
            .iter()
            .any(|item| matches!(item, Outbound::SwitchWindowTab { to, .. } if to == "AudioSettings"))
    );
}

#[test]
fn npc_dialog_option_on_test() {
    let Some(mut session) = live_session() else {
        return;
    };
    let chosen = session
        .enter_world_and_choose_npc_option()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!chosen.text.is_empty());
    assert_eq!(chosen.wire_option, i8::try_from(chosen.index + 1).unwrap());
    assert_eq!(session.last_selected(), Some(chosen.index));
    assert!(session.outbound().iter().any(|item| matches!(item, Outbound::MapLoaded)));
    assert!(session.outbound().iter().any(|item| {
        matches!(
            item,
            Outbound::ChooseDialogOption { option, .. } if *option == chosen.wire_option
        )
    }));
    assert!(
        session
            .sent_choices()
            .iter()
            .any(|choice| choice.npc_id == chosen.npc_id && choice.option == chosen.wire_option),
        "choices {:?}, headers {:04x?}",
        session.sent_choices(),
        session.sent_headers()
    );
}

/// Real client window under Xvfb and lavapipe. Skips unless live tests are
/// opted in. A missing renderer does not fail the settings logic scenario.
#[test]
fn client_window_shows_settings() {
    if std::env::var("KORANGAR_E2E").ok().as_deref() != Some("1") {
        eprintln!("E2E_SKIP set KORANGAR_E2E=1 to capture the client window");
        return;
    }
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_korangar"));
    match capture_client_settings(&bin, &shot_dir(), "kgap28") {
        Ok(Capture { path, bytes }) => {
            assert!(bytes > 1000, "client window shot is too small");
            assert_eq!(&std::fs::read(&path).unwrap()[..8], b"\x89PNG\r\n\x1a\n");
            eprintln!("E2E_SHOT {}", path.display());
        }
        Err(error) => panic!("{error}"),
    }
}
