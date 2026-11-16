//! English class names and party share settings. The offline case runs a
//! Hercules-shaped roster; the live case creates and leaves a one-member
//! party on TEST.

use korangar::e2e::{LiveConnect, LogicSession, job_names_scenario};

#[test]
fn class_names_and_party_settings_offline() {
    let report = job_names_scenario();
    assert_eq!(report.party_labels, [
        "[Leader] Ari  Rune Knight  Lv99  prontera  Online",
        "Bex  Dragon Knight  Lv99  prontera  Online",
        "Cyd  Christmas  Lv99  prontera  Online",
        "Dov  Summoner  Lv99  prontera  Online",
    ]);
    assert_eq!(
        report.options_text,
        "Experience: Even share\nItem pickup: Even share\nLoot: Each takes"
    );
    assert_eq!(report.view_equip_summary, "Sura · Female");
    assert_eq!(report.select_names, [
        (0, Some("Novice")),
        (4008, Some("Lord Knight")),
        (4054, Some("Rune Knight")),
        (4190, Some("Expanded Super Novice")),
        (4307, Some("Hyper Novice")),
        (4053, None),
    ]);
}

#[test]
fn party_class_names_on_test() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() % 100_000)
        .unwrap_or(0);
    let report = session.party_job_names_on_test(&format!("kjn{stamp}")).unwrap();
    eprintln!(
        "E2E_PARTY_JOBS classes={:?} already={} create={:?} own={:?} options={:?} switched={:?} left={}",
        report.character_classes,
        report.already_in_party,
        report.create_result,
        report.own_label,
        report.options_text,
        report.switched_options_text,
        report.left
    );
    assert!(!report.character_classes.is_empty());
    for (job, name) in &report.character_classes {
        assert!(name.is_some(), "TEST character job {job} has no class name");
    }
    if report.already_in_party {
        assert!(report.own_label.is_some());
        return;
    }
    let Some(create_result) = report.create_result else {
        eprintln!("E2E_SKIP the TEST character got no party-create reply (Basic Skill below 7); class names were checked");
        return;
    };
    assert_eq!(create_result, 0, "the TEST party create was refused");
    let own = report.own_label.as_deref().expect("own party row");
    assert!(!own.contains("Job "), "{own}");
    assert!(own.starts_with("[Leader] "), "{own}");
    assert_eq!(
        report.options_text,
        "Experience: Each takes\nItem pickup: Each takes\nLoot: Each takes"
    );
    assert_eq!(
        report.switched_options_text,
        "Experience: Even share\nItem pickup: Even share\nLoot: Each takes"
    );
    assert!(report.left);
}
