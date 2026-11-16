//! Full item tooltip and Shift compare (inventory review #4, #5).
//!
//! Offline always runs. Live reads the TEST character's inventory and skips
//! when credentials or Hercules TEST are missing.

use korangar::e2e::{LiveConnect, LogicSession, offline_tooltip_view};

#[test]
fn item_tooltip_lines_offline() {
    let view = offline_tooltip_view();
    assert!(view.weapon.contains("+7"), "{}", view.weapon);
    assert!(view.weapon.contains("Grade 2"), "{}", view.weapon);
    assert!(view.weapon.contains("[2]"), "{}", view.weapon);
    assert!(view.weapon.contains("Poring Card"), "{}", view.weapon);
    assert!(view.weapon.contains("Drops Card"), "{}", view.weapon);
    assert!(view.weapon.contains("STR +5"), "{}", view.weapon);
    assert!(view.weapon.contains("Weight: 50"), "{}", view.weapon);
    assert!(view.weapon.contains("Required Level: 4"), "{}", view.weapon);
    assert!(view.weapon.contains("Swordsman classes"), "{}", view.weapon);
    assert!(view.armor_compare.contains("Equipped"), "{}", view.armor_compare);
    assert!(view.armor_compare.contains("Cotton Shirt"), "{}", view.armor_compare);
    assert!(view.potion.contains("Quantity: 12"), "{}", view.potion);
    assert!(!view.potion.contains("Equipped"), "{}", view.potion);
    assert!(!view.potion.contains("Cotton Shirt"), "{}", view.potion);
    assert_eq!(view.unidentified, "Unidentified Sword\nYou can't tell.");
}

#[test]
fn item_tooltips_on_test_character() {
    let mut session = match LogicSession::start_live() {
        LiveConnect::Skip(reason) => {
            eprintln!("E2E_SKIP {reason}");
            return;
        }
        LiveConnect::Invalid(error) => panic!("e2e account: {error}"),
        LiveConnect::Ready(session) => session,
    };
    let report = session.item_tooltips_on_test().unwrap();
    eprintln!(
        "E2E_ITEM_TOOLTIP rows={} built={} worn={} compare_pairs={} note={}",
        report.rows, report.built, report.worn, report.compare_pairs, report.note
    );
    assert!(report.inventory_seen);
    assert_eq!(report.built, report.rows);
    if let Some((worn_name, compare)) = &report.sample_compare {
        assert!(compare.contains("Equipped"), "{compare}");
        assert!(compare.contains(worn_name), "{compare} missing {worn_name}");
    }
}
