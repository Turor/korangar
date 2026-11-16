//! Clan chat, attendance calendar, achievement catalog and navigation route.
//! Offline cases always run. The live case asks Hercules TEST for the
//! attendance calendar only when `KORANGAR_E2E=1` and an account is set up.

use korangar::e2e::{
    LiveConnect, LogicSession, achievement_scenario, attendance_scenario, clan_chat_scenario, navigation_follow, navigation_scenario,
};

const LINKS: &str = r#"Navi_Link = {
    { "prontera", 5001, 200, 99999, "", "", 156, 22, "prt_fild08", 170, 375 },
    { "prt_fild08", 5002, 200, 99999, "", "", 170, 20, "izlude", 128, 200 },
    { "prontera", 5003, 202, 117, "Kafra Employee", "", 146, 89, "geffen", 120, 60 },
}"#;
const NPCS: &str = r#"Navi_Npc = {
    { "izlude", 1, 101, 86, "Kafra Employee", "", 134, 87 },
    { "geffen", 2, 101, 86, "Geffen Guide", "", 119, 66 },
}"#;
const MOBS: &str = r#"Navi_Mob = { { "prt_fild08", 1, 300, 1002, "Poring", "PORING", 1, 70 } }"#;

#[test]
fn navigation_searches_and_routes_across_maps() {
    let report = navigation_scenario(LINKS, NPCS, MOBS, ("prontera", 150, 150), "kafra", false).unwrap();
    assert_eq!(report.hits, vec!["[NPC] Kafra Employee — izlude 134,87".to_owned()]);
    assert_eq!(report.status, "Route: 2 warps to izlude");
    assert_eq!(report.steps.len(), 3);
    assert!(report.steps[0].contains("warp portal at 156,22 to prt_fild08"));
    assert!(
        report.guide.starts_with("Next: warp at 156,22 to prt_fild08 — 128 cells S"),
        "{}",
        report.guide
    );
}

#[test]
fn navigation_needs_services_for_paid_links() {
    let walk = navigation_scenario(LINKS, NPCS, MOBS, ("prontera", 150, 150), "guide", false).unwrap();
    assert!(walk.steps.is_empty());
    assert!(walk.status.contains("try Services"), "{}", walk.status);
    let paid = navigation_scenario(LINKS, NPCS, MOBS, ("prontera", 150, 150), "guide", true).unwrap();
    assert_eq!(paid.steps.len(), 2);
    assert!(paid.steps[0].contains("service (Kafra Employee)"));
}

#[test]
fn navigation_guide_follows_the_player() {
    let reports = navigation_follow(LINKS, NPCS, MOBS, ("prontera", 150, 150), "kafra", &[
        ("prt_fild08", 170, 375),
        ("izlude", 128, 200),
        ("izlude", 133, 88),
    ])
    .unwrap();
    assert!(reports[1].guide.contains("to izlude"));
    assert!(reports[2].guide.starts_with("Destination 134,87"));
    assert_eq!(reports[3].guide, "Arrived: Kafra Employee");
}

#[test]
fn navigation_monster_search_ends_on_the_map() {
    let report = navigation_scenario(LINKS, NPCS, MOBS, ("prontera", 150, 150), "por", false).unwrap();
    assert_eq!(report.hits, vec!["[MOB] Poring Lv 1 — prt_fild08".to_owned()]);
    assert!(report.steps.last().unwrap().contains("anywhere on the map"));
}

const ACH_BASE: &str = r#"achievement_tbl = {
    [120001] = { group = "ADVENTURE", major = 2, minor = 0, title = "kr", content = { summary = "", details = "" },
        resource = { [1] = { text = "kr goal" } }, reward = { item = 22876 }, score = 10 },
    [230001] = { group = "BATTLE", major = 3, minor = 0, title = "Duel winner", content = { summary = "Win", details = "Win a duel" },
        resource = {}, reward = { title = 1001, buff = 3 }, score = 20 },
}"#;
const ACH_EN: &str = r#"achievement_tbl = {
    [120001] = { group = "ADVENTURE", major = 2, minor = 0, title = "North Prontera Field Exploration(1)",
        content = { summary = "Discover hidden treasure", details = "Discover treasure in north Prontera field" },
        resource = { [1] = { text = "Discover treasure in north Prontera field" } }, reward = { item = 22876 }, score = 10 },
}"#;

