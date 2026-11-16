//! Local GRF and folder overlays. Offline cases use a temp install only.
//! The live case skips unless `KORANGAR_E2E=1` and Hercules TEST is reachable.

use korangar::e2e::archives::{probe_offline_archives, start_live_with_overlay};
use korangar::e2e::{LiveConnect, LogicSession, Outbound, Phase};

#[test]
fn offline_overlay_beats_grf_for_the_same_path() {
    let report = probe_offline_archives().unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(report.shared_bytes, b"from-overlay");
    assert!(report.shared_origin.starts_with("overlay:"), "{}", report.shared_origin);
    assert!(report.shared_origin.contains(&report.install_dir.display().to_string()));
    assert_eq!(report.lub_bytes, b"lub-from-overlay");
    assert!(report.lub_origin.starts_with("overlay:"), "{}", report.lub_origin);
    assert_eq!(report.grf_only_bytes, b"only-grf");
    assert!(report.grf_only_origin.starts_with("archive:"), "{}", report.grf_only_origin);
    assert!(report.grf_only_origin.ends_with("base.grf"), "{}", report.grf_only_origin);
    assert!(report.grf_only_origin.contains(&report.install_dir.display().to_string()));
    assert_eq!(report.korangar_bytes, b"from-korangar");
    assert!(report.korangar_origin.starts_with("korangar:"), "{}", report.korangar_origin);
    assert_eq!(report.display_name, "E2E Overlay Prontera");
    assert!(
        report.warnings.iter().any(|warning| warning.contains("missing-required.grf")),
        "{:?}",
        report.warnings
    );
    assert!(
        !report.warnings.iter().any(|warning| warning.contains("missing-optional")),
        "{:?}",
        report.warnings
    );
    assert!(report.anchor_line.contains("install directory"), "{}", report.anchor_line);
    assert!(report.anchor_line.contains(&report.install_dir.display().to_string()));
    assert_ne!(report.install_dir, std::env::current_dir().unwrap());
    assert!(report.search_order[0].starts_with("overlay:"), "{:?}", report.search_order);
    let archive_at = report
        .search_order
        .iter()
        .position(|entry| entry.starts_with("archive:"))
        .expect("configured GRF is mounted");
    let korangar_at = report
        .search_order
        .iter()
        .position(|entry| entry.starts_with("korangar:"))
        .expect("korangar archive is mounted");
    assert!(archive_at < korangar_at, "{:?}", report.search_order);

    let mut session = LogicSession::offline();
    session.apply_line("open GameSettings").unwrap();
    session.apply_line("tab AudioSettings").unwrap();
    assert_eq!(session.phase(), Phase::Idle);
    assert!(
        session
            .outbound()
            .iter()
            .any(|item| matches!(item, Outbound::SwitchWindowTab { to, .. } if to == "AudioSettings"))
    );
}

#[test]
fn live_overlay_reaches_character_select() {
    match start_live_with_overlay() {
        LiveConnect::Skip(reason) => eprintln!("E2E_SKIP {reason}"),
        LiveConnect::Invalid(error) => panic!("{error}"),
        LiveConnect::Ready(mut live) => {
            live.session.login_to_character_select().unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(live.session.phase(), Phase::CharacterSelect);
            assert!(live.session.outbound().iter().any(|item| matches!(item, Outbound::Login)));
            assert_eq!(live.display_name, "E2E Overlay Prontera");
            assert_eq!(live.asset_bytes, b"prontera.rsw#E2E Overlay Prontera#\n");
            assert!(live.origin.starts_with("overlay:"), "{}", live.origin);
        }
    }
}
