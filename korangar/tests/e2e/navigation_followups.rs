//! World-map routing and achievement reward names. Offline cases run the
//! window state on `navi_*`, `achievement_tbl` and `title_tbl` fixtures. The
//! live case routes from the cell Hercules TEST puts the pool character on.

use std::time::Duration;

use korangar::e2e::{LiveConnect, LogicSession, Phase, map_route_scenario, reward_detail_scenario};

const LINKS: &str = r#"Navi_Link = {
    { "prontera", 5001, 200, 99999, "", "", 156, 22, "prt_fild08", 170, 375 },
    { "prt_fild08", 5002, 200, 99999, "", "", 170, 20, "izlude", 128, 200 },
    { "izlude", 5004, 200, 99999, "", "", 128, 210, "prt_fild08", 170, 25 },
    { "prt_fild08", 5005, 200, 99999, "", "", 170, 380, "prontera", 156, 28 },
    { "prontera", 5003, 202, 117, "Kafra Employee", "", 146, 89, "geffen", 120, 60 },
}"#;
const NPCS: &str = r#"Navi_Npc = { { "izlude", 1, 101, 86, "Kafra Employee", "", 134, 87 } }"#;
const MOBS: &str = r#"Navi_Mob = { { "prt_fild08", 1, 300, 1002, "Poring", "PORING", 1, 70 } }"#;

#[test]
fn world_map_right_click_routes_to_the_map_and_arrives() {
    let reports = map_route_scenario((LINKS, NPCS, MOBS), ("prontera", 150, 150), "izlude", "Izlude", &[
        ("prt_fild08", 170, 375),
        ("izlude", 128, 200),
    ])
    .unwrap();
    assert_eq!(reports[0].status, "Route: 2 warps to izlude");
    assert_eq!(reports[0].steps.len(), 3);
    assert_eq!(reports[0].steps[2], "izlude: arrive in Izlude");
    assert!(
        reports[0].guide.starts_with("Next: warp at 156,22 to prt_fild08"),
        "{}",
        reports[0].guide
    );
    assert!(reports[1].guide.contains("to izlude"), "{}", reports[1].guide);
    assert_eq!(reports[2].guide, "Arrived: Izlude (izlude)");
}

#[test]
fn world_map_route_to_a_paid_only_map_asks_for_services() {
    let reports = map_route_scenario((LINKS, NPCS, MOBS), ("prontera", 150, 150), "geffen.gat", "Geffen", &[]).unwrap();
    assert!(reports[0].steps.is_empty());
    assert_eq!(reports[0].status, "No walking route from prontera to geffen; try Services");
}

const ACHIEVEMENTS: &str = r#"achievement_tbl = {
    [120001] = { group = "ADVENTURE", major = 2, minor = 0, title = "North Prontera Field Exploration(1)",
        content = { summary = "Discover", details = "Discover treasure" }, resource = {}, reward = { item = 22876 }, score = 10 },
    [230001] = { group = "BATTLE", major = 3, minor = 0, title = "Duel winner", content = { summary = "Win", details = "Win a duel" },
        resource = {}, reward = { title = 1001, buff = 34 }, score = 20 },
    [230002] = { group = "BATTLE", major = 3, minor = 0, title = "Unnamed", content = { summary = "", details = "" },
        resource = {}, reward = { item = 7777, title = 4242 }, score = 5 },
}"#;
const TITLES: &str = r#"title_tbl = { [1001] = "Duelist", [1002] = "Explorer" }"#;

#[test]
fn achievement_rewards_show_names_and_request_the_item_icon() {
    let items = [(22876, "Treasure Box")];
    let skills = [(34, "Blessing")];
    let item = reward_detail_scenario(ACHIEVEMENTS, TITLES, &items, &skills, 120001).unwrap();
    assert!(
        item.detail.contains(&"Reward: Treasure Box (item)".to_owned()),
        "{:?}",
        item.detail
    );
    assert_eq!(item.reward_item, Some(22876));
    assert_eq!(item.missing_icons, vec![22876]);

    let title = reward_detail_scenario(ACHIEVEMENTS, TITLES, &items, &skills, 230001).unwrap();
    assert!(
        title.detail.contains(&"Reward: title \"Duelist\", buff Blessing".to_owned()),
        "{:?}",
        title.detail
    );
    assert_eq!(title.reward_item, None);
    assert!(title.missing_icons.is_empty());

    // Ids the client tables do not name stay visible as ids.
    let unknown = reward_detail_scenario(ACHIEVEMENTS, TITLES, &items, &skills, 230002).unwrap();
    assert!(
        unknown.detail.contains(&"Reward: item 7777, title 4242".to_owned()),
        "{:?}",
        unknown.detail
    );
}

#[test]
fn live_world_map_route_from_the_test_spawn() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    session.enter_world_and_watch_statuses(Duration::from_secs(2)).expect("enter TEST");
    assert_eq!(session.phase(), Phase::InWorld);
    let (map, x, y) = session.player_map_cell().expect("TEST sent the map and cell");
    let reports = map_route_scenario((LINKS, NPCS, MOBS), (&map, x, y), "izlude", "Izlude", &[]).unwrap();
    eprintln!("E2E_LIVE start={map} {x},{y} status={:?}", reports[0].status);
    let status = &reports[0].status;
    if map == "izlude" {
        assert_eq!(status, "Route: on this map (Izlude)");
        assert_eq!(reports[0].guide, "Arrived: Izlude (izlude)");
    } else if ["prontera", "prt_fild08"].contains(&map.as_str()) {
        assert!(status.starts_with("Route: "), "{status}");
        assert!(reports[0].guide.starts_with("Next: warp at"), "{}", reports[0].guide);
    } else {
        // A start map outside the fixture has no link chain; the window says so.
        assert_eq!(status, &format!("No walking route from {map} to izlude; try Services"));
    }
}