#[test]
fn achievements_use_the_client_table() {
    let entries = [(120001, true, false), (230001, false, false)];
    let summary = achievement_scenario(ACH_BASE, ACH_EN, &entries, (0, None), Some(120001)).unwrap();
    assert!(summary.rows[0].starts_with("North Prontera Field Exploration(1) · complete"));
    assert!(summary.rows[1].starts_with("Duel winner · in progress"));
    assert!(summary.gauges.contains(&"Adventure 1/1".to_owned()));
    assert!(summary.gauges.contains(&"Battle 0/1".to_owned()));
    assert_eq!(summary.detail[0], "North Prontera Field Exploration(1)");
    assert!(summary.detail.contains(&"Reward: item 22876".to_owned()));
    assert_eq!(summary.claimable, Some(120001));

    let battle = achievement_scenario(ACH_BASE, ACH_EN, &entries, (3, Some(0)), Some(230001)).unwrap();
    assert!(battle.header.contains("category Battle / Duel"));
    assert_eq!(battle.rows.len(), 1);
    assert!(battle.detail.contains(&"Reward: title 1001, buff 3".to_owned()));
    assert_eq!(battle.claimable, None);
}

const CHECK_ATTENDANCE: &str = r#"
Config = { EvendOnOff = 1, StartDate = 20261021, EndDate = 20991231 }
Reward = { { 1, 12211, 5 }, { 2, 608, 3 }, { 3, 12208, 1 } }
function main()
    InsertCheckAttendanceConfig(Config.EvendOnOff, Config.StartDate, Config.EndDate)
    for k, r in pairs(Reward) do InsertCheckAttendanceReward(r[1], r[2], r[3]) end
    return true, "success"
end
"#;

#[test]
fn attendance_calendar_uses_client_rewards() {
    let (labels, states, status) = attendance_scenario(CHECK_ATTENDANCE, 1, false);
    assert_eq!(labels, vec![
        "Day 1\nItem 12211 x5",
        "Day 2\nItem 608 x3",
        "Day 3\nItem 12208 x1"
    ]);
    assert_eq!(states, vec!["Claimed", "Today", "Upcoming"]);
    assert!(status.contains("ready"));
    let (_, claimed, status) = attendance_scenario(CHECK_ATTENDANCE, 2, true);
    assert_eq!(claimed, vec!["Claimed", "Claimed", "Upcoming"]);
    assert!(status.contains("claimed"));
}

#[test]
fn clan_chat_routes_and_formats() {
    assert_eq!(
        clan_chat_scenario("/cl hello", "Bob : hi\0"),
        (Some("hello".to_owned()), "[Clan] Bob : hi".to_owned())
    );
    assert_eq!(clan_chat_scenario("/clear", "").0, None);
}

#[test]
fn autopilot_lines_name_the_social_windows() {
    // The logic session only tracks windows it models, but these lines must
    // parse so verify scripts can open the real windows.
    let mut session = LogicSession::offline();
    for line in [
        "open navigation",
        "open achievements",
        "open attendance",
        "event ToggleAttendanceWindow",
    ] {
        session.apply_line(line).unwrap_or_else(|error| panic!("{line}: {error}"));
    }
    assert!(session.apply_line("open clanattendance").is_err());
}

#[test]
fn attendance_calendar_on_test() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    match session.request_attendance_live() {
        Ok(Some(reply)) => {
            eprintln!(
                "E2E_LIVE attendance days={} claimed_today={}",
                reply.days_attended, reply.claimed_today
            );
            assert!(reply.days_attended <= 31);
        }
        Ok(None) => eprintln!("E2E_SKIP TEST sent no attendance calendar (feature off or outside its period)"),
        Err(error) => panic!("{error}"),
    }
}
