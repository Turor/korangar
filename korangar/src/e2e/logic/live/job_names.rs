//! Live party round trip on Hercules TEST for class names and share
//! settings: create a one-member party, switch its share settings, read the
//! row and settings text the party window would draw, and leave. A character
//! that is already in a party is only read, never made to leave.

use std::time::{Duration, Instant};

use korangar_networking::NetworkEvent;
use ragnarok_packets::AccountId;

use super::super::LogicSession;
use crate::state::job_names::job_name;
use crate::state::party::PartyState;

/// What the live round trip saw.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LivePartyJobReport {
    /// (job id, class name) per character on the TEST account.
    pub character_classes: Vec<(u16, Option<&'static str>)>,
    pub already_in_party: bool,
    /// `0x00FA` result of the create. `None` when no create was sent or the
    /// server did not answer with `0x00FA` (Basic Skill below 7).
    pub create_result: Option<u8>,
    /// The session character's own party row.
    pub own_label: Option<String>,
    pub options_text: String,
    /// Settings text after switching experience and pickup to even share.
    pub switched_options_text: String,
    pub left: bool,
}

const STEP: Duration = Duration::from_secs(6);

impl LogicSession {
    /// Log in, enter the map, and run the party round trip. Only TEST.
    pub fn party_job_names_on_test(&mut self, party_name: &str) -> Result<LivePartyJobReport, String> {
        self.login_to_character_select()?;
        let mut report = LivePartyJobReport {
            character_classes: self
                .headgear
                .characters()
                .iter()
                .map(|character| (character.job_id.0, job_name(i32::from(character.job_id.0))))
                .collect(),
            ..LivePartyJobReport::default()
        };
        self.enter_world()?;
        let local = self.login_data.map(|data| data.account_id).ok_or("no login data")?;

        let mut party = PartyState::default();
        // A party the character already has arrives after the map-loaded ack.
        self.pump_party(&mut party, local, Duration::from_secs(3), |_| false)?;
        if party.joined {
            report.already_in_party = true;
            report.own_label = own_label(&party, local);
            report.options_text.clone_from(&party.options_text);
            return Ok(report);
        }

        self.party_net()?
            .create_party(party_name.to_owned(), false, false)
            .map_err(not_connected)?;
        // Hercules answers a create without Basic Skill 7 with a skill
        // failure instead of `0x00FA`, so no reply is reported, not an error.
        let mut create_result = None;
        let _ = self.pump_party_with(&mut party, local, STEP, &mut create_result, |party, result| {
            result.is_some() && (party.joined || *result != Some(0))
        });
        self.take_failure()?;
        report.create_result = create_result;
        if create_result != Some(0) {
            return Ok(report);
        }
        // The roster can trail the create ack.
        let _ = self.pump_party(&mut party, local, STEP, |party| own_label(party, local).is_some());
        report.own_label = own_label(&party, local);
        report.options_text.clone_from(&party.options_text);

        let before = party.options_text.clone();
        let switched = self
            .party_net()?
            .set_party_options(1, true, false)
            .map_err(not_connected)
            .and_then(|()| self.pump_party(&mut party, local, STEP, |party| party.options_text != before));
        report.switched_options_text.clone_from(&party.options_text);

        self.party_net()?.leave_party().map_err(not_connected)?;
        self.pump_party(&mut party, local, STEP, |party| !party.joined)?;
        report.left = !party.joined;
        switched.map(|()| report)
    }

    fn party_net(&mut self) -> Result<&mut korangar_networking::NetworkingSystem<super::SentHeaders>, String> {
        self.net.as_mut().ok_or_else(|| "logic mode has no network".to_owned())
    }

    fn pump_party(
        &mut self,
        party: &mut PartyState,
        local: AccountId,
        budget: Duration,
        done: impl Fn(&PartyState) -> bool,
    ) -> Result<(), String> {
        let mut ignored = None;
        self.pump_party_with(party, local, budget, &mut ignored, |party, _| done(party))
    }

    /// Party updates go to `party` (create acks also to `create_result`);
    /// everything else to the generic handler. Without a `done` match the
    /// whole budget is spent and that is not an error when `done` can never
    /// match the empty state.
    fn pump_party_with(
        &mut self,
        party: &mut PartyState,
        local: AccountId,
        budget: Duration,
        create_result: &mut Option<u8>,
        done: impl Fn(&PartyState, &Option<u8>) -> bool,
    ) -> Result<(), String> {
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            let drained = {
                let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) else {
                    return Err("logic mode has no network".to_owned());
                };
                net.get_events(events);
                events.drain().collect::<Vec<_>>()
            };
            for event in drained {
                match event {
                    NetworkEvent::Party(update) => {
                        if let korangar_networking::PartyUpdate::CreateResult(result) = &update {
                            *create_result = Some(*result);
                        }
                        party.apply(update, local);
                    }
                    other => self.on_event(other)?,
                }
            }
            self.take_failure()?;
            if done(party, create_result) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if done(party, create_result) || budget <= Duration::from_secs(3) {
            Ok(())
        } else {
            Err(self.timeout("the party reply"))
        }
    }
}

fn own_label(party: &PartyState, local: AccountId) -> Option<String> {
    party
        .members
        .iter()
        .find(|member| member.account_id == local)
        .map(|member| member.label.clone())
}

fn not_connected<E>(_: E) -> String {
    "map server is not connected".to_owned()
}
