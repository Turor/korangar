//! English class names and party share settings, headless.
//!
//! [`job_names_scenario`] feeds a Hercules-shaped party roster with third,
//! fourth and outfit job ids into [`PartyState`] the way the client match arm
//! does and reads back the labels the party window draws, plus the class
//! names View Equipment and character select use. The live party round trip
//! is [`super::LogicSession::party_job_names_on_test`].

use korangar_networking::{PartyRosterEntry, PartyUpdate};
use ragnarok_packets::{AccountId, CharacterId};

use crate::state::job_names::job_name;
use crate::state::party::PartyState;
use crate::state::view_equip::ViewEquipState;

/// What the offline scenario observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobNamesReport {
    /// Party window rows, in roster order.
    pub party_labels: Vec<String>,
    /// The share-settings text under the roster.
    pub options_text: String,
    /// View Equipment's "class · sex" line for a Sura.
    pub view_equip_summary: String,
    /// Character select names for these job ids.
    pub select_names: Vec<(u16, Option<&'static str>)>,
}

fn roster_entry(account: u32, name: &str, job: i16, leader: bool) -> PartyRosterEntry {
    PartyRosterEntry {
        account_id: AccountId(account),
        character_id: CharacterId(account + 100),
        name: name.to_owned(),
        map_name: "prontera.gat".to_owned(),
        leader,
        online: true,
        job,
        base_level: 99,
        x: 0,
        y: 0,
        has_position: false,
    }
}

pub fn job_names_scenario() -> JobNamesReport {
    let mut party = PartyState::default();
    let local = AccountId(1);
    party.apply(
        PartyUpdate::Roster {
            name: "Turoran".to_owned(),
            members: vec![
                roster_entry(1, "Ari", 4060, true),
                roster_entry(2, "Bex", 4252, false),
                roster_entry(3, "Cyd", 26, false),
                roster_entry(4, "Dov", 4218, false),
            ],
        },
        local,
    );
    party.apply(
        PartyUpdate::Options {
            experience_share: 1,
            item_pickup: true,
            item_division: false,
        },
        local,
    );
    let party_labels = party.members.iter().map(|member| member.label.clone()).collect();

    let mut viewed = ViewEquipState::default();
    viewed.replace("Eve".to_owned(), 4070, 0, Vec::new());

    let select_names = [0u16, 4008, 4054, 4190, 4307, 4053]
        .into_iter()
        .map(|job| (job, job_name(i32::from(job))))
        .collect();

    JobNamesReport {
        party_labels,
        options_text: party.options_text.clone(),
        view_equip_summary: viewed.summary.clone(),
        select_names,
    }
}
