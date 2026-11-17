//! Skill descriptions reach the skill window tooltip (t273u). The offline
//! cases use a local host with remoteclient2's case rules; the live case reads
//! ro.turoran.com and skips unless `KORANGAR_E2E=1`.

use korangar::e2e::{DescriptionHost, LiveConnect, live_skill_description, offline_skill_description};

#[test]
fn bash_tooltip_has_turoran_description_offline() {
    let report = offline_skill_description(DescriptionHost::TuroranRenewal).unwrap_or_else(|error| panic!("{error}"));
    assert!(
        report
            .requests
            .iter()
            .any(|path| path == "/client/System/skilldescript.turoran.re2.lub"),
        "{:?}",
        report.requests
    );
    assert!(
        !report
            .requests
            .iter()
            .any(|path| path == "/client/system/skilldescript.turoran.re2.lub"),
        "an exact-case hit must not retry lower case: {:?}",
        report.requests
    );
    assert!(report.turoran_file_cached);
    assert!(
        report.bash_body.contains("Hits one enemy with a heavy blow."),
        "{}",
        report.bash_body
    );
    assert!(
        report
            .bash_tooltip
            .starts_with("Bash\nLevel: 0 / 10\nSP Cost (Lv 1): ^7777778^000000\nBash\n"),
        "{}",
        report.bash_tooltip
    );
    assert!(
        report.bash_tooltip.contains("Description: ^777777Hits one enemy"),
        "{}",
        report.bash_tooltip
    );
}

#[test]
fn bash_tooltip_falls_back_to_official_skillinfoz_offline() {
    let report = offline_skill_description(DescriptionHost::OfficialOnly).unwrap_or_else(|error| panic!("{error}"));
    assert!(
        report.bash_tooltip.contains("Official skillinfoz text."),
        "{}",
        report.bash_tooltip
    );
    assert!(!report.turoran_file_cached);
    assert!(
        report
            .requests
            .iter()
            .any(|path| path.eq_ignore_ascii_case("/client/data/luafiles514/lua%20files/skillinfoz/skilldescript.lub")),
        "{:?}",
        report.requests
    );
}

#[test]
fn bash_tooltip_from_test_asset_host() {
    let report = match live_skill_description() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("{error}"),
        LiveConnect::Ready(report) => report,
    };
    eprintln!(
        "E2E_SKILL_DESCRIPTIONS count={} bash_lines={}",
        report.description_count,
        report.bash_body.lines().count()
    );
    assert!(report.turoran_file_cached, "the Turoran renewal file was not the source");
    assert!(report.description_count > 1000, "{}", report.description_count);
    assert!(report.bash_body.starts_with("Bash"), "{}", report.bash_body);
    assert!(report.bash_body.contains("Description:"), "{}", report.bash_body);
    assert!(report.bash_tooltip.lines().count() > 4, "{}", report.bash_tooltip);
}
